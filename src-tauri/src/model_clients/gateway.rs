//! Authenticated namespaces inside the existing CC Switch proxy. No global
//! provider fallback: a Muse request can never consume a Codex/Claude key.
use super::{read_store, resolve_key, ClientProfile, ModelClient};
use crate::proxy::{
    providers::transform_codex_responses_namespace::{
        flatten_request_namespaces, namespace_restore_map, restore_response_namespaces,
        restore_sse_event_namespaces,
    },
    providers::{
        streaming_codex_anthropic, streaming_codex_chat, transform_codex_anthropic,
        transform_codex_chat,
    },
    sse::{append_utf8_safe, strip_sse_field, take_sse_block},
};
use axum::{
    body::{Body, Bytes},
    extract::{Path, Request},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures::StreamExt;
use serde_json::{json, Value};

pub(crate) mod gemini;

fn error(status: StatusCode, message: &str) -> Response {
    #[cfg(test)]
    eprintln!("GATEWAY ERROR {status}: {message}");
    (
        status,
        Json(json!({"error":{"message":message,"type":"model_client_error"}})),
    )
        .into_response()
}

fn authenticated(headers: &HeaderMap, query: Option<&str>, token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    let key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| headers.get("x-goog-api-key").and_then(|v| v.to_str().ok()));
    let query_key = query.and_then(|q| {
        url::form_urlencoded::parse(q.as_bytes())
            .find(|(k, _)| k == "key")
            .map(|(_, v)| v.into_owned())
    });
    let candidate = key.or(query_key.as_deref()).unwrap_or("");
    candidate.len() == token.len()
        && candidate
            .bytes()
            .zip(token.bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
}

fn parse_client(s: &str) -> Option<ModelClient> {
    match s {
        "antigravity" => Some(ModelClient::Antigravity),
        "muse" => Some(ModelClient::Muse),
        _ => None,
    }
}

pub(crate) fn catalog(client: ModelClient, p: &ClientProfile) -> Value {
    match client {
        ModelClient::Muse => {
            json!({"data":p.models.iter().enumerate().map(|(i,m)|json!({"id":m.id,"model_id":m.id,"provider_id":"meta","profile_id":m.id,"display_label":m.name,"visibility":"visible","release_date":"","display_order":i,"is_default":m.id==p.default_model,"context_limit":m.context,"output_limit":m.output})).collect::<Vec<_>>()})
        }
        ModelClient::Antigravity => {
            json!({"models":p.models.iter().map(|m|json!({"name":format!("models/{}",m.id),"displayName":m.name,"inputTokenLimit":m.context.saturating_sub(m.output),"outputTokenLimit":m.output,"supportedGenerationMethods":["generateContent","streamGenerateContent"]})).collect::<Vec<_>>()})
        }
        _ => {
            json!({"object":"list","data":p.models.iter().map(|m|json!({"id":m.id,"object":"model","owned_by":"configured-provider","name":m.name,"context_length":m.context})).collect::<Vec<_>>()})
        }
    }
}

pub async fn handle(Path((client, path)): Path<(String, String)>, request: Request) -> Response {
    match crate::deck::binding_active(&client) {
        Ok(true) => return crate::deck::gateway::handle_client(&client, &path, request).await,
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "ModelDeck resources are unavailable",
            )
        }
        Ok(false) => {}
    }
    let Some(client) = parse_client(&client) else {
        return error(StatusCode::NOT_FOUND, "Unknown model client");
    };
    let profile = match read_store().and_then(|s| {
        s.active
            .get(&client)
            .cloned()
            .ok_or("Client profile not configured".into())
    }) {
        Ok(p) if p.enabled => p,
        _ => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Client profile is not active",
            )
        }
    };
    handle_profile(client, profile, &path, request).await
}

