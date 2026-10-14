use super::*;
use crate::{
    deck::{HarnessBinding, Protocol},
    settings, AppSettings,
};
use serial_test::serial;
use std::ffi::OsString;

const SCHEMA: &str = r#"
CREATE TABLE model_providers (
 id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL, base_url TEXT NOT NULL,
 wire_api TEXT NOT NULL CHECK(wire_api IN ('completions','responses')),
 azure_api_version TEXT, auth_kind TEXT NOT NULL CHECK(auth_kind IN ('api_key','bearer_token','none')),
 headers_json TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT 'original',
 updated_at TEXT NOT NULL DEFAULT 'original', type TEXT NOT NULL, settings_json TEXT NOT NULL,
 native_extra TEXT DEFAULT 'provider-owned'
);
CREATE TABLE provider_models (
 id TEXT PRIMARY KEY NOT NULL, provider_id TEXT NOT NULL REFERENCES model_providers(id) ON DELETE CASCADE,
 model_id TEXT NOT NULL, wire_model TEXT, display_name TEXT NOT NULL,
 max_prompt_tokens INTEGER, max_output_tokens INTEGER, wire_api_override TEXT,
 created_at TEXT NOT NULL DEFAULT 'original', updated_at TEXT NOT NULL DEFAULT 'original',
 native_extra TEXT DEFAULT 'model-owned', UNIQUE(provider_id,model_id)
);
CREATE TABLE app_state(key TEXT PRIMARY KEY NOT NULL,value TEXT NOT NULL,updated_at TEXT NOT NULL DEFAULT 'original');
CREATE TABLE github_accounts(token TEXT NOT NULL);
CREATE TABLE sessions(id TEXT PRIMARY KEY,provider_id TEXT REFERENCES model_providers(id) ON DELETE SET NULL,transcript TEXT);
INSERT INTO github_accounts VALUES('official-login-only');
INSERT INTO model_providers(id,name,type,base_url,wire_api,auth_kind,headers_json,settings_json)
 VALUES('user-provider','User provider','custom','https://user.invalid/v1','completions','api_key','{"X-Private":"unrelated-provider-key"}','{}');
INSERT INTO provider_models(id,provider_id,model_id,display_name) VALUES('user-model','user-provider','user-model','User model');
INSERT INTO app_state(key,value) VALUES('copilot-selected-model','original-choice'),('theme','user-theme');
INSERT INTO sessions VALUES('user-session','user-provider','user-owned conversation');
"#;

struct TestHome {
    dir: tempfile::TempDir,
    settings: AppSettings,
    env: Vec<(&'static str, Option<OsString>)>,
}

impl TestHome {
    fn new() -> Self {
        let settings = settings::get_settings();
        let dir = tempfile::tempdir().unwrap();
        let env = ["CC_SWITCH_TEST_HOME", "COPILOT_HOME"]
            .into_iter()
            .map(|key| (key, std::env::var_os(key)))
            .collect();
        std::env::set_var("CC_SWITCH_TEST_HOME", dir.path());
        std::env::set_var("COPILOT_HOME", dir.path().join("copilot"));
        settings::update_settings(AppSettings::default()).unwrap();
        fs::create_dir_all(dir.path().join("copilot")).unwrap();
        let conn = Connection::open(database_path().unwrap()).unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        Self { dir, settings, env }
    }

    fn profile(&self) -> ClientProfile {
        let store = crate::deck::tests::fixture();
        crate::deck::project_profile(
            &store,
            "copilot-desktop",
            &HarnessBinding {
                endpoint_id: "entry".into(),
                model: "coding".into(),
                ..Default::default()
            },
            "http://127.0.0.1:15721",
        )
        .unwrap()
    }

    fn conn(&self) -> Connection {
        connect(&database_path().unwrap()).unwrap()
    }

