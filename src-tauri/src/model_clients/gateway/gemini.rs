use super::{error, ClientProfile};
use crate::proxy::sse::{append_utf8_safe, strip_sse_field, take_sse_block};
use axum::{
    body::{Body, Bytes},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use futures::StreamExt;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, VecDeque};

fn schema(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        if k == "type" {
                            v.as_str()
                                .map(|s| json!(s.to_lowercase()))
                                .unwrap_or_else(|| v.clone())
                        } else {
                            schema(v)
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(schema).collect()),
        _ => v.clone(),
    }
}

fn to_chat(p: &ClientProfile, model: &str, b: &Value, stream: bool) -> Result<Value, String> {
    let m = p
        .models
        .iter()
        .find(|m| m.id == model)
        .ok_or("Model is not in this client's catalog")?;
    if b.get("cachedContent").is_some() {
        return Err("Gemini cachedContent is not available on a Chat endpoint".into());
    }
    let mut messages = Vec::new();
    let mut pending: HashMap<String, VecDeque<String>> = HashMap::new();
    let mut serial = 0;
    if let Some(parts) = b
        .pointer("/systemInstruction/parts")
        .or_else(|| b.pointer("/system_instruction/parts"))
        .and_then(Value::as_array)
    {
        let text = parts
            .iter()
            .filter_map(|x| x.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        messages.push(json!({"role":"system","content":text}));
    }
    for c in b
        .get("contents")
        .and_then(Value::as_array)
        .ok_or("Gemini contents is required")?
    {
        let role = if c.get("role").and_then(Value::as_str) == Some("model") {
            "assistant"
        } else {
            "user"
        };
        let mut parts = Vec::new();
        let mut calls = Vec::new();
        let mut results = Vec::new();
        for part in c
            .get("parts")
            .and_then(Value::as_array)
            .ok_or("Invalid Gemini content parts")?
        {
            if part.get("thought").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                parts.push(json!({"type":"text","text":text}));
            } else if let Some(image) = part.get("inlineData").or_else(|| part.get("inline_data")) {
                let mime = image
                    .get("mimeType")
                    .or_else(|| image.get("mime_type"))
                    .and_then(Value::as_str)
                    .ok_or("Missing media MIME type")?;
                if !mime.starts_with("image/") {
                    return Err("This Chat bridge supports inline images; use a Gemini endpoint for audio/video".into());
                }
                let data = image
                    .get("data")
                    .and_then(Value::as_str)
                    .ok_or("Missing image data")?;
                parts.push(json!({"type":"image_url","image_url":{"url":format!("data:{mime};base64,{data}")}}));
            } else if let Some(call) = part
                .get("functionCall")
                .or_else(|| part.get("function_call"))
            {
                let name = call
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or("Missing function name")?;
                serial += 1;
                let id = call
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("agy_{serial}"));
                pending
                    .entry(name.into())
                    .or_default()
                    .push_back(id.clone());
                calls.push(json!({"id":id,"type":"function","function":{"name":name,"arguments":call.get("args").cloned().unwrap_or(json!({})).to_string()}}));
            } else if let Some(result) = part
                .get("functionResponse")
                .or_else(|| part.get("function_response"))
            {
                let name = result
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or("Missing function response name")?;
                let queued = pending.get_mut(name).and_then(VecDeque::pop_front);
                let id = result
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .or(queued)
                    .ok_or("Function response has no matching call")?;
                results.push(json!({"role":"tool","tool_call_id":id,"content":result.get("response").cloned().unwrap_or(json!({})).to_string()}));
            } else {
                return Err("Unsupported Gemini content part; use a native Gemini endpoint".into());
            }
        }
        if !parts.is_empty() || !calls.is_empty() {
            let mut message =
                json!({"role":role,"content":if parts.is_empty(){Value::Null}else{json!(parts)}});
            if !calls.is_empty() {
                message["tool_calls"] = json!(calls);
            }
            messages.push(message);
        }
        messages.extend(results);
    }
    let mut out = json!({"model":model,"messages":messages,"stream":stream});
    if stream {
        out["stream_options"] = json!({"include_usage":true});
    }
    if let Some(tools) = b.get("tools").and_then(Value::as_array) {
        let mut functions = Vec::new();
        for tool in tools {
            let declarations = tool
                .get("functionDeclarations")
                .or_else(|| tool.get("function_declarations"))
                .and_then(Value::as_array)
                .ok_or("Gemini built-in tools require a native Gemini endpoint")?;
            for f in declarations {
                functions.push(json!({"type":"function","function":{"name":f["name"],"description":f.get("description").cloned().unwrap_or(json!("")),"parameters":schema(f.get("parameters").or_else(||f.get("parametersJsonSchema")).unwrap_or(&json!({"type":"object","properties":{}})))}}));
            }
        }
        if !functions.is_empty() {
            out["tools"] = json!(functions);
        }
    }
    if let Some(config) = b
        .get("generationConfig")
        .or_else(|| b.get("generation_config"))
    {
        for (from, to) in [
            ("temperature", "temperature"),
            ("topP", "top_p"),
            ("stopSequences", "stop"),
        ] {
            if let Some(v) = config.get(from) {
                out[to] = v.clone();
            }
        }
        let tokens = config
            .get("maxOutputTokens")
            .and_then(Value::as_u64)
            .unwrap_or(m.output);
        out["max_tokens"] = json!(tokens.min(m.output));
        if let Some(format) = config.get("responseMimeType").and_then(Value::as_str) {
            if format == "application/json" {
                out["response_format"] = json!({"type":"json_object"});
            }
        }
    } else {
        out["max_tokens"] = json!(m.output);
    }
    if let Some(mode) = b
        .pointer("/toolConfig/functionCallingConfig/mode")
        .and_then(Value::as_str)
    {
        out["tool_choice"] = json!(match mode {
            "NONE" => "none",
            "ANY" => "required",
            _ => "auto",
        });
    }
    let thinking = b
        .pointer("/generationConfig/thinkingConfig")
        .or_else(|| b.pointer("/generation_config/thinking_config"));
    let mut reasoning = serde_json::Map::new();
    if let Some(thinking) = thinking {
        if let Some(level) = thinking
            .get("thinkingLevel")
            .or_else(|| thinking.get("thinking_level"))
        {
            reasoning.insert(
                "effort".into(),
                level
                    .as_str()
                    .map(|s| json!(s.to_ascii_lowercase()))
                    .unwrap_or_else(|| level.clone()),
            );
        }
        if let Some(budget) = thinking
            .get("thinkingBudget")
            .or_else(|| thinking.get("thinking_budget"))
        {
            match budget.as_i64() {
                Some(0) => {
                    reasoning.insert("enabled".into(), json!(false));
                }
                Some(-1) => {
                    reasoning.insert("enabled".into(), json!(true));
                }
                _ => {
                    reasoning.insert("max_tokens".into(), budget.clone());
                }
            }
        }
        if let Some(include) = thinking
            .get("includeThoughts")
            .or_else(|| thinking.get("include_thoughts"))
            .and_then(Value::as_bool)
        {
            reasoning.insert("exclude".into(), json!(!include));
        }
    } else if !p.effort.is_empty() {
        reasoning.insert("effort".into(), json!(p.effort));
    }
    if !reasoning.is_empty() {
        let openrouter = url::Url::parse(&p.endpoint)
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
            .as_deref()
            == Some("openrouter.ai");
        if !openrouter {
            if let Some(effort) = reasoning.remove("effort") {
                out["reasoning_effort"] = effort;
            }
        }
        if !reasoning.is_empty() {
            out["reasoning"] = Value::Object(reasoning);
        }
    }
    let provider = crate::model_clients::provider_adapter::project(p, "");
    crate::model_clients::provider_adapter::adapt_chat(&mut out, &provider);
    Ok(out)
}