pub async fn muse_models(request: Request) -> Response {
    match crate::deck::binding_active("muse") {
        Ok(true) => {
            return crate::deck::gateway::handle_client("muse", "muse-code/models", request).await
        }
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "ModelDeck resources are unavailable",
            )
        }
        Ok(false) => {}
    }
    let profile = match read_store().and_then(|s| {
        s.active
            .get(&ModelClient::Muse)
            .cloned()
            .ok_or("Muse profile not configured".into())
    }) {
        Ok(p) if p.enabled => p,
        _ => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Muse profile is not active",
            )
        }
    };
    handle_profile(ModelClient::Muse, profile, "muse-code/models", request).await
}

async fn handle_profile(
    client: ModelClient,
    profile: ClientProfile,
    path: &str,
    request: Request,
) -> Response {
    if !authenticated(
        request.headers(),
        request.uri().query(),
        &profile.local_token,
    ) {
        return error(
            StatusCode::UNAUTHORIZED,
            "Invalid client gateway credential",
        );
    }
    let path = path.trim_start_matches("v1/").trim_start_matches("v1beta/");
    if request.method() == axum::http::Method::GET && matches!(path, "models" | "muse-code/models")
    {
        return Json(catalog(client, &profile)).into_response();
    }
    if request.method() != axum::http::Method::POST {
        return error(StatusCode::NOT_FOUND, "Unsupported model endpoint");
    }
    let bytes = match axum::body::to_bytes(request.into_body(), 32 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Invalid or oversized request body",
            )
        }
    };
    let body: Value = match serde_json::from_slice(&bytes) {
        Ok(b) => b,
        Err(_) => return error(StatusCode::BAD_REQUEST, "Invalid JSON request"),
    };
    let key = match resolve_key(&profile) {
        Ok(k) => k,
        Err(e) => return error(StatusCode::SERVICE_UNAVAILABLE, &e),
    };
    let http = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(1800))
        .build()
    {
        Ok(h) => h,
        Err(_) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Cannot initialize upstream transport",
            )
        }
    };
    if client == ModelClient::Antigravity {
        return gemini::handle(&http, &profile, &key, path, body).await;
    }
    let suffix = match (client, path) {
        (ModelClient::Muse, "responses") => "responses",
        _ => return error(StatusCode::NOT_FOUND, "Unsupported model endpoint"),
    };
    forward(
        &http,
        &profile,
        &key,
        suffix,
        body,
        client == ModelClient::Muse,
    )
    .await
}

fn set_model(p: &ClientProfile, body: &mut Value) -> Result<(), String> {
    if !body.is_object() {
        return Err("Inference requests must be JSON objects".into());
    }
    let id = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(&p.default_model)
        .to_owned();
    #[cfg(test)]
    eprintln!(
        "MODEL {id}, max_output_tokens={:?}",
        body.get("max_output_tokens")
    );
    let model = p
        .models
        .iter()
        .find(|m| m.id == id)
        .ok_or("Model is not in this client's configured catalog")?;
    body["model"] = json!(id);
    for field in ["max_output_tokens", "max_tokens"] {
        if let Some(n) = body.get(field).and_then(Value::as_u64) {
            body[field] = json!(n.min(model.output));
        }
    }
    Ok(())
}

