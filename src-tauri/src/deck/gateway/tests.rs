use super::*;

use axum::{routing::post, Router};
use std::sync::{Arc, Mutex};

#[test]
fn native_opencode_session_context_survives_without_forwarding_local_credentials() {
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", "opencode/verified".parse().unwrap());
    for name in [
        "x-opencode-session-id",
        "x-session-affinity",
        "x-session-id",
    ] {
        headers.insert(name, "native-session".parse().unwrap());
    }
    headers.insert("authorization", "Bearer gateway-only".parse().unwrap());
    headers.insert("x-api-key", "gateway-only".parse().unwrap());
    headers.insert("cookie", "private-cookie".parse().unwrap());
    let context = upstream_context_headers(&headers, Some("opencode"));
    for name in [
        "x-opencode-session-id",
        "x-session-affinity",
        "x-session-id",
    ] {
        assert_eq!(context[name], headers[name]);
    }
    for name in ["authorization", "x-api-key", "cookie"] {
        assert!(!context.contains_key(name));
    }
    assert!(
        !upstream_context_headers(&HeaderMap::new(), Some("opencode"))
            .contains_key("x-opencode-session-id")
    );
}

#[test]
fn native_claude_and_codex_session_identity_is_preserved_without_synthesizing_a_mode() {
    let mut headers = HeaderMap::new();
    for (name, value) in [
        ("x-claude-code-session-id", "native-claude-session"),
        ("x-app", "cli"),
        ("session-id", "native-codex-session"),
        ("session_id", "native-codex-session"),
        ("thread-id", "native-codex-thread"),
        ("x-client-request-id", "native-request"),
        ("originator", "Codex Desktop"),
    ] {
        headers.insert(name, value.parse().unwrap());
    }
    headers.insert(
        "x-codex-turn-metadata",
        "private-device-data".parse().unwrap(),
    );
    headers.insert("authorization", "Bearer gateway-only".parse().unwrap());
    for client in ["claude", "codex"] {
        let context = upstream_context_headers(&headers, Some(client));
        for name in [
            "x-claude-code-session-id",
            "x-app",
            "session-id",
            "session_id",
            "thread-id",
            "x-client-request-id",
            "originator",
        ] {
            assert_eq!(context[name], headers[name]);
        }
        assert!(!context.contains_key("authorization"));
        assert!(!context.contains_key("x-codex-turn-metadata"));
        assert!(upstream_context_headers(&HeaderMap::new(), Some(client)).is_empty());
    }
}

#[test]
fn cursor_local_attribution_preserves_native_identity_and_private_headers() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "user-agent",
        "Cursor-CLI/verified (darwin arm64)".parse().unwrap(),
    );
    headers.insert("authorization", "Bearer local-only-secret".parse().unwrap());
    headers.insert("cookie", "private-session".parse().unwrap());
    let context = upstream_context_headers(&headers, Some("cursor-agent"));
    assert_eq!(context["user-agent"], headers["user-agent"]);
    assert_eq!(context["http-referer"], "https://cursor.com");
    assert_eq!(context["x-openrouter-categories"], "cli-agent");
    assert!(!context.contains_key("authorization"));
    assert!(!context.contains_key("cookie"));
    assert!(!upstream_context_headers(&headers, Some("muse")).contains_key("http-referer"));
    headers.insert(
        "http-referer",
        "https://my-explicit-app.example".parse().unwrap(),
    );
    assert_eq!(
        upstream_context_headers(&headers, Some("cursor-agent"))["http-referer"],
        "https://my-explicit-app.example"
    );
}

#[test]
fn cursor_catalog_describes_only_compatible_api_and_model_capabilities() {
    let store = crate::deck::tests::fixture();
    let result = catalog(
        &store,
        &store.endpoints["entry"],
        "models",
        Some("cursor-agent"),
    );
    let model = &result["data"][0];
    assert_eq!(model["id"], "coding");
    assert_eq!(model["api_types"], json!(["chat_completions"]));
    assert_eq!(model["capabilities"]["context_length"], 200000);
    assert_eq!(model["capabilities"]["max_output_tokens"], 8192);
    assert_eq!(model["capabilities"]["supports_tool_use"], true);
    assert!(!result.to_string().contains("upstream-only"));
    assert!(!result.to_string().contains("real-model"));
}

