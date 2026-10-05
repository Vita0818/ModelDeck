use super::*;
use crate::proxy::{
    providers::{
        streaming, streaming_gemini, streaming_responses, transform, transform_gemini,
        transform_responses,
    },
    sse::{append_utf8_safe, take_sse_block},
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

fn error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(json!({"error":{"type":"modeldeck_error","message":message}})),
    )
        .into_response()
}

fn authenticated(headers: &HeaderMap, query: Option<&str>, token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    let header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| headers.get("x-api-key").and_then(|v| v.to_str().ok()))
        .or_else(|| headers.get("x-goog-api-key").and_then(|v| v.to_str().ok()));
    let query = query.and_then(|q| {
        url::form_urlencoded::parse(q.as_bytes())
            .find(|(k, _)| k == "key")
            .map(|(_, v)| v.into_owned())
    });
    let candidate = header.or(query.as_deref()).unwrap_or("");
    candidate.len() == token.len()
        && candidate
            .bytes()
            .zip(token.bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
}

pub async fn handle(Path((endpoint, path)): Path<(String, String)>, request: Request) -> Response {
    let store = match read_store() {
        Ok(s) => s,
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "ModelDeck resources are unavailable",
            )
        }
    };
    serve(&store, &endpoint, None, None, &path, request).await
}

pub async fn handle_client(client: &str, path: &str, request: Request) -> Response {
    let store = match read_store() {
        Ok(s) => s,
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "ModelDeck resources are unavailable",
            )
        }
    };
    let Some(binding) = store.active_bindings.get(client) else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Harness selection is not active",
        );
    };
    serve(
        &store,
        &binding.endpoint_id,
        Some(client),
        Some(binding),
        path,
        request,
    )
    .await
}

pub async fn handle_harness(
    Path((client, path)): Path<(String, String)>,
    request: Request,
) -> Response {
    handle_client(&client, &path, request).await
}

fn aliases(
    store: &DeckStore,
    endpoint: &ExposedEndpoint,
    input: Protocol,
    client: Option<&str>,
) -> Vec<String> {
    if !endpoint.protocols.contains(&input) {
        return Vec::new();
    }
    if let Some(client) = client {
        return compatible_models(store, client, endpoint)
            .into_iter()
            .filter(|alias| {
                let model = &store.models[&endpoint.routes[alias].model_id];
                protocols_compatible(input, store.providers[&model.provider_id].protocol)
            })
            .collect();
    }
    endpoint
        .routes
        .iter()
        .filter_map(|(alias, route)| {
            let model = store.models.get(&route.model_id)?;
            let provider = store.providers.get(&model.provider_id)?;
            protocols_compatible(input, provider.protocol).then(|| alias.clone())
        })
        .collect()
}

fn catalog(
    store: &DeckStore,
    endpoint: &ExposedEndpoint,
    path: &str,
    client: Option<&str>,
) -> Value {
    let input = client
        .and_then(|client| {
            store
                .active_bindings
                .get(client)
                .and_then(|b| b.input_protocol)
                .or_else(|| client_protocol(client))
        })
        .unwrap_or(if path.starts_with("v1beta/") {
            Protocol::Gemini
        } else {
            Protocol::OpenaiChat
        });
    let aliases = aliases(store, endpoint, input, client);
    if input == Protocol::Gemini {
        json!({"models":aliases.iter().map(|alias|{let model=&store.models[&endpoint.routes[alias].model_id];json!({"name":format!("models/{alias}"),"displayName":alias,"inputTokenLimit":model.context.saturating_sub(model.output),"outputTokenLimit":model.output,"supportedGenerationMethods":["generateContent","streamGenerateContent"]})}).collect::<Vec<_>>()})
    } else if client == Some("muse") {
        json!({"data":aliases.iter().enumerate().map(|(i,alias)|{let model=&store.models[&endpoint.routes[alias].model_id];json!({"id":alias,"model_id":alias,"provider_id":"meta","profile_id":alias,"display_label":alias,"visibility":"visible","release_date":"","display_order":i,"is_default":store.active_bindings.get("muse").is_some_and(|binding|binding.model==*alias),"context_limit":model.context,"output_limit":model.output})}).collect::<Vec<_>>()})
    } else {
        json!({"object":"list","data":aliases.iter().map(|alias|{let model=&store.models[&endpoint.routes[alias].model_id];json!({"id":alias,"object":"model","type":"model","owned_by":"modeldeck","name":alias,"display_name":alias,"created_at":"2026-01-01T00:00:00Z","context_length":model.context})}).collect::<Vec<_>>(),"has_more":false})
    }
}

