use super::*;
use crate::{settings, AppSettings, Database};
use serial_test::serial;
use std::{ffi::OsString, sync::Arc};

struct TestHome {
    dir: tempfile::TempDir,
    settings: AppSettings,
    env: Vec<(&'static str, Option<OsString>)>,
}

impl TestHome {
    fn new() -> Self {
        let previous_settings = settings::get_settings();
        let dir = tempfile::tempdir().unwrap();
        let vars = [
            "CC_SWITCH_TEST_HOME",
            "HERMES_HOME",
            "XDG_CONFIG_HOME",
            "LOCALAPPDATA",
        ];
        let env = vars
            .into_iter()
            .map(|name| (name, std::env::var_os(name)))
            .collect();
        std::env::set_var("CC_SWITCH_TEST_HOME", dir.path());
        std::env::set_var("HERMES_HOME", dir.path().join(".hermes"));
        std::env::set_var("XDG_CONFIG_HOME", dir.path().join(".config"));
        std::env::set_var("LOCALAPPDATA", dir.path().join("AppData/Local"));
        settings::update_settings(AppSettings::default()).unwrap();
        Self {
            dir,
            settings: previous_settings,
            env,
        }
    }

    fn state(&self) -> AppState {
        AppState::new(Arc::new(Database::memory().unwrap()))
    }