async fn mock_upstream() -> (
    String,
    Arc<Mutex<Vec<(HeaderMap, Value)>>>,
    tokio::task::JoinHandle<()>,
) {
    let records = Arc::new(Mutex::new(Vec::new()));
    let captured = records.clone();
    let router=Router::new().route("/v1/chat/completions",post(move |headers:HeaderMap,Json(body):Json<Value>|{
        let records=captured.clone();async move {
            records.lock().unwrap().push((headers,body.clone()));
            if body.get("stream").and_then(Value::as_bool)==Some(true) {
                let a=format!("data: {}\n\n",json!({"id":"chatcmpl-test","object":"chat.completion.chunk","model":body["model"],"choices":[{"index":0,"delta":{"role":"assistant","content":"hello"},"finish_reason":null}]}));
                let b=format!("data: {}\n\ndata: [DONE]\n\n",json!({"id":"chatcmpl-test","model":body["model"],"choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}));
                ([("content-type","text/event-stream")],Body::from(format!("{a}{b}"))).into_response()
            } else {
                Json(json!({"id":"chatcmpl-test","object":"chat.completion","model":body["model"],"choices":[{"index":0,"message":{"role":"assistant","content":"hello"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}})).into_response()
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (url, records, task)
}

fn request(path: &str, body: Value, token: &str) -> Request {
    Request::builder()
        .method("POST")
        .uri(format!("/deck/entry/{path}"))
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn native_tier_choices_survive_all_protocol_bridges() {
    let (url, records, task) = mock_upstream().await;
    let mut store = crate::deck::tests::fixture();
    store.providers.get_mut("upstream").unwrap().base_url = url;
    store.models.get_mut("resource").unwrap().reasoning = ["low", "medium", "high", "xhigh", "max"]
        .map(str::to_string)
        .to_vec();
    store
        .endpoints
        .get_mut("entry")
        .unwrap()
        .routes
        .get_mut("coding")
        .unwrap()
        .effort = "max".into();
    for (client, path, default, explicit) in [
        (
            "codex",
            "v1/responses",
            json!({"model":"coding","input":"hello"}),
            json!({"reasoning":{"effort":"low"}}),
        ),
        (
            "muse",
            "v1/responses",
            json!({"model":"coding","input":"hello"}),
            json!({"reasoning":{"effort":"low"}}),
        ),
        (
            "opencode",
            "v1/chat/completions",
            json!({"model":"coding","messages":[{"role":"user","content":"hello"}]}),
            json!({"reasoning_effort":"low"}),
        ),
        (
            "claude",
            "v1/messages",
            json!({"model":"coding","max_tokens":4096,"messages":[{"role":"user","content":"hello"}]}),
            json!({"output_config":{"effort":"low"}}),
        ),
        (
            "antigravity",
            "v1beta/models/coding:generateContent",
            json!({"contents":[{"role":"user","parts":[{"text":"hello"}]}]}),
            json!({"generationConfig":{"thinkingConfig":{"thinkingLevel":"LOW"}}}),
        ),
    ] {
        for (override_body, expected) in [(json!({}), "max"), (explicit, "low")] {
            let mut body = default.clone();
            body.as_object_mut()
                .unwrap()
                .extend(override_body.as_object().unwrap().clone());
            let response = serve(
                &store,
                "entry",
                Some(client),
                None,
                path,
                request(path, body, "gateway-only"),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK, "{client}");
            let captured = records.lock().unwrap();
            let upstream = &captured.last().unwrap().1;
            assert_eq!(upstream["reasoning_effort"], expected, "{client}");
            assert_eq!(upstream["model"], "real-model");
        }
    }
    for (requested, expected) in [
        ("ultra", "ultra"),
        ("xhigh", "xhigh"),
        ("vendor-level", "vendor-level"),
    ] {
        let path = "v1/responses";
        let response = serve(
            &store,
            "entry",
            Some("muse"),
            None,
            path,
            request(
                path,
                json!({"model":"coding","input":"hello","reasoning":{"effort":requested}}),
                "gateway-only",
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            records.lock().unwrap().last().unwrap().1["reasoning_effort"],
            expected
        );
    }
    assert_eq!(store.endpoints["entry"].routes["coding"].effort, "max");
    task.abort();
}

#[tokio::test]
async fn matching_protocols_preserve_native_reasoning_objects() {
    let records = Arc::new(Mutex::new(Vec::new()));
    let captured = records.clone();
    let router = Router::new().fallback(post(move |Json(body): Json<Value>| {
        let captured = captured.clone();
        async move {
            captured.lock().unwrap().push(body);
            Json(json!({"object":"response","status":"completed","output":[],"choices":[],"content":[],"candidates":[]}))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let mut store = crate::deck::tests::fixture();
    store.providers.get_mut("upstream").unwrap().base_url = url;
    // Stale catalog capabilities must not filter explicit client controls.
    let model = store.models.get_mut("resource").unwrap();
    model.reasoning_supported = Some(false);
    model.reasoning.clear();
    for (protocol, path, body, field) in [
        (
            Protocol::OpenaiResponses,
            "v1/responses",
            json!({"model":"coding","input":"hello","reasoning":{"effort":"vendor-level","summary":"auto","enabled":false,"vendor_option":true}}),
            "/reasoning",
        ),
        (
            Protocol::OpenaiChat,
            "v1/chat/completions",
            json!({"model":"coding","messages":[],"reasoning":{"effort":"xhigh","max_tokens":2048,"enabled":false,"vendor_option":true},"reasoning_effort":"ultra"}),
            "/reasoning",
        ),
        (
            Protocol::Anthropic,
            "v1/messages",
            json!({"model":"coding","messages":[],"max_tokens":4096,"output_config":{"effort":"xhigh","format":{"type":"json"}},"thinking":{"type":"enabled","budget_tokens":2048,"vendor_option":true}}),
            "/thinking",
        ),
        (
            Protocol::Gemini,
            "v1beta/models/coding:generateContent",
            json!({"contents":[],"generationConfig":{"thinkingConfig":{"thinkingLevel":"VENDOR_LEVEL","thinkingBudget":-1,"includeThoughts":false,"vendor_option":true}}}),
            "/generationConfig/thinkingConfig",
        ),
    ] {
        store.providers.get_mut("upstream").unwrap().protocol = protocol;
        let response = serve(
            &store,
            "entry",
            None,
            None,
            path,
            request(path, body.clone(), "gateway-only"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "{protocol:?}");
        let captured = records.lock().unwrap();
        let upstream = captured.last().unwrap();
        assert_eq!(upstream.pointer(field), body.pointer(field), "{protocol:?}");
        for field in ["output_config", "reasoning_effort"] {
            if body.get(field).is_some() {
                assert_eq!(upstream[field], body[field]);
            }
        }
    }
    task.abort();
}

#[tokio::test]
async fn responses_to_anthropic_keeps_xhigh_distinct_from_max() {
    let records = Arc::new(Mutex::new(Vec::new()));
    let captured = records.clone();
    let router = Router::new().route("/v1/messages", post(move |Json(body): Json<Value>| {
        let captured = captured.clone();
        async move {
            captured.lock().unwrap().push(body.clone());
            Json(json!({"id":"msg-test","type":"message","role":"assistant","model":body["model"],"content":[{"type":"text","text":"hello"}],"stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":1}}))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let mut store = crate::deck::tests::fixture();
    let provider = store.providers.get_mut("upstream").unwrap();
    provider.base_url = url;
    provider.protocol = Protocol::Anthropic;
    let model = store.models.get_mut("resource").unwrap();
    model.upstream_id = "anthropic/claude-opus-5.5".into();
    model.reasoning = ["low", "high", "xhigh", "max"].map(str::to_string).to_vec();
    for effort in ["low", "xhigh", "max"] {
        let path = "v1/responses";
        let response = serve(
            &store,
            "entry",
            Some("codex"),
            None,
            path,
            request(
                path,
                json!({"model":"coding","input":"hello","reasoning":{"effort":effort}}),
                "gateway-only",
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            records.lock().unwrap().last().unwrap()["output_config"]["effort"],
            effort
        );
    }
    task.abort();
}

#[tokio::test]
async fn request_uses_shared_route_credentials_and_tier_and_hides_real_model() {
    let (url, records, task) = mock_upstream().await;
    let mut store = crate::deck::tests::fixture();
    store.providers.get_mut("upstream").unwrap().base_url = url;
    let response=serve(&store,"entry",None,None,"v1/responses",request("v1/responses",json!({"model":"coding","input":"hello","reasoning":{"effort":"none"},"max_output_tokens":99999}),"gateway-only")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let data: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1000000)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(data["model"], "coding");
    assert!(!data.to_string().contains("real-model"));
    let records = records.lock().unwrap();
    assert_eq!(records[0].0["authorization"], "Bearer upstream-only");
    assert_eq!(records[0].1["model"], "real-model");
    assert_eq!(records[0].1["reasoning_effort"], "none");
    task.abort();
}

#[tokio::test]
async fn route_edit_changes_live_calls_without_touching_harness_bindings() {
    let (url, records, task) = mock_upstream().await;
    let mut store = crate::deck::tests::fixture();
    store.providers.get_mut("upstream").unwrap().base_url = url;
    let binding = HarnessBinding {
        endpoint_id: "entry".into(),
        model: "coding".into(),
        ..Default::default()
    };
    for (real, effort) in [("first-model", "high"), ("second-model", "max")] {
        store.models.get_mut("resource").unwrap().upstream_id = real.into();
        store
            .endpoints
            .get_mut("entry")
            .unwrap()
            .routes
            .get_mut("coding")
            .unwrap()
            .effort = effort.into();
        let response = serve(
            &store,
            "entry",
            Some("hermes"),
            Some(&binding),
            "v1/chat/completions",
            request(
                "v1/chat/completions",
                json!({"model":"coding","messages":[{"role":"user","content":"hello"}]}),
                "gateway-only",
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    let records = records.lock().unwrap();
    assert_eq!(records[0].1["model"], "first-model");
    assert_eq!(records[1].1["model"], "second-model");
    assert_eq!(records[1].1["reasoning_effort"], "max");
    task.abort();
}

#[tokio::test]
async fn free_tier_names_route_through_chat_responses_and_gemini() {
    let (url, records, task) = mock_upstream().await;
    let mut store = crate::deck::tests::fixture();
    store.providers.get_mut("upstream").unwrap().base_url = url;
    let route = store
        .endpoints
        .get_mut("entry")
        .unwrap()
        .routes
        .remove("coding")
        .unwrap();
    store
        .endpoints
        .get_mut("entry")
        .unwrap()
        .routes
        .insert("thinkingmachines/inkling:free".into(), route);
    validate(&store).unwrap();
    for (path, body) in [
        (
            "v1/chat/completions",
            json!({"model":"thinkingmachines/inkling:free","messages":[{"role":"user","content":"hello"}]}),
        ),
        (
            "v1/responses",
            json!({"model":"thinkingmachines/inkling:free","input":"hello"}),
        ),
        (
            "v1beta/models/thinkingmachines/inkling:free:generateContent",
            json!({"contents":[{"role":"user","parts":[{"text":"hello"}]}]}),
        ),
    ] {
        let response = serve(
            &store,
            "entry",
            None,
            None,
            path,
            request(path, body, "gateway-only"),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "free-tier aliases must route on every compatible protocol"
        );
        axum::body::to_bytes(response.into_body(), 1000000)
            .await
            .unwrap();
    }
    let captured = records.lock().unwrap();
    assert_eq!(captured.len(), 3);
    assert!(captured
        .iter()
        .all(|(_, body)| body["model"] == "real-model"));
    task.abort();
}

#[tokio::test]
async fn rejects_invalid_auth_unknown_models_and_unsupported_images_before_upstream() {
    let (url, records, task) = mock_upstream().await;
    let mut store = crate::deck::tests::fixture();
    store.providers.get_mut("upstream").unwrap().base_url = url;
    for (token, body, status) in [
        (
            "upstream-only",
            json!({"model":"coding"}),
            StatusCode::UNAUTHORIZED,
        ),
        (
            "gateway-only",
            json!({"model":"real-model"}),
            StatusCode::BAD_REQUEST,
        ),
        (
            "gateway-only",
            json!({"model":"coding","messages":[{"role":"user","content":[{"type":"image_url","image_url":{"url":"https://example.test/image"}}]}]}),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        assert_eq!(
            serve(
                &store,
                "entry",
                None,
                None,
                "v1/chat/completions",
                request("v1/chat/completions", body, token)
            )
            .await
            .status(),
            status
        );
    }
    assert!(records.lock().unwrap().is_empty());
    task.abort();
}

#[tokio::test]
async fn responses_stream_preserves_events_and_public_model_names() {
    let (url, _, task) = mock_upstream().await;
    let mut store = crate::deck::tests::fixture();
    store.providers.get_mut("upstream").unwrap().base_url = url;
    let response = serve(
        &store,
        "entry",
        None,
        None,
        "v1/responses",
        request(
            "v1/responses",
            json!({"model":"coding","input":"hello","stream":true}),
            "gateway-only",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1_000_000)
        .await
        .unwrap();
    let events = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(events.contains("response.completed"));
    assert!(events.contains("hello"));
    assert!(events.contains("coding"));
    assert!(!events.contains("real-model"));
    task.abort();
}

#[test]
fn public_catalog_contains_no_upstream_identifiers_or_credentials() {
    let store = crate::deck::tests::fixture();
    let result = catalog(&store, &store.endpoints["entry"], "v1/models", Some("muse")).to_string();
    assert!(result.contains("coding"));
    for private in [
        "upstream-only",
        "gateway-only",
        "real-model",
        "Private model",
        "example.test",
    ] {
        assert!(!result.contains(private));
    }
}

#[tokio::test]
async fn explicit_native_tier_overrides_provider_default() {
    let (url, records, task) = mock_upstream().await;
    let mut store = crate::deck::tests::fixture();
    store.providers.get_mut("upstream").unwrap().base_url = url;
    store
        .endpoints
        .get_mut("entry")
        .unwrap()
        .routes
        .get_mut("coding")
        .unwrap()
        .effort
        .clear();
    let response = serve(&store,"entry",None,None,"v1/chat/completions",request("v1/chat/completions",json!({"model":"coding","reasoning_effort":"max","messages":[{"role":"user","content":"hello"}]}),"gateway-only")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(records.lock().unwrap()[0].1["reasoning_effort"], "max");
    task.abort();
}

#[tokio::test]
async fn restored_adapters_satisfy_strict_provider_request_contracts() {
    for (name, input, metadata, expected, forbidden) in [
        (
            "OpenRouter",
            Protocol::OpenaiResponses,
            None,
            json!({"/reasoning/effort":"max"}),
            vec!["/reasoning_effort", "/thinking", "/enable_thinking"],
        ),
        (
            "DeepSeek",
            Protocol::OpenaiResponses,
            None,
            json!({"/reasoning_effort":"max","/thinking/type":"enabled"}),
            vec!["/reasoning", "/enable_thinking"],
        ),
        (
            "ModelScope",
            Protocol::OpenaiResponses,
            None,
            json!({"/enable_thinking":true}),
            vec!["/reasoning_effort", "/reasoning", "/thinking"],
        ),
        (
            "SiliconFlow",
            Protocol::OpenaiResponses,
            None,
            json!({"/enable_thinking":true}),
            vec!["/reasoning_effort", "/reasoning", "/thinking"],
        ),
        (
            "OpenRouter",
            Protocol::Anthropic,
            None,
            json!({"/reasoning/effort":"max"}),
            vec!["/reasoning_effort", "/thinking"],
        ),
        (
            "OpenRouter",
            Protocol::OpenaiChat,
            None,
            json!({"/reasoning/effort":"max"}),
            vec!["/reasoning_effort", "/thinking"],
        ),
        (
            "DeepSeek",
            Protocol::OpenaiResponses,
            Some(
                json!({"codexChatReasoning":{"supportsThinking":false,"supportsEffort":true,"thinkingParam":"none","effortParam":"reasoning.effort","effortValueMode":"passthrough"}}),
            ),
            json!({"/reasoning/effort":"max"}),
            vec!["/reasoning_effort", "/thinking"],
        ),
        (
            "OpenRouter",
            Protocol::Gemini,
            None,
            json!({"/reasoning/effort":"low","/reasoning/exclude":false}),
            vec!["/reasoning_effort", "/thinking"],
        ),
    ] {
        let expected = Arc::new(expected);
        let captured = expected.clone();
        let observed = Arc::new(Mutex::new(0));
        let count = observed.clone();
        let router = Router::new().route("/v1/chat/completions",post(move |headers: HeaderMap, Json(body): Json<Value>| {
            let expected=captured.clone();let count=count.clone();let forbidden=forbidden.clone();
            async move {
                *count.lock().unwrap()+=1;
                let fields_ok=expected.as_object().unwrap().iter().all(|(path,value)|body.pointer(path)==Some(value));
                let absent=forbidden.iter().all(|path|body.pointer(path).is_none());
                let auth_ok=headers.get("authorization").is_some_and(|v|v=="Bearer upstream-only") && !headers.contains_key("x-api-key");
                if !fields_ok || !absent || !auth_ok { return (StatusCode::UNPROCESSABLE_ENTITY,Json(json!({"error":"provider contract mismatch"}))).into_response(); }
                Json(json!({"id":"strict-provider","object":"chat.completion","model":body["model"],"choices":[{"index":0,"message":{"role":"assistant","content":"contract-ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}})).into_response()
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let mut store = crate::deck::tests::fixture();
        let provider = store.providers.get_mut("upstream").unwrap();
        provider.base_url = url;
        provider.name = name.into();
        provider.provider_meta = metadata;
        store
            .endpoints
            .get_mut("entry")
            .unwrap()
            .routes
            .get_mut("coding")
            .unwrap()
            .effort = "max".into();
        let (path, body) = match input {
            Protocol::OpenaiResponses => (
                "v1/responses",
                json!({"model":"coding","input":"hello","reasoning":{"effort":"max"}}),
            ),
            Protocol::Anthropic => (
                "v1/messages",
                json!({"model":"coding","messages":[{"role":"user","content":"hello"}],"max_tokens":4096,"output_config":{"effort":"max"}}),
            ),
            Protocol::Gemini => (
                "v1beta/models/coding:generateContent",
                json!({"contents":[{"role":"user","parts":[{"text":"hello"}]}],"generationConfig":{"thinkingConfig":{"thinkingLevel":"LOW","includeThoughts":true}}}),
            ),
            _ => (
                "v1/chat/completions",
                json!({"model":"coding","messages":[{"role":"user","content":"hello"}],"reasoning_effort":"max"}),
            ),
        };
        let response = serve(
            &store,
            "entry",
            None,
            None,
            path,
            request(path, body, "gateway-only"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "{name} {input:?}");
        assert_eq!(*observed.lock().unwrap(), 1);
        task.abort();
    }
}