fn contains_image(value: &Value) -> bool {
    match value {
        Value::Array(values) => values.iter().any(contains_image),
        Value::Object(values) => {
            values
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|t| matches!(t, "image" | "image_url" | "input_image"))
                || values.contains_key("inlineData")
                || values.contains_key("fileData")
                || values.values().any(contains_image)
        }
        _ => false,
    }
}

async fn serve(
    store: &DeckStore,
    endpoint_id: &str,
    client: Option<&str>,
    binding: Option<&HarnessBinding>,
    path: &str,
    request: Request,
) -> Response {
    let Some(endpoint) = store.endpoints.get(endpoint_id) else {
        return error(StatusCode::NOT_FOUND, "Unknown ModelDeck endpoint");
    };
    if !authenticated(request.headers(), request.uri().query(), &endpoint.token) {
        return error(
            StatusCode::UNAUTHORIZED,
            "Invalid ModelDeck endpoint credential",
        );
    }
    let normalized = path.trim_start_matches("v1/").trim_start_matches("v1beta/");
    if request.method() == axum::http::Method::GET
        && matches!(normalized, "models" | "muse-code/models")
    {
        return Json(catalog(store, endpoint, path, client)).into_response();
    }
    if request.method() != axum::http::Method::POST {
        return error(StatusCode::NOT_FOUND, "Unsupported endpoint operation");
    }
    let input = match normalized {
        "responses" | "responses/compact" => Protocol::OpenaiResponses,
        "chat/completions" => Protocol::OpenaiChat,
        "messages" => Protocol::Anthropic,
        s if s.starts_with("models/") => Protocol::Gemini,
        _ => return error(StatusCode::NOT_FOUND, "Unsupported endpoint operation"),
    };
    if !endpoint.protocols.contains(&input)
        || client.is_some_and(|client| !client_protocols(client).contains(&input))
        || binding
            .and_then(|binding| binding.input_protocol)
            .is_some_and(|p| p != input)
    {
        return error(
            StatusCode::BAD_REQUEST,
            "This endpoint does not expose the requested protocol",
        );
    }
    let mut headers = request.headers().clone();
    let bytes = match axum::body::to_bytes(request.into_body(), 32 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Invalid or oversized request body",
            )
        }
    };
    let bytes = match crate::proxy::decode_codex_request_body(&mut headers, bytes) {
        Ok(bytes) if bytes.len() <= 32 * 1024 * 1024 => bytes,
        _ => {
            return error(
                StatusCode::BAD_REQUEST,
                "Invalid, unsupported or oversized compressed request",
            )
        }
    };
    let mut body: Value = match serde_json::from_slice(&bytes) {
        Ok(Value::Object(v)) => Value::Object(v),
        _ => {
            return error(
                StatusCode::BAD_REQUEST,
                "Inference requests must be JSON objects",
            )
        }
    };
    let mut alias = if input == Protocol::Gemini {
        normalized
            .strip_prefix("models/")
            .and_then(|s| s.rsplit_once(':'))
            .map(|(model, _)| model.to_string())
            .unwrap_or_default()
    } else {
        body.get("model")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| binding.map(|b| b.model.clone()))
            .unwrap_or_default()
    };
    if client == Some("antigravity")
        && alias == "gemini-3.1-flash-lite-preview"
        && !endpoint.routes.contains_key(&alias)
    {
        if let Some(binding) = binding {
            alias = if binding.utility_model.is_empty() {
                binding.model.clone()
            } else {
                binding.utility_model.clone()
            };
        }
    }
    if client == Some("claude-desktop") {
        alias = alias.trim_end_matches("[1m]").to_string();
        if !endpoint.routes.contains_key(&alias)
            && crate::claude_desktop_config::is_claude_safe_model_id(&alias)
        {
            if let Some(binding) = binding {
                alias = if alias.contains("haiku") && !binding.utility_model.is_empty() {
                    binding.utility_model.clone()
                } else {
                    binding.model.clone()
                };
            }
        }
    }
    if !aliases(store, endpoint, input, client).contains(&alias) {
        return error(
            StatusCode::BAD_REQUEST,
            "The requested model is unavailable on this endpoint and protocol",
        );
    }
    let route = &endpoint.routes[&alias];
    let Some(model) = store.models.get(&route.model_id) else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "The model resource is unavailable",
        );
    };
    let Some(provider) = store.providers.get(&model.provider_id) else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "The provider resource is unavailable",
        );
    };
    if normalized == "responses/compact" && provider.protocol != Protocol::OpenaiResponses {
        return error(
            StatusCode::NOT_IMPLEMENTED,
            "Remote compaction requires a native Responses upstream",
        );
    }
    if contains_image(&body) && !model.images {
        return error(
            StatusCode::BAD_REQUEST,
            "This model resource does not accept images",
        );
    }
    if body
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|v| !v.is_empty())
        && !model.tools
    {
        return error(
            StatusCode::BAD_REQUEST,
            "This model resource does not support tools",
        );
    }
    let key = match resolve_key(provider) {
        Ok(k) => k,
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "The upstream credential is unavailable; check ModelDeck resource settings",
            )
        }
    };
    body["model"] = json!(model.upstream_id);
    for field in ["max_tokens", "max_output_tokens", "max_completion_tokens"] {
        if let Some(n) = body.get(field).and_then(Value::as_u64) {
            body[field] = json!(n.min(model.output));
        }
    }
    // Tier choice belongs to the resource route. An empty choice means the
    // upstream default, rather than a harness's independently stored effort.
    if let Some(object) = body.as_object_mut() {
        object.remove("reasoning_effort");
        object.remove("thinking");
    }
    if let Some(reasoning) = body.get_mut("reasoning").and_then(Value::as_object_mut) {
        reasoning.remove("effort");
    }
    if let Some(config) = body.get_mut("output_config").and_then(Value::as_object_mut) {
        config.remove("effort");
    }
    if let Some(config) = body
        .get_mut("generationConfig")
        .and_then(Value::as_object_mut)
    {
        config.remove("thinkingConfig");
    }
    if model.reasoning_supported == Some(false) {
        if let Some(o) = body.as_object_mut() {
            o.remove("reasoning");
            o.remove("reasoning_effort");
            o.remove("thinking");
        }
    } else if !route.effort.is_empty() {
        match input {
            Protocol::OpenaiChat => body["reasoning_effort"] = json!(route.effort),
            Protocol::OpenaiResponses => {
                if !body["reasoning"].is_object() {
                    body["reasoning"] = json!({});
                }
                body["reasoning"]["effort"] = json!(route.effort);
            }
            Protocol::Anthropic => {
                let budget =
                    crate::proxy::providers::transform_codex_anthropic::effort_to_thinking_budget(
                        &route.effort,
                    )
                    .unwrap_or(0)
                    .min(model.output.saturating_sub(1));
                if budget >= 1024 {
                    body["thinking"] = json!({"type":"enabled","budget_tokens":budget});
                } else {
                    body.as_object_mut().unwrap().remove("thinking");
                }
            }
            Protocol::Gemini => {}
        }
    }
    let profile = ClientProfile {
        connection_name: String::new(),
        endpoint: provider.base_url.clone(),
        protocol: provider.protocol.as_str().into(),
        api_key: String::new(),
        key_env: String::new(),
        models: vec![ClientModel {
            id: model.upstream_id.clone(),
            name: alias.clone(),
            context: model.context,
            output: model.output,
            reasoning: model.reasoning.clone(),
            reasoning_supported: model.reasoning_supported,
        }],
        default_model: model.upstream_id.clone(),
        utility_model: String::new(),
        effort: route.effort.clone(),
        executable: String::new(),
        enabled: true,
        local_token: String::new(),
    };
    let http = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(1800))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Cannot initialize upstream transport",
            )
        }
    };
    let response = match input {
        Protocol::OpenaiResponses => {
            crate::model_clients::gateway::forward(&http, &profile, &key, normalized, body, true)
                .await
        }
        Protocol::Gemini => {
            body.as_object_mut().unwrap().remove("model");
            let method = normalized.rsplit_once(':').map(|(_, m)| m).unwrap_or("");
            let path = format!("models/{}:{method}", model.upstream_id);
            crate::model_clients::gateway::gemini::handle(&http, &profile, &key, &path, body).await
        }
        Protocol::Anthropic | Protocol::OpenaiChat => {
            forward_standard(&http, &profile, &key, input, body).await
        }
    };
    public_response(response, &model.upstream_id, &alias).await
}