#[derive(Default)]
struct ChatStream {
    calls: BTreeMap<u64, (String, String, String)>,
}
impl ChatStream {
    fn event(&mut self, v: &Value) -> Result<Value, String> {
        if v.get("error").is_some() {
            return Err("Upstream returned a stream error".into());
        }
        let mut candidates = Vec::new();
        for choice in v
            .get("choices")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let delta = choice
                .get("delta")
                .or_else(|| choice.get("message"))
                .unwrap_or(&Value::Null);
            let mut parts = Vec::new();
            if let Some(t) = delta.get("content").and_then(Value::as_str) {
                if !t.is_empty() {
                    parts.push(json!({"text":t}));
                }
            }
            if let Some(t) = delta
                .get("reasoning_content")
                .or_else(|| delta.get("reasoning"))
                .and_then(Value::as_str)
            {
                if !t.is_empty() {
                    parts.push(json!({"text":t,"thought":true}));
                }
            }
            for call in delta
                .get("tool_calls")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let index = call
                    .get("index")
                    .and_then(Value::as_u64)
                    .unwrap_or(self.calls.len() as u64);
                let entry = self.calls.entry(index).or_default();
                if let Some(id) = call.get("id").and_then(Value::as_str) {
                    entry.0 = id.into();
                }
                if let Some(name) = call.pointer("/function/name").and_then(Value::as_str) {
                    entry.1.push_str(name);
                }
                if let Some(args) = call.pointer("/function/arguments").and_then(Value::as_str) {
                    entry.2.push_str(args);
                }
            }
            let finish = choice.get("finish_reason").and_then(Value::as_str);
            if finish.is_some() {
                for (_, (id, name, args)) in std::mem::take(&mut self.calls) {
                    let args: Value = if args.is_empty() {
                        json!({})
                    } else {
                        serde_json::from_str(&args)
                            .map_err(|_| "Invalid streamed function arguments")?
                    };
                    parts.push(json!({"functionCall":{"id":id,"name":name,"args":args}}));
                }
            }
            let mut candidate = json!({"index":choice.get("index").and_then(Value::as_u64).unwrap_or(0),"content":{"role":"model","parts":parts}});
            if let Some(reason) = finish {
                candidate["finishReason"] = json!(match reason {
                    "length" => "MAX_TOKENS",
                    "content_filter" => "SAFETY",
                    _ => "STOP",
                });
            }
            candidates.push(candidate);
        }
        let mut out = json!({"candidates":candidates});
        if let Some(u) = v.get("usage").filter(|v| !v.is_null()) {
            out["usageMetadata"] = json!({"promptTokenCount":u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0),"candidatesTokenCount":u.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0),"totalTokenCount":u.get("total_tokens").and_then(Value::as_u64).unwrap_or(0)});
        }
        Ok(out)
    }
}