pub(crate) async fn forward(
    http: &reqwest::Client,
    p: &ClientProfile,
    key: &str,
    suffix: &str,
    mut body: Value,
    muse: bool,
) -> Response {
    if let Err(e) = set_model(p, &mut body) {
        return error(StatusCode::BAD_REQUEST, &e);
    }
    let provider = super::provider_adapter::project(p, key);
    let map = namespace_restore_map(&body);
    let tool_context = transform_codex_chat::build_codex_tool_context_from_request(&body);
    let stream = body.get("stream").and_then(Value::as_bool).unwrap_or(false);
    let mut url = super::provider_adapter::url(p, suffix);
    if muse {
        if p.protocol == "openai-responses"
            && crate::proxy::providers::provider_needs_responses_namespace_flatten(&provider)
            && flatten_request_namespaces(&mut body).is_err()
        {
            return error(
                StatusCode::BAD_REQUEST,
                "Ambiguous namespaced tool definitions",
            );
        }
        // Muse stores history locally. Provider-side storage and incompatible
        // routing hints must not silently select another model or account.
        body["store"] = json!(false);
    }
    if muse && p.protocol == "openai-chat" {
        url = super::provider_adapter::url(p, "chat/completions");
        let prompt_cache_key = body
            .get("prompt_cache_key")
            .and_then(Value::as_str)
            .map(str::to_string);
        let reasoning = super::provider_adapter::reasoning_config(&provider, &body);
        body = match transform_codex_chat::responses_to_chat_completions_with_reasoning(
            body,
            reasoning.as_ref(),
        ) {
            Ok(v) => v,
            Err(_) => {
                return error(
                    StatusCode::BAD_REQUEST,
                    "The Responses request cannot be represented as Chat Completions",
                )
            }
        };
        if reasoning.is_none() && !p.effort.is_empty() {
            body["reasoning_effort"] = json!(p.effort);
        }
        crate::proxy::providers::inject_codex_chat_prompt_cache_key(
            &provider,
            &mut body,
            prompt_cache_key.as_deref(),
            None,
        );
        if crate::proxy::providers::transform_codex_chat_moonshot_schema::upstream_requires_ref_sibling_all_of(&p.endpoint) {
            crate::proxy::providers::transform_codex_chat_moonshot_schema::wrap_ref_siblings_in_chat_tools(&mut body);
        }
    } else if muse && p.protocol == "anthropic" {
        url = super::provider_adapter::url(p, "/v1/messages");
        let limit = p
            .models
            .iter()
            .find(|m| Some(m.id.as_str()) == body.get("model").and_then(Value::as_str))
            .map(|m| m.output)
            .unwrap_or(8192);
        body = match transform_codex_anthropic::responses_request_to_anthropic(body, limit) {
            Ok(v) => v,
            Err(_) => {
                return error(
                    StatusCode::BAD_REQUEST,
                    "The Responses request cannot be represented as Anthropic Messages",
                )
            }
        };
        // Carry the chosen value across field names without inferring a tier.
        if !p.effort.is_empty() {
            if !body["output_config"].is_object() {
                body["output_config"] = json!({});
            }
            body["output_config"]["effort"] = json!(p.effort);
        }
    }
    let req = http
        .post(url)
        .header("accept-encoding", "identity")
        .json(&body);
    let req = match super::provider_adapter::authenticate(req, p, key) {
        Ok(request) => request,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "Invalid upstream authentication configuration",
            )
        }
    };
    let upstream = match req.send().await {
        Ok(r) => r,
        Err(_) => return error(StatusCode::BAD_GATEWAY, "Upstream connection failed"),
    };
    if !upstream.status().is_success() {
        return error(
            upstream.status(),
            &format!("Upstream returned HTTP {}", upstream.status().as_u16()),
        );
    }
    if !stream {
        let mut data: Value = match upstream.json().await {
            Ok(v) => v,
            Err(_) => return error(StatusCode::BAD_GATEWAY, "Invalid upstream JSON"),
        };
        if muse && p.protocol == "openai-chat" {
            data = match transform_codex_chat::chat_completion_to_response_with_context(
                data,
                &tool_context,
            ) {
                Ok(v) => v,
                Err(_) => {
                    return error(StatusCode::BAD_GATEWAY, "Invalid Chat Completions response")
                }
            };
        } else if muse && p.protocol == "anthropic" {
            data = match transform_codex_anthropic::anthropic_response_to_responses_with_context(
                data,
                &tool_context,
            ) {
                Ok(v) => v,
                Err(_) => return error(StatusCode::BAD_GATEWAY, "Invalid Anthropic response"),
            };
        } else {
            restore_response_namespaces(&mut data, &map);
        }
        return Json(data).into_response();
    }
    let input: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<Bytes, std::io::Error>> + Send>,
    > = if muse && p.protocol == "openai-chat" {
        Box::pin(
            streaming_codex_chat::create_responses_sse_stream_from_chat_with_context(
                upstream.bytes_stream(),
                tool_context,
            ),
        )
    } else if muse && p.protocol == "anthropic" {
        Box::pin(
            streaming_codex_anthropic::create_responses_sse_stream_from_anthropic_with_context(
                upstream.bytes_stream(),
                tool_context,
            ),
        )
    } else {
        Box::pin(
            upstream
                .bytes_stream()
                .map(|r| r.map_err(|_| std::io::Error::other("Upstream stream failed"))),
        )
    };
    let events = async_stream::stream! {
        let mut input=input;let mut buffer=String::new();let mut tail=Vec::new();let mut sequence=0u64;
        while let Some(chunk)=input.next().await {
            let chunk=match chunk{Ok(c)=>c,Err(_)=>{yield Err::<Bytes,std::io::Error>(std::io::Error::other("Upstream stream failed"));return;}};
            append_utf8_safe(&mut buffer,&mut tail,&chunk);
            if buffer.len()>16*1024*1024 {yield Err(std::io::Error::other("Oversized upstream SSE event"));return;}
            while let Some(block)=take_sse_block(&mut buffer) {
                match normalize_event(&block,&mut sequence,muse,&map) {
                    Ok(Some(event))=>yield Ok(Bytes::from(event)),Ok(None)=>{},Err(e)=>{yield Err(std::io::Error::other(e));return;}
                }
            }
        }
        if !buffer.trim().is_empty() || !tail.is_empty(){yield Err(std::io::Error::other("Truncated upstream SSE event"));}
    };
    (
        [
            ("content-type", "text/event-stream"),
            ("cache-control", "no-cache"),
        ],
        Body::from_stream(events),
    )
        .into_response()
}

