//! Opt-in smoke tests against installed clients, using isolated native state and
//! localhost inference only. No user credentials, model service, or agent tools.
use super::*;
use axum::{extract::State, routing::any, Router};
use std::{
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
struct Probe {
    requests: Arc<Mutex<Vec<Value>>>,
}
async fn upstream(State(probe): State<Probe>, req: Request) -> Response {
    let auth = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let path = req.uri().path().to_owned();
    let body = axum::body::to_bytes(req.into_body(), 32 * 1024 * 1024)
        .await
        .unwrap();
    let data: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    probe.requests.lock().unwrap().push(json!({"path":path,"authMatches":auth=="Bearer native-client-dedicated-test-key","model":data.get("model"),"tools":data.get("tools").and_then(Value::as_array).map(Vec::len)}));
    if path.ends_with("chat/completions") {
        let events = [
            json!({"id":"probe","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"role":"assistant","content":"native-probe-ready"},"finish_reason":null}]}),
            json!({"id":"probe","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":3,"total_tokens":13}}),
        ];
        let text = events
            .iter()
            .map(|e| format!("data: {e}\n\n"))
            .collect::<String>()
            + "data: [DONE]\n\n";
        return ([("content-type", "text/event-stream")], text).into_response();
    }
    if path.ends_with("messages") {
        let events = [
            json!({"type":"message_start","message":{"id":"msg_probe","type":"message","role":"assistant","model":data["model"],"content":[],"usage":{"input_tokens":10,"output_tokens":0}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"native-probe-ready"}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":3}}),
            json!({"type":"message_stop"}),
        ];
        return (
            [("content-type", "text/event-stream")],
            events
                .iter()
                .map(|v| format!("event: {}\ndata: {v}\n\n", v["type"].as_str().unwrap()))
                .collect::<String>(),
        )
            .into_response();
    }
    let item = json!({"id":"msg_probe","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"native-probe-ready","annotations":[]}]});
    let response = json!({"id":"resp_probe","object":"response","created_at":1791090000,"model":data["model"],"status":"completed","output":[item.clone()],"usage":{"input_tokens":10,"output_tokens":3,"total_tokens":13}});
    let events = vec![
        json!({"type":"response.created","response":{"id":"resp_probe","object":"response","created_at":1791090000,"model":data["model"],"status":"in_progress","output":[]}}),
        json!({"type":"response.output_item.added","output_index":0,"item":{"id":"msg_probe","type":"message","role":"assistant","status":"in_progress","content":[]}}),
        json!({"type":"response.content_part.added","item_id":"msg_probe","output_index":0,"content_index":0,"part":{"type":"output_text","text":"","annotations":[]}}),
        json!({"type":"response.output_text.delta","item_id":"msg_probe","output_index":0,"content_index":0,"delta":"native-probe-ready"}),
        json!({"type":"response.output_text.done","item_id":"msg_probe","output_index":0,"content_index":0,"text":"native-probe-ready"}),
        json!({"type":"response.content_part.done","item_id":"msg_probe","output_index":0,"content_index":0,"part":{"type":"output_text","text":"native-probe-ready","annotations":[]}}),
        json!({"type":"response.output_item.done","output_index":0,"item":item}),
        json!({"type":"response.completed","response":response}),
    ];
    // Deliberately omit sequence_number: the actual bridge must supply it.
    (
        [("content-type", "text/event-stream")],
        events
            .iter()
            .map(|e| format!("event: {}\ndata: {e}\n\n", e["type"].as_str().unwrap()))
            .collect::<String>(),
    )
        .into_response()
}

async fn smoke(client: ModelClient, exe: String, protocol: &str) {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let mock = Router::new().fallback(upstream).with_state(Probe {
        requests: requests.clone(),
    });
    let upstream_task = tokio::spawn(async move { axum::serve(listener, mock).await.unwrap() });
    let model = if client == ModelClient::Antigravity {
        "openai/gpt-6-astra"
    } else {
        "probe/model"
    };
    let profile:ClientProfile=serde_json::from_value(json!({"endpoint":format!("http://{addr}/v1"),"protocol":protocol,"apiKey":"native-client-dedicated-test-key","models":[{"id":model,"name":"Native probe","context":1000000,"output":32768}],"defaultModel":model,"enabled":true,"localToken":"native-local-test-token"})).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gateway_addr = listener.local_addr().unwrap();
    let gateway = Router::new()
        .route(
            "/model-clients/:client/*path",
            any(
                |State(p): State<ClientProfile>,
                 Path((id, path)): Path<(String, String)>,
                 req: Request| async move {
                    let method = req.method().clone();
                    let r = handle_profile(parse_client(&id).unwrap(), p, &path, req).await;
                    eprintln!("GATEWAY {method} {path} {}", r.status());
                    r
                },
            ),
        )
        .route(
            "/muse-code/models",
            any(|State(p): State<ClientProfile>, req: Request| async move {
                handle_profile(ModelClient::Muse, p, "muse-code/models", req).await
            }),
        )
        .fallback(|req: Request| async move {
            eprintln!("UNMATCHED {} {}", req.method(), req.uri().path());
            StatusCode::NOT_FOUND
        })
        .with_state(profile);
    let gateway_task = tokio::spawn(async move { axum::serve(listener, gateway).await.unwrap() });
    let base = format!("http://{gateway_addr}/model-clients/{}", client.id());
    let mut cmd = Command::new(exe);
    cmd.current_dir(dir)
        .stdin(Stdio::null())
        .env_remove("GOOGLE_API_KEY")
        .env_remove("GEMINI_API_KEY")
        .env_remove("META_API_KEY");
    let output = std::fs::File::create(dir.join("stdout")).unwrap();
    let errors = std::fs::File::create(dir.join("stderr")).unwrap();
    cmd.stdout(output).stderr(errors);
    if client == ModelClient::Antigravity {
        let gemini_dir = dir.join("gemini");
        let config = gemini_dir.join("antigravity-cli");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(
            config.join("settings.json"),
            json!({"modelProvider":"gemini","model":model,"customModelsConfig":{"customModels":{model:{"modelId":model,"modelName":model,"label":"Native probe","maxTokens":1000000}}}}).to_string(),
        )
        .unwrap();
        cmd.arg(format!("--gemini_dir={}", gemini_dir.display()))
            .args([
                "--model",
                model,
                "--disable-slash-commands",
                "--print-timeout",
                "45s",
                "--print",
                "Reply native-probe-ready; do not call tools.",
            ])
            .env("GEMINI_API_KEY", "native-local-test-token")
            .env("GOOGLE_GEMINI_BASE_URL", &base)
            .env("AGY_CLI_DISABLE_AUTO_UPDATE", "1");
    } else {
        let config = dir.join("config/muse");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(config.join("settings.json"), "{\"schema_version\":1}").unwrap();
        cmd.env("XDG_CONFIG_HOME", dir.join("config"))
            .env("XDG_DATA_HOME", dir.join("data"))
            .env("XDG_CACHE_HOME", dir.join("cache"))
            .env("META_API_KEY", "native-local-test-token")
            .args([
                "exec",
                "--base-url",
                &base,
                "--model",
                model,
                "--json",
                "--max-model-steps",
                "1",
                "--no-session-log",
                "--no-foreign-personal-context",
                "--disable-web-tools",
                "--disable-write",
                "--disable-shell",
                "Reply native-probe-ready; do not call tools.",
            ]);
    }
    let mut child = cmd.spawn().unwrap();
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if start.elapsed() > Duration::from_secs(50) {
            child.kill().ok();
            child.wait().ok();
            break None;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    };
    gateway_task.abort();
    upstream_task.abort();
    let stdout = std::fs::read_to_string(dir.join("stdout")).unwrap_or_default();
    let stderr = std::fs::read_to_string(dir.join("stderr")).unwrap_or_default();
    let captured = requests.lock().unwrap();
    eprintln!(
        "client={} status={status:?} requests={}",
        client.id(),
        serde_json::to_string(&*captured).unwrap()
    );
    assert!(
        !captured.is_empty(),
        "Native client made no inference request. stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(captured
        .iter()
        .all(|r| r["authMatches"] == true && r["model"] == model));
    assert!(
        status.is_some_and(|s| s.success()),
        "Native client failed. stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        stdout.contains("native-probe-ready"),
        "Missing native result. stdout={stdout:?} stderr={stderr:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Requires an installed Antigravity CLI; uses only isolated state and local inference"]
async fn antigravity_native() {
    smoke(
        ModelClient::Antigravity,
        std::env::var("CC_SWITCH_NATIVE_AGY")
            .expect("Set CC_SWITCH_NATIVE_AGY to the native executable"),
        "openai-chat",
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Requires an installed Muse binary; uses only isolated state and local inference"]
async fn muse_native() {
    smoke(
        ModelClient::Muse,
        std::env::var("CC_SWITCH_NATIVE_MUSE")
            .expect("Set CC_SWITCH_NATIVE_MUSE to the native binary, not an auto-updater"),
        "openai-responses",
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Requires installed Muse; localhost Chat Completions conversion"]
async fn muse_native_chat() {
    smoke(
        ModelClient::Muse,
        std::env::var("CC_SWITCH_NATIVE_MUSE").unwrap(),
        "openai-chat",
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Requires installed Muse; localhost Anthropic Messages conversion"]
async fn muse_native_anthropic() {
    smoke(
        ModelClient::Muse,
        std::env::var("CC_SWITCH_NATIVE_MUSE").unwrap(),
        "anthropic",
    )
    .await;
}
