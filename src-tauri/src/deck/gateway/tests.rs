use super::*;
use axum::{routing::post, Router};
use std::sync::{Arc, Mutex};

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
    assert_eq!(records[0].1["reasoning_effort"], "high");
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
async fn provider_default_does_not_inherit_a_harness_reasoning_tier() {
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
    assert!(records.lock().unwrap()[0]
        .1
        .get("reasoning_effort")
        .is_none());
    task.abort();
}