fn normalize_event(
    block: &str,
    sequence: &mut u64,
    muse: bool,
    map: &std::collections::HashMap<
        String,
        crate::proxy::providers::transform_codex_responses_namespace::NamespacedName,
    >,
) -> Result<Option<String>, String> {
    let data = block
        .lines()
        .filter_map(|l| strip_sse_field(l, "data"))
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() {
        return Ok(Some(format!("{block}\n\n")));
    }
    if data == "[DONE]" {
        return Ok(Some("data: [DONE]\n\n".into()));
    }
    let mut event: Value =
        serde_json::from_str(&data).map_err(|_| "Malformed upstream SSE JSON")?;
    if !event.is_object() {
        return Err("Upstream SSE events must be JSON objects".into());
    }
    if muse {
        event["sequence_number"] = json!(*sequence);
        *sequence += 1;
        restore_sse_event_namespaces(&mut event, map);
    }
    let kind = block
        .lines()
        .find_map(|l| strip_sse_field(l, "event"))
        .or_else(|| event.get("type").and_then(Value::as_str));
    Ok(Some(format!(
        "{}data: {}\n\n",
        kind.map(|s| format!("event: {s}\n")).unwrap_or_default(),
        event
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auth_is_client_specific_and_required() {
        let mut h = HeaderMap::new();
        assert!(!authenticated(&h, None, ""));
        h.insert("authorization", "Bearer muse-test".parse().unwrap());
        assert!(authenticated(&h, None, "muse-test"));
        assert!(!authenticated(&h, None, "codex-test"));
        assert!(authenticated(
            &HeaderMap::new(),
            Some("key=agy-test"),
            "agy-test"
        ));
    }
    #[test]
    fn sequence_is_present_on_every_json_event_and_preserves_unicode() {
        let mut n = 0;
        let map = Default::default();
        let first=normalize_event("event: response.output_text.delta\r\ndata:{\"type\":\"response.output_text.delta\",\"delta\":\"中文\"}",&mut n,true,&map).unwrap().unwrap();
        assert!(first.contains("\"sequence_number\":0"));
        assert!(first.contains("中文"));
        let next = normalize_event(
            "data: {\"type\":\"response.completed\",\"sequence_number\":44}",
            &mut n,
            true,
            &map,
        )
        .unwrap()
        .unwrap();
        assert!(next.contains("\"sequence_number\":1"));
        assert!(normalize_event("data: invalid", &mut n, true, &map).is_err());
    }
}

#[cfg(test)]
pub(crate) mod native_tests;