    fn assert_user_data(&self) {
        let conn = self.conn();
        assert_eq!(
            conn.query_row("SELECT token FROM github_accounts", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            "official-login-only"
        );
        assert_eq!(
            conn.query_row(
                "SELECT transcript FROM sessions WHERE id='user-session'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "user-owned conversation"
        );
        assert_eq!(
            conn.query_row("SELECT value FROM app_state WHERE key='theme'", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
            "user-theme"
        );
        assert!(conn
            .query_row(
                "SELECT headers_json FROM model_providers WHERE id='user-provider'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap()
            .contains("unrelated-provider-key"));
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        settings::update_settings(self.settings.clone()).unwrap();
        for (key, value) in &self.env {
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}

#[test]
#[serial]
fn native_records_preserve_other_providers_auth_and_session_data() {
    let home = TestHome::new();
    let mut profile = home.profile();
    for (protocol, kind, wire) in [
        ("openai-chat", "custom", "completions"),
        ("openai-responses", "custom", "responses"),
        ("anthropic", "anthropic", "completions"),
    ] {
        profile.protocol = protocol.into();
        apply(&profile, || Ok(())).unwrap();
        let state = read_state(&home.conn()).unwrap();
        let provider = state.provider.as_ref().unwrap();
        assert_eq!(provider["type"], kind);
        assert_eq!(provider["wire_api"], wire);
        assert_eq!(provider["auth_kind"], "none");
        let headers: Value =
            serde_json::from_str(provider["headers_json"].as_str().unwrap()).unwrap();
        assert_eq!(headers["Authorization"], "Bearer gateway-only");
        let mirrored: Value =
            serde_json::from_str(provider["settings_json"].as_str().unwrap()).unwrap();
        assert_eq!(mirrored["baseUrl"], provider["base_url"]);
        assert_eq!(mirrored["headers"], headers);
        assert_eq!(mirrored["authKind"], provider["auth_kind"]);
        assert_eq!(mirrored["wireApi"], provider["wire_api"]);
        assert_eq!(state.models.len(), profile.models.len());
        assert!(state.models.iter().all(
            |model| model["max_prompt_tokens"] == 191808 && model["max_output_tokens"] == 8192
        ));
        assert_eq!(
            state.selection.as_ref().unwrap()["value"],
            format!("{PROVIDER_ID}/coding")
        );
        let text = fs::read_to_string(backup_path()).unwrap();
        for secret in [
            "official-login-only",
            "unrelated-provider-key",
            "upstream-only",
            "user-owned conversation",
        ] {
            assert!(!text.contains(secret));
        }
        home.assert_user_data();
    }
    home.conn()
        .execute(
            "INSERT INTO sessions VALUES('managed-session',?1,'managed conversation')",
            [PROVIDER_ID],
        )
        .unwrap();
    profile.models[0].output = 4096;
    apply(&profile, || Ok(())).unwrap();
    let conn = home.conn();
    assert_eq!(
        conn.query_row(
            "SELECT provider_id FROM sessions WHERE id='managed-session'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        PROVIDER_ID
    );
    restore(|| Ok(())).unwrap();
    assert!(!backup_path().exists());
    assert_eq!(
        read_state(&conn).unwrap().selection.unwrap()["value"],
        "original-choice"
    );
    assert_eq!(
        conn.query_row(
            "SELECT transcript FROM sessions WHERE id='managed-session'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "managed conversation"
    );
    home.assert_user_data();
}

#[test]
#[serial]
fn failed_apply_and_restore_roll_back_only_modeldeck_records() {
    let home = TestHome::new();
    let mut profile = home.profile();
    let original = read_state(&home.conn()).unwrap();
    assert_eq!(
        apply(&profile, || Err("store failure".into())).unwrap_err(),
        "store failure"
    );
    assert_eq!(read_state(&home.conn()).unwrap(), original);
    assert!(!backup_path().exists());
    apply(&profile, || Ok(())).unwrap();
    let applied = read_state(&home.conn()).unwrap();
    let previous_backup = fs::read(backup_path()).unwrap();
    profile.default_model = "claude-sonnet-5".into();
    profile.models.remove(1);
    assert_eq!(
        apply(&profile, || Err("store failure".into())).unwrap_err(),
        "store failure"
    );
    assert_eq!(read_state(&home.conn()).unwrap(), applied);
    assert_eq!(fs::read(backup_path()).unwrap(), previous_backup);
    assert_eq!(
        restore(|| Err("restore commit failure".into())).unwrap_err(),
        "restore commit failure"
    );
    assert_eq!(read_state(&home.conn()).unwrap(), applied);
    assert_eq!(fs::read(backup_path()).unwrap(), previous_backup);
    restore(|| Ok(())).unwrap();
    assert_eq!(read_state(&home.conn()).unwrap(), original);
    home.assert_user_data();
}

#[test]
#[serial]
fn restore_detects_native_catalog_edits_but_keeps_new_user_model_selections() {
    let home = TestHome::new();
    let profile = home.profile();
    apply(&profile, || Ok(())).unwrap();
    let conn = home.conn();
    conn.execute(
        "UPDATE model_providers SET name='external edit' WHERE id=?1",
        [PROVIDER_ID],
    )
    .unwrap();
    assert!(restore(|| panic!("must not commit"))
        .unwrap_err()
        .contains("changed after"));
    conn.execute(
        "UPDATE model_providers SET name='ModelDeck' WHERE id=?1",
        [PROVIDER_ID],
    )
    .unwrap();
    conn.execute(
        "UPDATE app_state SET value='user-provider/user-model' WHERE key=?1",
        [SELECTION_KEY],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO app_state(key,value) VALUES('new-preference','user-owned')",
        [],
    )
    .unwrap();
    restore(|| Ok(())).unwrap();
    assert_eq!(
        read_state(&conn).unwrap().selection.unwrap()["value"],
        "user-provider/user-model"
    );
    assert_eq!(
        conn.query_row(
            "SELECT value FROM app_state WHERE key='new-preference'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "user-owned"
    );
    home.assert_user_data();
    apply(&profile, || Ok(())).unwrap();
    conn.execute("DELETE FROM app_state WHERE key=?1", [SELECTION_KEY])
        .unwrap();
    restore(|| Ok(())).unwrap();
    assert!(read_state(&conn).unwrap().selection.is_none());
}

#[test]
#[serial]
fn unknown_schema_and_configuration_directory_changes_fail_without_mutation() {
    let home = TestHome::new();
    let profile = home.profile();
    apply(&profile, || Ok(())).unwrap();
    let other = home.dir.path().join("other");
    fs::create_dir_all(&other).unwrap();
    let conn = Connection::open(other.join("data.db")).unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    let original = read_state(&conn).unwrap();
    std::env::set_var("COPILOT_HOME", &other);
    assert!(apply(&profile, || panic!("must not commit"))
        .unwrap_err()
        .contains("before changing"));
    assert_eq!(read_state(&conn).unwrap(), original);
    conn.execute("DROP TABLE provider_models", []).unwrap();
    assert!(apply(&profile, || panic!("must not commit"))
        .unwrap_err()
        .contains("schema"));
    std::env::set_var("COPILOT_HOME", home.dir.path().join("missing"));
    assert!(apply(&profile, || panic!("must not commit"))
        .unwrap_err()
        .contains("initialize"));
    assert!(!home.dir.path().join("missing/data.db").exists());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn desktop_database_configuration_reaches_the_shared_gateway() {
    use axum::{
        extract::Json,
        http::HeaderMap,
        routing::{any, post},
        Router,
    };
    let home = TestHome::new();
    let upstream_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_url = format!("http://{}/v1", upstream_listener.local_addr().unwrap());
    let upstream = Router::new().route("/v1/chat/completions",post(|headers: HeaderMap,Json(body): Json<Value>| async move {
        assert_eq!(headers["authorization"],"Bearer upstream-only");
        assert_eq!(body["model"],"real-model");
        assert_eq!(body["reasoning_effort"],"high");
        Json(json!({"id":"test","object":"chat.completion","model":"real-model","choices":[{"index":0,"message":{"role":"assistant","content":"desktop-ready"},"finish_reason":"stop"}]}))
    }));
    let upstream_task = tokio::spawn(async move {
        axum::serve(upstream_listener, upstream).await.unwrap();
    });
    let gateway_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", gateway_listener.local_addr().unwrap());
    let gateway = Router::new().route(
        "/deck-clients/:client/*path",
        any(crate::deck::gateway::handle_harness),
    );
    let gateway_task = tokio::spawn(async move {
        axum::serve(gateway_listener, gateway).await.unwrap();
    });
    let mut store = crate::deck::tests::fixture();
    store.providers.get_mut("upstream").unwrap().base_url = upstream_url;
    store.providers.get_mut("upstream").unwrap().protocol = Protocol::OpenaiChat;
    let binding = HarnessBinding {
        endpoint_id: "entry".into(),
        model: "coding".into(),
        ..Default::default()
    };
    store
        .active_bindings
        .insert("copilot-desktop".into(), binding.clone());
    let profile =
        crate::deck::project_profile(&store, "copilot-desktop", &binding, &origin).unwrap();
    apply(&profile, || crate::deck::write_store(&store)).unwrap();
    let state = read_state(&home.conn()).unwrap();
    let provider = state.provider.unwrap();
    let headers: BTreeMap<String, String> =
        serde_json::from_str(provider["headers_json"].as_str().unwrap()).unwrap();
    let native_model = state
        .models
        .iter()
        .find(|model| model["model_id"] == "coding")
        .unwrap();
    let mut request = reqwest::Client::builder().no_proxy().build().unwrap()
        .post(format!("{}/chat/completions",provider["base_url"].as_str().unwrap()))
        .json(&json!({"model":native_model["wire_model"],"messages":[{"role":"user","content":"hello"}]}));
    for (name, value) in headers {
        request = request.header(name, value);
    }
    let response = request.send().await.unwrap();
    assert!(response.status().is_success());
    let response = response.json::<Value>().await.unwrap();
    assert!(response.to_string().contains("desktop-ready"));
    assert!(!response.to_string().contains("real-model"));
    restore(|| Ok(())).unwrap();
    home.assert_user_data();
    gateway_task.abort();
    upstream_task.abort();
}

async fn desktop_sdk_protocol(protocol: Protocol) {
    use crate::model_clients::gateway::native_tests::{upstream, Probe};
    use axum::{routing::any, Router};
    use std::{
        process::{Command, Stdio},
        sync::{Arc, Mutex},
        time::Instant,
    };
    let home = TestHome::new();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let upstream_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_url = format!("http://{}/v1", upstream_listener.local_addr().unwrap());
    let mock = Router::new().fallback(upstream).with_state(Probe {
        requests: requests.clone(),
    });
    let upstream_task = tokio::spawn(async move {
        axum::serve(upstream_listener, mock).await.unwrap();
    });
    let gateway_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", gateway_listener.local_addr().unwrap());
    let gateway = Router::new().route(
        "/deck-clients/:client/*path",
        any(crate::deck::gateway::handle_harness),
    );
    let gateway_task = tokio::spawn(async move {
        axum::serve(gateway_listener, gateway).await.unwrap();
    });
    let mut store = crate::deck::tests::fixture();
    store.providers.get_mut("upstream").unwrap().protocol = protocol;
    store.providers.get_mut("upstream").unwrap().base_url = upstream_url;
    store.providers.get_mut("upstream").unwrap().api_key =
        "native-client-dedicated-test-key".into();
    let binding = HarnessBinding {
        endpoint_id: "entry".into(),
        model: "coding".into(),
        ..Default::default()
    };
    store
        .active_bindings
        .insert("copilot-desktop".into(), binding.clone());
    let profile =
        crate::deck::project_profile(&store, "copilot-desktop", &binding, &origin).unwrap();
    apply(&profile, || crate::deck::write_store(&store)).unwrap();
    let config = home.dir.path().join("native-model-records.json");
    fs::write(
        &config,
        serde_json::to_vec(&read_state(&home.conn()).unwrap()).unwrap(),
    )
    .unwrap();
    let stdout_path = home.dir.path().join("sdk-stdout");
    let stderr_path = home.dir.path().join("sdk-stderr");
    let mut child = Command::new("node")
        .arg(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../harness-tests/copilot-desktop/sdk-probe.mjs"),
        )
        .arg(config)
        .current_dir(home.dir.path())
        .env("COPILOT_CACHE_HOME", home.dir.path().join("copilot-cache"))
        .env(
            "MODELDECK_NATIVE_COPILOT",
            std::env::var("MODELDECK_NATIVE_COPILOT").unwrap(),
        )
        .env(
            "MODELDECK_NATIVE_COPILOT_SDK",
            std::env::var("MODELDECK_NATIVE_COPILOT_SDK").unwrap(),
        )
        .stdin(Stdio::null())
        .stdout(fs::File::create(&stdout_path).unwrap())
        .stderr(fs::File::create(&stderr_path).unwrap())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if started.elapsed() > Duration::from_secs(40) {
            child.kill().unwrap();
            child.wait().unwrap();
            break None;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    let stdout = fs::read_to_string(stdout_path).unwrap();
    let stderr = fs::read_to_string(stderr_path).unwrap();
    assert!(
        status.is_some_and(|status| status.success()),
        "Desktop SDK failed: {stderr}"
    );
    assert!(stdout.contains("desktop-sdk-native-probe-ready"));
    let requests = requests.lock().unwrap();
    assert!(
        !requests.is_empty(),
        "The desktop SDK made no inference request"
    );
    assert!(requests
        .iter()
        .all(|request| request["authMatches"] == true && request["model"] == "real-model"));
    assert!(
        requests.iter().any(|request| request["effort"] == "high"),
        "{protocol:?}: {requests:?}"
    );
    drop(requests);
    restore(|| Ok(())).unwrap();
    home.assert_user_data();
    gateway_task.abort();
    upstream_task.abort();
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
#[ignore = "Requires the official Copilot app SDK and CLI; isolated state and localhost only"]
async fn installed_desktop_sdk_uses_the_database_model_records() {
    for protocol in [
        Protocol::OpenaiChat,
        Protocol::OpenaiResponses,
        Protocol::Anthropic,
    ] {
        desktop_sdk_protocol(protocol).await;
    }
}