    fn projection(&self, client: &str) -> ClientProfile {
        let store = crate::deck::tests::fixture();
        let binding = HarnessBinding {
            endpoint_id: "entry".into(),
            model: if client == "claude-desktop" {
                "claude-sonnet-5"
            } else {
                "coding"
            }
            .into(),
            ..Default::default()
        };
        project_profile(&store, client, &binding, "http://127.0.0.1:15721").unwrap()
    }
}

struct NativeConnection {
    base: String,
    token: String,
    model: String,
}

fn connection_from_native_files(client: &str) -> NativeConnection {
    let json_file =
        |path: PathBuf| -> Value { serde_json::from_slice(&fs::read(path).unwrap()).unwrap() };
    let text = |value: &Value| value.as_str().unwrap().to_string();
    match client {
        "codex" => {
            let config: toml::Value =
                fs::read_to_string(crate::codex_config::get_codex_config_path())
                    .unwrap()
                    .parse()
                    .unwrap();
            NativeConnection {
                base: config["model_providers"]["modeldeck"]["base_url"]
                    .as_str()
                    .unwrap()
                    .into(),
                token: config["model_providers"]["modeldeck"]["experimental_bearer_token"]
                    .as_str()
                    .unwrap()
                    .into(),
                model: config["model"].as_str().unwrap().into(),
            }
        }
        "claude" => {
            let config = json_file(crate::config::get_claude_settings_path());
            NativeConnection {
                base: text(&config["env"]["ANTHROPIC_BASE_URL"]),
                token: text(&config["env"]["ANTHROPIC_AUTH_TOKEN"]),
                model: text(&config["env"]["ANTHROPIC_MODEL"]),
            }
        }
        "claude-desktop" => {
            let config =
                json_file(crate::claude_desktop_config::managed_file_paths().unwrap()[2].clone());
            let model = config["inferenceModels"][0]
                .get("name")
                .unwrap_or(&config["inferenceModels"][0]);
            NativeConnection {
                base: text(&config["inferenceGatewayBaseUrl"]),
                token: text(&config["inferenceGatewayApiKey"]),
                model: text(model),
            }
        }
        "grokbuild" => {
            let config: toml::Value =
                fs::read_to_string(crate::grok_config::get_grok_config_path())
                    .unwrap()
                    .parse()
                    .unwrap();
            let model = config["models"]["default"].as_str().unwrap();
            NativeConnection {
                base: config["model"][model]["base_url"].as_str().unwrap().into(),
                token: config["model"][model]["api_key"].as_str().unwrap().into(),
                model: model.into(),
            }
        }
        "opencode" => {
            let config = crate::opencode_config::read_opencode_config().unwrap();
            NativeConnection {
                base: text(&config["provider"]["modeldeck"]["options"]["baseURL"]),
                token: text(&config["provider"]["modeldeck"]["options"]["apiKey"]),
                model: config["model"]
                    .as_str()
                    .unwrap()
                    .strip_prefix("modeldeck/")
                    .unwrap()
                    .into(),
            }
        }
        "hermes" => {
            let config = crate::hermes_config::read_hermes_config().unwrap();
            let provider = config["custom_providers"]
                .as_sequence()
                .unwrap()
                .iter()
                .find(|p| p["name"].as_str() == Some("modeldeck"))
                .unwrap();
            NativeConnection {
                base: provider["base_url"].as_str().unwrap().into(),
                token: provider["api_key"].as_str().unwrap().into(),
                model: config["model"]["default"].as_str().unwrap().into(),
            }
        }
        _ => panic!("Unsupported fixture client"),
    }
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn applied_native_provider_configs_reach_the_persisted_shared_gateway() {
    use axum::{
        extract::Json,
        http::HeaderMap,
        routing::{any, post},
        Router,
    };
    use std::sync::Mutex;
    let home = TestHome::new();
    let state = home.state();
    let records = Arc::new(Mutex::new(Vec::<(String, Value, HeaderMap)>::new()));
    let hermes_file = crate::hermes_config::get_hermes_config_path();
    fs::create_dir_all(hermes_file.parent().unwrap()).unwrap();
    fs::write(&hermes_file, "memory:\n  memory_enabled: false\n  provider: user-owned\nmodel:\n  default: original-model\n").unwrap();
    let memory_file = home.dir.path().join(".hermes/memories/MEMORY.md");
    fs::create_dir_all(memory_file.parent().unwrap()).unwrap();
    fs::write(&memory_file, "User-owned Hermes memory content").unwrap();
    let captured = records.clone();
    let upstream = Router::new().route("/v1/chat/completions", post(move |headers: HeaderMap, Json(body): Json<Value>| {
        let captured = captured.clone();
        async move {
            captured.lock().unwrap().push((headers["authorization"].to_str().unwrap().into(), body.clone(), headers));
            Json(json!({"id":"native-test","object":"chat.completion","model":body["model"],"choices":[{"index":0,"message":{"role":"assistant","content":"native-config-ready"},"finish_reason":"stop"}],"usage":{"prompt_tokens":2,"completion_tokens":1,"total_tokens":3}}))
        }
    }));
    let upstream_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_url = format!("http://{}/v1", upstream_listener.local_addr().unwrap());
    let upstream_task =
        tokio::spawn(async move { axum::serve(upstream_listener, upstream).await.unwrap() });
    let gateway_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", gateway_listener.local_addr().unwrap());
    let gateway = Router::new().route(
        "/deck-clients/:client/*path",
        any(crate::deck::gateway::handle_harness),
    );
    let gateway_task =
        tokio::spawn(async move { axum::serve(gateway_listener, gateway).await.unwrap() });
    let mut store = crate::deck::tests::fixture();
    store.providers.get_mut("upstream").unwrap().base_url = upstream_url;
    store.models.get_mut("resource").unwrap().context = 1_000_000;
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let mut connections = Vec::new();

    for id in [
        "codex",
        "claude",
        "claude-desktop",
        "grokbuild",
        "opencode",
        "hermes",
    ] {
        let binding = HarnessBinding {
            endpoint_id: "entry".into(),
            model: if id == "claude-desktop" {
                "claude-sonnet-5"
            } else {
                "coding"
            }
            .into(),
            ..Default::default()
        };
        store.bindings.insert(id.into(), binding.clone());
        store.active_bindings.insert(id.into(), binding.clone());
        let profile = project_profile(&store, id, &binding, &origin).unwrap();
        apply(id, &profile, None, &state, || {
            crate::deck::write_store(&store)
        })
        .unwrap();
        let connection = connection_from_native_files(id);
        assert_eq!(connection.token, "gateway-only");
        assert_eq!(connection.model, binding.model);
        if id == "hermes" {
            let config = crate::hermes_config::read_hermes_config().unwrap();
            assert_eq!(config["memory"]["provider"].as_str(), Some("user-owned"));
            assert_eq!(config["memory"]["memory_enabled"].as_bool(), Some(false));
            assert_eq!(
                fs::read_to_string(&memory_file).unwrap(),
                "User-owned Hermes memory content"
            );
        }
        for snapshot in capture(&paths(id).unwrap()).unwrap() {
            if let Some(bytes) = snapshot.bytes {
                assert!(
                    !String::from_utf8_lossy(&bytes).contains("upstream-only"),
                    "{id} leaked upstream credentials"
                );
            }
        }
        let protocol = crate::deck::client_protocol(id).unwrap();
        let (path, body) = match protocol {
            Protocol::OpenaiResponses => (
                "responses",
                json!({"model":connection.model,"input":"hello","max_output_tokens":99999}),
            ),
            Protocol::Anthropic => (
                "messages",
                json!({"model":connection.model,"max_tokens":99999,"messages":[{"role":"user","content":"hello"}]}),
            ),
            _ => (
                "chat/completions",
                json!({"model":connection.model,"max_tokens":99999,"messages":[{"role":"user","content":"hello"}]}),
            ),
        };
        let base = connection.base.trim_end_matches('/');
        let url = format!(
            "{base}{}{path}",
            if base.ends_with("/v1") { "/" } else { "/v1/" }
        );
        let response = client
            .post(&url)
            .bearer_auth(&connection.token)
            .header("user-agent", format!("native-{id}/1.0"))
            .header("http-referer", format!("https://example.invalid/{id}"))
            .header("x-openrouter-title", format!("Native {id}"))
            .header("cookie", "local-cookie=must-not-leak")
            .header("x-api-key", "local-only-credential")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "{id}: {}",
            response.text().await.unwrap()
        );
        let response: Value = response.json().await.unwrap();
        assert!(response.to_string().contains("native-config-ready"));
        assert!(!response.to_string().contains("real-model"));
        let captured = records.lock().unwrap();
        let (auth, request, upstream_headers) = captured.last().unwrap();
        assert_eq!(auth, "Bearer upstream-only");
        assert_eq!(upstream_headers["user-agent"], format!("native-{id}/1.0"));
        assert_eq!(
            upstream_headers["http-referer"],
            format!("https://example.invalid/{id}")
        );
        assert_eq!(
            upstream_headers["x-openrouter-title"],
            format!("Native {id}")
        );
        assert!(!upstream_headers.contains_key("cookie"));
        assert!(!upstream_headers.contains_key("x-api-key"));
        assert_eq!(request["model"], "real-model");
        assert_eq!(
            request["reasoning_effort"],
            if id == "claude-desktop" {
                "max"
            } else {
                "high"
            }
        );
        assert!(request["max_tokens"].as_u64().unwrap() <= 8192);
        drop(captured);
        connections.push((id, connection));
    }

    // Reusing the already-applied CLI file must pick up a resource edit.
    let unchanged_config = fs::read(crate::codex_config::get_codex_config_path()).unwrap();
    store.models.get_mut("resource").unwrap().upstream_id = "new-real-model".into();
    store
        .endpoints
        .get_mut("entry")
        .unwrap()
        .routes
        .get_mut("coding")
        .unwrap()
        .effort = "max".into();
    crate::deck::write_store(&store).unwrap();
    let connection = &connections[0].1;
    let response = client
        .post(format!("{}/responses", connection.base))
        .bearer_auth(&connection.token)
        .json(&json!({"model":connection.model,"input":"again"}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let captured = records.lock().unwrap();
    assert_eq!(captured.last().unwrap().1["model"], "new-real-model");
    assert_eq!(captured.last().unwrap().1["reasoning_effort"], "max");
    drop(captured);
    assert_eq!(
        fs::read(crate::codex_config::get_codex_config_path()).unwrap(),
        unchanged_config
    );
    assert_eq!(records.lock().unwrap().len(), 7);
    for (id, _) in connections {
        restore(id, || Ok(())).unwrap();
    }
    gateway_task.abort();
    upstream_task.abort();
}

#[test]
#[serial]
fn old_codex_backups_cannot_restore_or_compare_official_login_files() {
    let home = TestHome::new();
    let state = home.state();
    let profile = home.projection("codex");
    apply("codex", &profile, None, &state, || Ok(())).unwrap();
    let backup_file = backup_path("codex").unwrap();
    let mut backup: Backup = serde_json::from_slice(&fs::read(&backup_file).unwrap()).unwrap();
    let auth_path = crate::codex_config::get_codex_auth_path();
    let marker_path = crate::codex_config::get_codex_managed_oauth_live_auth_marker_path();
    for path in [&auth_path, &marker_path] {
        let snapshot = Snapshot {
            path: path.clone(),
            bytes: Some(b"obsolete login snapshot".to_vec()),
            mode: None,
        };
        backup.original.push(snapshot.clone());
        backup.applied.push(snapshot);
        fs::write(path, "current official login").unwrap();
    }
    fs::write(&backup_file, serde_json::to_vec(&backup).unwrap()).unwrap();
    apply("codex", &profile, Some(&profile), &state, || Ok(())).unwrap();
    restore("codex", || Ok(())).unwrap();
    assert_eq!(
        fs::read_to_string(auth_path).unwrap(),
        "current official login"
    );
    assert_eq!(
        fs::read_to_string(marker_path).unwrap(),
        "current official login"
    );
}

impl Drop for TestHome {
    fn drop(&mut self) {
        // Restore the settings cache while writes still target this fixture.
        settings::update_settings(self.settings.clone()).unwrap();
        for (name, value) in &self.env {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

#[test]
#[serial]
fn model_connections_preserve_client_owned_mcp_instructions_and_skills() {
    let home = TestHome::new();
    let state = home.state();
    let claude_path = crate::config::get_claude_settings_path();
    let codex_path = crate::codex_config::get_codex_config_path();
    let opencode_path = crate::opencode_config::get_opencode_config_path();
    let hermes_path = crate::hermes_config::get_hermes_config_path();
    let claude = json!({
        "env": {"USER_OWNED_ENV": "keep"},
        "permissions": {"allow": ["Read"]},
        "customSetting": "keep"
    });
    let codex =
        "approval_policy = \"on-request\"\n[mcp_servers.native]\ncommand = \"native-server\"\n";
    let opencode = json!({
        "mcp": {"native": {"type": "local", "command": ["native-server"]}},
        "instructions": ["user-instructions.md"],
        "permission": {"edit": "ask"}
    });
    let hermes = "memory:\n  memory_enabled: false\n  provider: user-owned\nmcp_servers:\n  native:\n    command: native-server\nmodel:\n  default: original-model\n";
    let originals = [
        (claude_path.clone(), serde_json::to_vec(&claude).unwrap()),
        (codex_path.clone(), codex.as_bytes().to_vec()),
        (
            opencode_path.clone(),
            serde_json::to_vec(&opencode).unwrap(),
        ),
        (hermes_path.clone(), hermes.as_bytes().to_vec()),
    ];
    for (path, bytes) in &originals {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    let independent_files = [
        home.dir.path().join(".claude.json"),
        home.dir.path().join(".claude/CLAUDE.md"),
        home.dir.path().join(".claude/skills/native/SKILL.md"),
        home.dir.path().join(".codex/AGENTS.md"),
        home.dir.path().join(".codex/skills/native/SKILL.md"),
        home.dir.path().join(".hermes/memories/MEMORY.md"),
    ];
    for path in &independent_files {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"User-owned content; never synchronize or replace.").unwrap();
    }

    for client in ["claude", "codex", "opencode", "hermes"] {
        apply(client, &home.projection(client), None, &state, || Ok(())).unwrap();
    }
    let applied_claude: Value = read_json_file(&claude_path).unwrap();
    assert_eq!(applied_claude["env"]["USER_OWNED_ENV"], "keep");
    assert_eq!(applied_claude["permissions"], claude["permissions"]);
    assert_eq!(applied_claude["customSetting"], "keep");
    let applied_codex: toml::Value = fs::read_to_string(&codex_path).unwrap().parse().unwrap();
    assert_eq!(
        applied_codex["approval_policy"].as_str(),
        Some("on-request")
    );
    assert_eq!(
        applied_codex["mcp_servers"]["native"]["command"].as_str(),
        Some("native-server")
    );
    let applied_opencode = crate::opencode_config::read_opencode_config().unwrap();
    for field in ["mcp", "instructions", "permission"] {
        assert_eq!(applied_opencode[field], opencode[field]);
    }
    let applied_hermes = crate::hermes_config::read_hermes_config().unwrap();
    assert_eq!(
        applied_hermes["memory"]["memory_enabled"].as_bool(),
        Some(false)
    );
    assert_eq!(
        applied_hermes["memory"]["provider"].as_str(),
        Some("user-owned")
    );
    assert_eq!(
        applied_hermes["mcp_servers"]["native"]["command"].as_str(),
        Some("native-server")
    );

    for client in ["claude", "codex", "opencode", "hermes"] {
        restore(client, || Ok(())).unwrap();
    }
    for (path, bytes) in &originals {
        assert_eq!(&fs::read(path).unwrap(), bytes);
    }
    for path in &independent_files {
        assert_eq!(
            fs::read(path).unwrap(),
            b"User-owned content; never synchronize or replace."
        );
    }
}

#[test]
#[serial]
fn codex_apply_and_restore_preserve_official_oauth_with_legacy_setting_disabled() {
    let home = TestHome::new();
    let state = home.state();
    let auth_path = crate::codex_config::get_codex_auth_path();
    let config_path = crate::codex_config::get_codex_config_path();
    fs::create_dir_all(auth_path.parent().unwrap()).unwrap();
    let original_auth = br#"{"auth_mode":"chatgpt","tokens":{"access_token":"official-access","refresh_token":"official-refresh"}}"#;
    let original_config =
        "model_provider = \"openai\"\nmodel = \"official-model\"\nweb_search = \"live\"\n";
    fs::write(&auth_path, original_auth).unwrap();
    fs::write(&config_path, original_config).unwrap();
    assert!(!settings::get_settings().preserve_codex_official_auth_on_switch);

    let profile = home.projection("codex");
    apply("codex", &profile, None, &state, || Ok(())).unwrap();
    assert_eq!(fs::read(&auth_path).unwrap(), original_auth);
    let config: toml::Value = fs::read_to_string(&config_path).unwrap().parse().unwrap();
    assert_eq!(config["model"].as_str(), Some("coding"));
    assert_eq!(config["web_search"].as_str(), Some("disabled"));
    let catalog: Value = serde_json::from_slice(
        &fs::read(crate::codex_config::get_codex_model_catalog_path()).unwrap(),
    )
    .unwrap();
    assert_eq!(catalog["models"][0]["supports_search_tool"], json!(false));
    assert_eq!(
        config["model_providers"]["modeldeck"]["experimental_bearer_token"].as_str(),
        Some("gateway-only")
    );
    assert_eq!(
        config["model_providers"]["modeldeck"]["base_url"].as_str(),
        Some("http://127.0.0.1:15721/deck-clients/codex/v1")
    );

    // The exempt desktop can refresh its OAuth while CLI routing is active.
    let refreshed_auth = br#"{"auth_mode":"chatgpt","tokens":{"access_token":"refreshed-access","refresh_token":"refreshed-refresh"}}"#;
    fs::write(&auth_path, refreshed_auth).unwrap();
    restore("codex", || Ok(())).unwrap();
    assert_eq!(fs::read(&auth_path).unwrap(), refreshed_auth);
    assert_eq!(fs::read_to_string(&config_path).unwrap(), original_config);
}

#[cfg(unix)]
#[test]
#[serial]
fn managed_cli_launchers_and_muse_config_use_the_shared_deck_routes() {
    use std::os::unix::fs::PermissionsExt;
    let home = TestHome::new();
    let state = home.state();
    let exe = home.dir.path().join("native-probe");
    fs::write(&exe, "#!/bin/sh\nprintf '1.2.16\\n'\n").unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();

    for client in ["antigravity", "muse"] {
        let mut profile = home.projection(client);
        profile.executable = exe.to_string_lossy().into_owned();
        apply(client, &profile, None, &state, || Ok(())).unwrap();
        let script = fs::read_to_string(get_app_config_dir().join("bin").join(client)).unwrap();
        assert!(
            script.contains(&format!("/deck-clients/{client}")),
            "{client} launcher still uses the legacy route"
        );
        assert!(script.contains("deck-gateway-url"));
        assert!(!script.contains("/model-clients/"));
        assert!(!script.contains("upstream-only"));
    }
    let config: Value = serde_json::from_slice(
        &fs::read(home.dir.path().join(".config/muse/settings.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        config["endpoint_transport"]["base_url"],
        "http://127.0.0.1:15721/deck-clients/muse"
    );
    assert_eq!(config["schema_version"], 1);
}

#[test]
#[serial]
fn codex_native_catalog_keeps_each_models_tiers_and_default() {
    let home = TestHome::new();
    let mut profile = home.projection("codex");
    profile.models[0].reasoning = vec!["low".into(), "high".into(), "max".into()];
    profile.models[0].default_effort = "max".into();
    profile.effort = "max".into();
    let provider = generated_provider("codex", &profile).unwrap();
    let value = &provider.settings_config;
    let config: toml::Value = value["config"].as_str().unwrap().parse().unwrap();
    assert_eq!(config["model_reasoning_effort"].as_str(), Some("max"));
    assert_eq!(
        value["modelCatalog"]["models"][0]["reasoningLevels"],
        json!(["low", "high", "max"])
    );
    assert_eq!(
        value["modelCatalog"]["models"][0]["defaultReasoningLevel"],
        "max"
    );
    profile.models[0].reasoning.clear();
    profile.models[0].default_effort.clear();
    profile.effort.clear();
    let provider = generated_provider("codex", &profile).unwrap();
    assert_eq!(
        provider.settings_config["modelCatalog"]["models"][0]["reasoningLevels"],
        json!(["none"])
    );
}

#[cfg(unix)]
#[test]
#[serial]
fn claude_max_launcher_preserves_explicit_native_overrides_and_models() {
    use std::os::unix::fs::PermissionsExt;
    let home = TestHome::new();
    let mut profile = home.projection("claude");
    profile.effort = "max".into();
    for model in &mut profile.models {
        model.default_effort = "max".into();
    }
    let executable = home.dir.path().join("native claude");
    fs::write(&executable, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let launcher = home.dir.path().join("launch");
    fs::write(&launcher, claude_launcher(&profile, &executable)).unwrap();
    for (args, expected) in [
        (vec!["-p", "hello"], "max"),
        (vec!["--effort", "low", "-p", "hello"], "low"),
    ] {
        let output = std::process::Command::new("sh")
            .arg(&launcher)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        let args = text.lines().collect::<Vec<_>>();
        assert_eq!(args.iter().filter(|arg| **arg == "--effort").count(), 1);
        assert!(args.contains(&expected));
    }
    let provider = generated_provider("claude", &profile).unwrap();
    assert_eq!(
        provider.settings_config["modelPicker"]["options"]
            .as_array()
            .unwrap()
            .len(),
        profile.models.len()
    );
    assert!(provider.settings_config["env"]
        .get("CLAUDE_CODE_EFFORT_LEVEL")
        .is_none());
}

#[cfg(unix)]
#[test]
#[serial]
fn old_claude_backups_restore_the_new_managed_launcher_too() {
    use std::os::unix::fs::PermissionsExt;
    let home = TestHome::new();
    let state = home.state();
    let path = crate::config::get_claude_settings_path();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let original = b"{\"theme\":\"original\"}";
    fs::write(&path, original).unwrap();
    let settings_only = capture(std::slice::from_ref(&path)).unwrap();
    let backup = Backup {
        original: settings_only.clone(),
        applied: settings_only,
    };
    atomic_write_private(
        &backup_path("claude").unwrap(),
        &serde_json::to_vec(&backup).unwrap(),
    )
    .unwrap();
    let executable = home.dir.path().join("claude");
    fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let mut profile = home.projection("claude");
    profile.executable = executable.to_string_lossy().into();
    apply("claude", &profile, None, &state, || Ok(())).unwrap();
    let launcher = get_app_config_dir().join("bin/claude");
    assert!(launcher.exists());
    restore("claude", || Ok(())).unwrap();
    assert!(!launcher.exists());
    assert_eq!(fs::read(path).unwrap(), original);
}

#[cfg(unix)]
async fn installed_client_uses_applied_deck(id: &str, executable: String) {
    use crate::model_clients::gateway::native_tests::{upstream, Probe};
    use axum::{routing::any, Router};
    use std::{
        process::{Command, Stdio},
        sync::Mutex,
        time::{Duration, Instant},
    };
    let home = TestHome::new();
    let state = home.state();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let upstream_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_url = format!("http://{}/v1", upstream_listener.local_addr().unwrap());
    let upstream = Router::new().fallback(upstream).with_state(Probe {
        requests: requests.clone(),
    });
    let upstream_task =
        tokio::spawn(async move { axum::serve(upstream_listener, upstream).await.unwrap() });
    let gateway_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = gateway_listener.local_addr().unwrap();
    let origin = format!("http://{address}");
    let gateway = Router::new()
        .route(
            "/deck-clients/:client/*path",
            any(|axum::extract::Path((client, path)): axum::extract::Path<(String, String)>, request: axum::extract::Request| async move {
                let (parts, body) = request.into_parts();
                let bytes = axum::body::to_bytes(body, 32 * 1024 * 1024).await.unwrap();
                let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                eprintln!("native ingress {client}: {}", json!({"model":value.get("model"),"reasoning":value.get("reasoning"),"output_config":value.get("output_config"),"generationConfig":value.get("generationConfig"),"keys":value.as_object().map(|o|o.keys().collect::<Vec<_>>()),"tools":value.get("tools").and_then(Value::as_array).map(Vec::len)}));
                crate::deck::gateway::handle_client(&client, &path, axum::extract::Request::from_parts(parts, axum::body::Body::from(bytes))).await
            }),
        )
        .route(
            "/muse-code/models",
            any(|req| async {
                crate::deck::gateway::handle_client("muse", "muse-code/models", req).await
            }),
        );
    let gateway_task =
        tokio::spawn(async move { axum::serve(gateway_listener, gateway).await.unwrap() });
    let mut store = crate::deck::tests::fixture();
    let provider = store.providers.get_mut("upstream").unwrap();
    provider.base_url = upstream_url;
    provider.api_key = "native-client-dedicated-test-key".into();
    store.models.get_mut("resource").unwrap().reasoning = ["low", "medium", "high", "xhigh", "max"]
        .map(str::to_string)
        .to_vec();
    let default_effort = if id == "antigravity" { "high" } else { "max" };
    let alias = if id == "claude" {
        "claude-opus-5-5"
    } else {
        "coding"
    };
    if id == "claude" {
        store.endpoints.get_mut("entry").unwrap().routes.insert(
            alias.into(),
            ModelRoute {
                model_id: "resource".into(),
                effort: default_effort.into(),
            },
        );
    }
    store
        .endpoints
        .get_mut("entry")
        .unwrap()
        .routes
        .get_mut("coding")
        .unwrap()
        .effort = default_effort.into();
    let binding = HarnessBinding {
        endpoint_id: "entry".into(),
        model: alias.into(),
        executable,
        ..Default::default()
    };
    store.bindings.insert(id.into(), binding.clone());
    store.active_bindings.insert(id.into(), binding.clone());
    crate::deck::publish_origin("127.0.0.1", address.port()).unwrap();
    let profile = project_profile(&store, id, &binding, &origin).unwrap();
    apply(id, &profile, None, &state, || {
        crate::deck::write_store(&store)
    })
    .unwrap();
    let launcher = get_app_config_dir().join("bin").join(id);
    let wire_default = if id == "muse" {
        "xhigh"
    } else {
        default_effort
    };
    let mut cases = vec![(None, wire_default), (Some("low"), "low")];
    if id == "muse" {
        cases.push((Some("xhigh"), "xhigh"));
    }
    for (requested, expected) in cases {
        requests.lock().unwrap().clear();
        let mut command = Command::new(if id == "codex" {
            Path::new(&binding.executable)
        } else {
            launcher.as_path()
        });
        command
            .current_dir(home.dir.path())
            .stdin(Stdio::null())
            .stdout(fs::File::create(home.dir.path().join("native-stdout")).unwrap())
            .stderr(fs::File::create(home.dir.path().join("native-stderr")).unwrap());
        if id == "antigravity" {
            command
                .arg(format!(
                    "--gemini_dir={}",
                    home.dir.path().join(".gemini").display()
                ))
                .args([
                    "--disable-slash-commands",
                    "--print-timeout",
                    "45s",
                    "--print",
                    "Reply native-probe-ready; do not call tools.",
                ])
                .env("AGY_CLI_DISABLE_AUTO_UPDATE", "1");
        } else if id == "codex" {
            command
                .env("CODEX_HOME", home.dir.path().join(".codex"))
                .args([
                    "--no-daemon",
                    "-a",
                    "never",
                    "-s",
                    "read-only",
                    "exec",
                    "--ephemeral",
                    "--json",
                    "--color",
                    "never",
                    "--skip-git-repo-check",
                    "Reply native-probe-ready; do not call tools.",
                ]);
        } else if id == "claude" {
            command
                .env("CLAUDE_CONFIG_DIR", home.dir.path().join(".claude"))
                .env_remove("CLAUDE_CODE_EFFORT_LEVEL")
                .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
                .args([
                    "--bare",
                    "-p",
                    "Reply native-probe-ready; do not call tools.",
                    "--output-format",
                    "stream-json",
                    "--verbose",
                    "--max-turns",
                    "1",
                    "--tools",
                    "",
                    "--strict-mcp-config",
                    "--mcp-config",
                    "{\"mcpServers\":{}}",
                    "--no-session-persistence",
                    "--settings",
                ])
                .arg(crate::config::get_claude_settings_path());
        } else {
            command
                .env("XDG_CONFIG_HOME", home.dir.path().join(".config"))
                .env("XDG_DATA_HOME", home.dir.path().join("data"))
                .env("XDG_CACHE_HOME", home.dir.path().join("cache"))
                .env_remove("MUSE_AUTH_PATH")
                .args([
                    "exec",
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
        if let Some(effort) = requested {
            if id == "codex" {
                command.args(["-c", &format!("model_reasoning_effort=\"{effort}\"")]);
            } else {
                command.args([
                    if id == "muse" {
                        "--reasoning-effort"
                    } else {
                        "--effort"
                    },
                    effort,
                ]);
            }
        }
        let mut child = command.spawn().unwrap();
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break Some(status);
            }
            if start.elapsed() > Duration::from_secs(50) {
                child.kill().unwrap();
                child.wait().unwrap();
                break None;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        let stdout = fs::read_to_string(home.dir.path().join("native-stdout")).unwrap();
        let stderr = fs::read_to_string(home.dir.path().join("native-stderr")).unwrap();
        let requests = requests.lock().unwrap();
        assert!(
            !requests.is_empty(),
            "{id} made no inference request: {stderr}"
        );
        assert!(requests
            .iter()
            .all(|r| r["authMatches"] == true && r["model"] == "real-model"));
        assert!(
            requests
                .iter()
                .filter(|r| r["tools"].as_u64().is_some_and(|n| n > 1))
                .all(|r| r["effort"] == expected),
            "{id}: requested {requested:?}, expected {expected}, got {requests:?}"
        );
        assert!(
            requests.iter().any(|r| r["effort"] == expected),
            "{id}: no request reached the selected tier {expected}: {requests:?}"
        );
        assert!(status.is_some_and(|s| s.success()), "{id} failed: {stderr}");
        assert!(stdout.contains("native-probe-ready"), "{id}: {stdout}");
    }
    gateway_task.abort();
    upstream_task.abort();
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
#[serial]
#[ignore = "Requires an installed Antigravity CLI; isolated state and localhost only"]
async fn antigravity_installed_client_uses_applied_deck() {
    installed_client_uses_applied_deck(
        "antigravity",
        std::env::var("CC_SWITCH_NATIVE_AGY").unwrap(),
    )
    .await;
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
#[serial]
#[ignore = "Requires an installed Muse binary; isolated state and localhost only"]
async fn muse_installed_client_uses_applied_deck() {
    installed_client_uses_applied_deck("muse", std::env::var("CC_SWITCH_NATIVE_MUSE").unwrap())
        .await;
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
#[serial]
#[ignore = "Requires an installed Codex CLI; isolated state and localhost only"]
async fn codex_installed_client_uses_applied_deck() {
    installed_client_uses_applied_deck("codex", std::env::var("CC_SWITCH_NATIVE_CODEX").unwrap())
        .await;
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
#[serial]
#[ignore = "Requires an installed Claude Code; isolated state and localhost only"]
async fn claude_installed_client_uses_applied_deck() {
    installed_client_uses_applied_deck("claude", std::env::var("CC_SWITCH_NATIVE_CLAUDE").unwrap())
        .await;
}

// Explicit opt-in fixture for the installed clients and real upstream APIs.
// It shares the production gateway and native writers, with project-local state.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
#[serial]
#[ignore = "Requires MODELDECK_LIVE_TEST_ROOT and explicitly authorized real API access"]
async fn real_harness_gateway_fixture() {
    use axum::{
        body::{to_bytes, Body},
        extract::{Request, State},
        http::StatusCode,
        middleware::{self, Next},
        response::{IntoResponse, Response},
        routing::{any, post},
        Json, Router,
    };
    use std::{io::Write, sync::Mutex};
    let root = PathBuf::from(std::env::var("MODELDECK_LIVE_TEST_ROOT").unwrap());
    let isolated = root.join("home");
    assert_eq!(
        get_home_dir(),
        isolated,
        "Real user configuration is exempt"
    );
    fs::create_dir_all(&isolated).unwrap();
    settings::update_settings(AppSettings::default()).unwrap();
    let state = Arc::new(AppState::new(Arc::new(Database::memory().unwrap())));
    let store: DeckStore =
        serde_json::from_slice(&fs::read(root.join("initial-store.json")).unwrap()).unwrap();
    write_store(&store).unwrap();
    let records = Arc::new(Mutex::new(
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("gateway-records.jsonl"))
            .unwrap(),
    ));
    async fn observe(
        State(records): State<Arc<Mutex<fs::File>>>,
        req: Request,
        next: Next,
    ) -> Response {
        let path = req.uri().path().to_owned();
        let method = req.method().as_str().to_owned();
        let user_agent = req
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let (parts, body) = req.into_parts();
        let bytes = to_bytes(body, 16 * 1024 * 1024).await.unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        let start = std::time::Instant::now();
        let response = next
            .run(Request::from_parts(parts, Body::from(bytes)))
            .await;
        let status = response.status();
        let record = json!({
            "path":path,"method":method,"model":value.get("model"),
            "stream":value.get("stream"),
            "userAgent":user_agent,
            "tools":value.get("tools").and_then(Value::as_array).map(Vec::len),
            "toolNames":value.get("tools").and_then(Value::as_array).map(|tools| tools.iter().map(|tool| json!({
                "type":tool.get("type"),"name":tool.get("name").or_else(||tool.pointer("/function/name")),
                "children":tool.get("tools").and_then(Value::as_array).map(|values|values.iter().map(|v|v.get("name")).collect::<Vec<_>>()),
            })).collect::<Vec<_>>()),
            "status":status.as_u16(),"latencyMs":start.elapsed().as_millis(),
        });
        if status.is_client_error() || status.is_server_error() {
            let (parts, body) = response.into_parts();
            let bytes = to_bytes(body, 2 * 1024 * 1024).await.unwrap();
            let mut record = record;
            record["error"] = serde_json::from_slice::<Value>(&bytes)
                .unwrap_or(json!(String::from_utf8_lossy(&bytes)));
            writeln!(records.lock().unwrap(), "{record}").unwrap();
            Response::from_parts(parts, Body::from(bytes))
        } else {
            writeln!(records.lock().unwrap(), "{record}").unwrap();
            response
        }
    }
    #[derive(Clone)]
    struct Control {
        app: Arc<AppState>,
        origin: String,
    }
    async fn control(State(control): State<Control>, Json(request): Json<Value>) -> Response {
        let mut store = read_store().unwrap();
        let client = request["client"].as_str().unwrap_or("");
        let label = request["label"].as_str().unwrap_or("");
        let Some(mut binding) = store.bindings.get(client).cloned() else {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"Unknown test client"})),
            )
                .into_response();
        };
        let alias = if client == "claude-desktop" {
            match label {
                "nemotron" => "claude-sonnet-4-6",
                "inkling" => "claude-opus-4-6",
                "arena" => "claude-haiku-4-5",
                _ => "",
            }
        } else {
            match label {
                "nemotron" => "md-nemotron",
                "inkling" => "md-inkling",
                "arena" => "md-arena",
                _ => "",
            }
        };
        if alias.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"Unknown model test"})),
            )
                .into_response();
        }
        binding.model = alias.into();
        binding.utility_model = alias.into();
        if client == "claude-desktop" {
            // The desktop sign-in probe always uses Haiku independently of its main model.
            // Keep every helper on the current free-model case instead of the skipped Arena route.
            let resource = format!("claude_desktop_{label}");
            for route in store
                .endpoints
                .get_mut(&binding.endpoint_id)
                .unwrap()
                .routes
                .values_mut()
            {
                route.model_id = resource.clone();
            }
        }
        if let Some(upstream) = request["fallbackId"].as_str() {
            let resource = store.endpoints[&binding.endpoint_id].routes[alias]
                .model_id
                .clone();
            store.models.get_mut(&resource).unwrap().upstream_id = upstream.into();
        }
        let profile = match project_profile(&store, client, &binding, &control.origin) {
            Ok(p) => p,
            Err(error) => {
                return (StatusCode::BAD_REQUEST, Json(json!({"error":error}))).into_response()
            }
        };
        let previous = store
            .active_bindings
            .get(client)
            .and_then(|binding| project_profile(&store, client, binding, &control.origin).ok());
        store.bindings.insert(client.into(), binding.clone());
        store.active_bindings.insert(client.into(), binding);
        store.revision += 1;
        if let Err(error) = apply(client, &profile, previous.as_ref(), &control.app, || {
            write_store(&store)
        }) {
            return (StatusCode::BAD_REQUEST, Json(json!({"error":error}))).into_response();
        }
        Json(
            json!({"client":client,"alias":alias,"origin":control.origin,
                    "command":command(client),"applied":true}),
        )
        .into_response()
    }
    let port = std::env::var("MODELDECK_LIVE_TEST_PORT")
        .ok()
        .map(|value| value.parse::<u16>().expect("valid test port"))
        .unwrap_or(15721);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let origin = format!("http://{address}");
    publish_origin("127.0.0.1", address.port()).unwrap();
    let gateway = Router::new()
        .route(
            "/deck-clients/:client/*path",
            any(crate::deck::gateway::handle_harness),
        )
        .route("/deck/:endpoint/*path", any(crate::deck::gateway::handle))
        .route(
            "/claude-desktop/*path",
            any(
                |axum::extract::Path(path): axum::extract::Path<String>, req| async move {
                    crate::deck::gateway::handle_client("claude-desktop", &path, req).await
                },
            ),
        )
        .route(
            "/muse-code/models",
            any(|req| async {
                crate::deck::gateway::handle_client("muse", "muse-code/models", req).await
            }),
        )
        .route(
            "/v1/muse-code/models",
            any(|req| async {
                crate::deck::gateway::handle_client("muse", "muse-code/models", req).await
            }),
        )
        .layer(middleware::from_fn_with_state(records, observe))
        .merge(
            Router::new()
                .route("/__test/apply", post(control))
                .with_state(Control {
                    app: state,
                    origin: origin.clone(),
                }),
        );
    fs::write(
        root.join("gateway-ready.json"),
        json!({"origin":origin,"pid":std::process::id()}).to_string(),
    )
    .unwrap();
    axum::serve(listener, gateway).await.unwrap();
}