async fn forward_standard(
    http: &reqwest::Client,
    p: &ClientProfile,
    key: &str,
    input: Protocol,
    mut body: Value,
) -> Response {
    let stream = body.get("stream").and_then(Value::as_bool).unwrap_or(false);
    let base = p.endpoint.trim_end_matches('/');
    let upstream = match p.protocol.as_str() {
        "anthropic" => Protocol::Anthropic,
        "openai-chat" => Protocol::OpenaiChat,
        "openai-responses" => Protocol::OpenaiResponses,
        "gemini" => Protocol::Gemini,
        _ => return error(StatusCode::BAD_REQUEST, "Unsupported upstream protocol"),
    };
    let limit = p.models[0].output;
    let hints = transform_gemini::extract_anthropic_tool_schema_hints(&body);
    let url = match (input, upstream) {
        (Protocol::Anthropic, Protocol::OpenaiChat) => {
            body = match transform::anthropic_to_openai(body) {
                Ok(b) => b,
                Err(_) => {
                    return error(
                        StatusCode::BAD_REQUEST,
                        "This Messages request cannot be represented as Chat Completions",
                    )
                }
            };
            if !p.effort.is_empty() {
                body["reasoning_effort"] = json!(p.effort);
            }
            format!("{base}/chat/completions")
        }
        (Protocol::Anthropic, Protocol::OpenaiResponses) => {
            body = match transform_responses::anthropic_to_responses(body, None, false, false) {
                Ok(b) => b,
                Err(_) => {
                    return error(
                        StatusCode::BAD_REQUEST,
                        "This Messages request cannot be represented as Responses",
                    )
                }
            };
            if !p.effort.is_empty() {
                if !body["reasoning"].is_object() {
                    body["reasoning"] = json!({});
                }
                body["reasoning"]["effort"] = json!(p.effort);
            }
            body["store"] = json!(false);
            format!("{base}/responses")
        }
        (Protocol::Anthropic, Protocol::Gemini) => {
            body = match transform_gemini::anthropic_to_gemini(body) {
                Ok(b) => b,
                Err(_) => {
                    return error(
                        StatusCode::BAD_REQUEST,
                        "This Messages request cannot be represented as Gemini",
                    )
                }
            };
            if !p.effort.is_empty() {
                if !body["generationConfig"].is_object() {
                    body["generationConfig"] = json!({});
                }
                body["generationConfig"]["thinkingConfig"] =
                    json!({"thinkingLevel":p.effort.to_uppercase()});
            }
            format!(
                "{base}/models/{}:{}{}",
                p.default_model,
                if stream {
                    "streamGenerateContent"
                } else {
                    "generateContent"
                },
                if stream { "?alt=sse" } else { "" }
            )
        }
        (_, Protocol::Anthropic) => {
            body["max_tokens"] = json!(body
                .get("max_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(limit)
                .min(limit));
            format!(
                "{base}/{}",
                if base.ends_with("/v1") {
                    "messages"
                } else {
                    "v1/messages"
                }
            )
        }
        (_, Protocol::OpenaiChat) => format!("{base}/chat/completions"),
        _ => return error(StatusCode::BAD_REQUEST, "Unsupported protocol combination"),
    };
    let mut req = http
        .post(url)
        .header("accept-encoding", "identity")
        .json(&body);
    req = match upstream {
        Protocol::Anthropic => req
            .header("anthropic-version", "2023-06-01")
            .header("x-api-key", key),
        Protocol::Gemini => req.header("x-goog-api-key", key),
        _ => req.bearer_auth(key),
    };
    let upstream_response = match req.send().await {
        Ok(r) => r,
        Err(_) => return error(StatusCode::BAD_GATEWAY, "Upstream connection failed"),
    };
    if !upstream_response.status().is_success() {
        return error(
            upstream_response.status(),
            &format!(
                "Upstream returned HTTP {}",
                upstream_response.status().as_u16()
            ),
        );
    }
    if !stream {
        let data: Value = match upstream_response.json().await {
            Ok(v) => v,
            Err(_) => return error(StatusCode::BAD_GATEWAY, "Invalid upstream JSON"),
        };
        let converted = match (input, upstream) {
            (Protocol::Anthropic, Protocol::OpenaiChat) => transform::openai_to_anthropic(data),
            (Protocol::Anthropic, Protocol::OpenaiResponses) => {
                transform_responses::responses_to_anthropic(data)
            }
            (Protocol::Anthropic, Protocol::Gemini) => transform_gemini::gemini_to_anthropic(data),
            _ => Ok(data),
        };
        return match converted {
            Ok(v) => Json(v).into_response(),
            Err(_) => error(
                StatusCode::BAD_GATEWAY,
                "Invalid upstream protocol response",
            ),
        };
    }
    let bytes = upstream_response.bytes_stream();
    let stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<Bytes, std::io::Error>> + Send>,
    > = match (input, upstream) {
        (Protocol::Anthropic, Protocol::OpenaiChat) => {
            Box::pin(streaming::create_anthropic_sse_stream(bytes))
        }
        (Protocol::Anthropic, Protocol::OpenaiResponses) => {
            Box::pin(streaming_responses::create_anthropic_sse_stream_from_responses(bytes))
        }
        (Protocol::Anthropic, Protocol::Gemini) => {
            Box::pin(streaming_gemini::create_anthropic_sse_stream_from_gemini(
                bytes,
                None,
                None,
                None,
                Some(hints),
            ))
        }
        _ => {
            Box::pin(bytes.map(|b| b.map_err(|_| std::io::Error::other("Upstream stream failed"))))
        }
    };
    (
        [
            ("content-type", "text/event-stream"),
            ("cache-control", "no-cache"),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}

fn hide_model(value: &mut Value, real: &str, alias: &str) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if matches!(
                    key.as_str(),
                    "model" | "model_id" | "modelVersion" | "model_version"
                ) && value.as_str() == Some(real)
                {
                    *value = json!(alias);
                } else {
                    hide_model(value, real, alias);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                hide_model(value, real, alias);
            }
        }
        _ => {}
    }
}