pub(crate) async fn handle(
    http: &reqwest::Client,
    p: &ClientProfile,
    key: &str,
    path: &str,
    body: Value,
) -> Response {
    let Some((model, method)) = path
        .strip_prefix("models/")
        .and_then(|s| s.rsplit_once(':'))
    else {
        return error(StatusCode::NOT_FOUND, "Invalid Gemini model endpoint");
    };
    // Antigravity uses this fixed native ID for title/background requests even
    // when a custom model is selected. Route that documented client behavior
    // through the user's auxiliary-model binding, never a different account.
    let model =
        if model == "gemini-3.1-flash-lite-preview" && !p.models.iter().any(|m| m.id == model) {
            if p.utility_model.is_empty() {
                p.default_model.as_str()
            } else {
                p.utility_model.as_str()
            }
        } else {
            model
        };
    if !p.models.iter().any(|m| m.id == model) {
        return error(
            StatusCode::BAD_REQUEST,
            "Model is not in this client's catalog",
        );
    }
    let stream = method == "streamGenerateContent";
    if !matches!(method, "generateContent" | "streamGenerateContent") {
        return error(
            StatusCode::NOT_IMPLEMENTED,
            "This operation is not available on the configured bridge",
        );
    }
    if p.protocol == "gemini" {
        let mut body = body;
        if !body.is_object() {
            return error(
                StatusCode::BAD_REQUEST,
                "Inference requests must be JSON objects",
            );
        }
        let selected = p
            .models
            .iter()
            .find(|m| m.id == model)
            .expect("validated model");
        if body.get("generationConfig").is_none() {
            body["generationConfig"] = json!({});
        }
        if !body["generationConfig"].is_object() {
            return error(StatusCode::BAD_REQUEST, "Invalid Gemini generationConfig");
        }
        let requested = body["generationConfig"]
            .get("maxOutputTokens")
            .and_then(Value::as_u64)
            .unwrap_or(selected.output);
        body["generationConfig"]["maxOutputTokens"] = json!(requested.min(selected.output));
        if body.pointer("/generationConfig/thinkingConfig").is_none()
            && body.pointer("/generation_config/thinking_config").is_none()
            && !p.effort.is_empty()
        {
            body["generationConfig"]["thinkingConfig"] =
                json!({"thinkingLevel":p.effort.to_uppercase()});
        }
        let url = crate::model_clients::provider_adapter::url(
            p,
            &format!(
                "models/{model}:{method}{}",
                if stream { "?alt=sse" } else { "" }
            ),
        );
        let request = http
            .post(url)
            .header("accept-encoding", "identity")
            .json(&body);
        let request = match crate::model_clients::provider_adapter::authenticate(request, p, key) {
            Ok(request) => request,
            Err(_) => {
                return error(
                    StatusCode::BAD_REQUEST,
                    "Invalid upstream authentication configuration",
                )
            }
        };
        let upstream = match request.send().await {
            Ok(r) => r,
            Err(_) => return error(StatusCode::BAD_GATEWAY, "Gemini upstream connection failed"),
        };
        if !upstream.status().is_success() {
            return error(upstream.status(), "Gemini upstream rejected the request");
        }
        let content = if stream {
            "text/event-stream"
        } else {
            "application/json"
        };
        return (
            [("content-type", content)],
            Body::from_stream(upstream.bytes_stream()),
        )
            .into_response();
    }
    let converted = match to_chat(p, model, &body, stream) {
        Ok(v) => v,
        Err(e) => return error(StatusCode::BAD_REQUEST, &e),
    };
    let request = http
        .post(crate::model_clients::provider_adapter::url(
            p,
            "chat/completions",
        ))
        .json(&converted);
    let req = match crate::model_clients::provider_adapter::authenticate(request, p, key) {
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
        Err(_) => return error(StatusCode::BAD_GATEWAY, "Chat upstream connection failed"),
    };
    if !upstream.status().is_success() {
        return error(upstream.status(), "Chat upstream rejected the request");
    }
    if !stream {
        let data: Value = match upstream.json().await {
            Ok(v) => v,
            Err(_) => return error(StatusCode::BAD_GATEWAY, "Invalid upstream JSON"),
        };
        return match ChatStream::default().event(&data) {
            Ok(v) => Json(v).into_response(),
            Err(e) => error(StatusCode::BAD_GATEWAY, &e),
        };
    }
    let stream = async_stream::stream! {
        let mut input=upstream.bytes_stream();let mut buffer=String::new();let mut tail=Vec::new();let mut state=ChatStream::default();
        while let Some(chunk)=input.next().await {
            let chunk=match chunk {Ok(c)=>c,Err(_)=>{yield Err::<Bytes,std::io::Error>(std::io::Error::other("Chat stream failed"));return;}};
            append_utf8_safe(&mut buffer,&mut tail,&chunk);
            if buffer.len()>16*1024*1024 {yield Err(std::io::Error::other("Oversized Chat event"));return;}
            while let Some(block)=take_sse_block(&mut buffer) {
                let data=block.lines().filter_map(|l|strip_sse_field(l,"data")).collect::<Vec<_>>().join("\n");
                if data.is_empty() { continue; }
                if data=="[DONE]" { return; }
                let event=serde_json::from_str::<Value>(&data).map_err(|_|"Malformed Chat SSE JSON".to_string()).and_then(|v|state.event(&v));
                match event{Ok(v)=>yield Ok(Bytes::from(format!("data: {v}\n\n"))),Err(e)=>{yield Err(std::io::Error::other(e));return;}}
            }
        }
        if !buffer.trim().is_empty() || !tail.is_empty(){yield Err(std::io::Error::other("Truncated Chat SSE event"));}
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

#[cfg(test)]
mod tests {
    use super::*;
    fn p() -> ClientProfile {
        serde_json::from_value(json!({"endpoint":"https://openrouter.ai/api/v1","protocol":"openai-chat","models":[{"id":"x/model","name":"Model","context":1000000,"output":32000,"reasoning":["max"]}],"defaultModel":"x/model","effort":"max"})).unwrap()
    }
    #[test]
    fn preserves_tool_round_trip_and_independent_limits() {
        let b = json!({"contents":[{"role":"model","parts":[{"functionCall":{"name":"read_file","args":{"path":"x"}}}]},{"role":"user","parts":[{"functionResponse":{"name":"read_file","response":{"content":"hello"}}}]}],"tools":[{"functionDeclarations":[{"name":"read_file","parameters":{"type":"OBJECT","properties":{"path":{"type":"STRING"}}}}]}]});
        let o = to_chat(&p(), "x/model", &b, true).unwrap();
        assert_eq!(
            o["messages"][0]["tool_calls"][0]["id"],
            o["messages"][1]["tool_call_id"]
        );
        assert_eq!(o["tools"][0]["function"]["parameters"]["type"], "object");
        assert_eq!(o["reasoning"]["effort"], "max");
        assert_eq!(o["max_tokens"], 32000);
    }

    #[test]
    fn native_budget_and_disable_controls_do_not_become_a_tier() {
        for (thinking, expected) in [
            (json!({"thinkingBudget":2048}), json!({"max_tokens":2048})),
            (json!({"thinkingBudget":0}), json!({"enabled":false})),
            (json!({"thinkingBudget":-1}), json!({"enabled":true})),
            (
                json!({"thinkingLevel":"XHIGH","includeThoughts":false}),
                json!({"effort":"xhigh","exclude":true}),
            ),
        ] {
            let body = json!({"contents":[],"generationConfig":{"thinkingConfig":thinking}});
            let output = to_chat(&p(), "x/model", &body, false).unwrap();
            assert_eq!(output["reasoning"], expected);
        }
    }
    #[test]
    fn accumulates_fragmented_calls_and_usage() {
        let mut s = ChatStream::default();
        s.event(&json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call1","function":{"name":"read","arguments":"{\"path\":"}}]}}]})).unwrap();
        let o=s.event(&json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"x\"}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":12,"completion_tokens":4,"total_tokens":16}})).unwrap();
        assert_eq!(
            o["candidates"][0]["content"]["parts"][0]["functionCall"]["args"]["path"],
            "x"
        );
        assert_eq!(o["usageMetadata"]["totalTokenCount"], 16);
    }
}