async fn public_response(response: Response, real: &str, alias: &str) -> Response {
    if !response.status().is_success() {
        return response;
    }
    let stream = response
        .headers()
        .get("content-type")
        .and_then(|s| s.to_str().ok())
        .is_some_and(|s| s.contains("text/event-stream"));
    let (mut parts, body) = response.into_parts();
    parts.headers.remove("content-length");
    if !stream {
        let bytes = match axum::body::to_bytes(body, 32 * 1024 * 1024).await {
            Ok(b) => b,
            Err(_) => return error(StatusCode::BAD_GATEWAY, "Invalid upstream body"),
        };
        let mut value: Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => return error(StatusCode::BAD_GATEWAY, "Invalid upstream JSON"),
        };
        hide_model(&mut value, real, alias);
        return Response::from_parts(parts, Body::from(value.to_string()));
    }
    let real = real.to_string();
    let alias = alias.to_string();
    let events = async_stream::stream! {
        let mut input=body.into_data_stream();let mut buffer=String::new();let mut tail=Vec::new();
        while let Some(chunk)=input.next().await{
            let chunk=match chunk{Ok(c)=>c,Err(_)=>{yield Err::<Bytes,std::io::Error>(std::io::Error::other("Upstream stream failed"));return;}};
            append_utf8_safe(&mut buffer,&mut tail,&chunk);
            if buffer.len()>16*1024*1024{yield Err(std::io::Error::other("Oversized SSE event"));return;}
            while let Some(block)=take_sse_block(&mut buffer){
                let mut out=String::new();
                let payload=block.lines().filter_map(|line|line.strip_prefix("data:").map(|s|s.strip_prefix(' ').unwrap_or(s))).collect::<Vec<_>>().join("\n");
                if let Ok(mut value)=serde_json::from_str::<Value>(&payload){
                    hide_model(&mut value,&real,&alias);
                    for line in block.lines().filter(|line|!line.starts_with("data:")){out.push_str(line);out.push('\n');}
                    out.push_str("data: ");out.push_str(&value.to_string());out.push('\n');
                } else {out.push_str(&block);if !block.ends_with('\n'){out.push('\n');}}
                out.push('\n');yield Ok(Bytes::from(out));
            }
        }
        if !buffer.trim().is_empty()||!tail.is_empty(){yield Err(std::io::Error::other("Truncated upstream SSE event"));}
    };
    Response::from_parts(parts, Body::from_stream(events))
}

#[cfg(test)]
mod tests;
