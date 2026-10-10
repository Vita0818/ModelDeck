use super::{resolve_key, ClientProfile, ModelClient};
use crate::config::{atomic_write_private, get_app_config_dir, get_home_dir};
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

const CURSOR_STORAGE: &str = "src.vs.platform.reactivestorage.browser.reactiveStorageServiceImpl.persistentStorage.applicationUser";
const CURSOR_ADAPTER: &str = include_str!("../../resources/cursor-models.cjs");
const CURSOR_LOCAL_ADAPTER: &str = include_str!("../../resources/cursor-local.cjs");
const CURSOR_MANIFEST: &str = include_str!("../../resources/cursor-models.manifest.json");

fn gateway_origin_path() -> PathBuf {
    get_app_config_dir().join("model-gateway-url")
}

pub fn publish_gateway_origin(origin: &str) -> Result<(), String> {
    let store = super::read_store()?;
    if ![ModelClient::Antigravity, ModelClient::Muse]
        .iter()
        .any(|c| store.profiles.contains_key(c))
    {
        return Ok(());
    }
    let old = fs::read_to_string(gateway_origin_path())
        .ok()
        .map(|s| s.trim().to_string());
    if store.active.contains_key(&ModelClient::Muse) {
        let path = native_config(ModelClient::Muse);
        if path.is_file() {
            let mut value = read_json(&path)?;
            if old.as_ref().is_some_and(|old| {
                value
                    .pointer("/endpoint_transport/base_url")
                    .and_then(Value::as_str)
                    == Some(format!("{old}/model-clients/muse").as_str())
            }) {
                value["endpoint_transport"]["base_url"] =
                    json!(format!("{origin}/model-clients/muse"));
                atomic_write_private(
                    &path,
                    &serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }
    atomic_write_private(&gateway_origin_path(), format!("{origin}\n").as_bytes())
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub command: Option<String>,
    pub restart_required: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInspection {
    pub client: ModelClient,
    pub installed: bool,
    pub executable: Option<String>,
    pub configuration: String,
    pub capability: &'static str,
    pub version: Option<String>,
}

fn native_config(client: ModelClient) -> PathBuf {
    match client {
        ModelClient::Antigravity => get_home_dir().join(".gemini/antigravity-cli/settings.json"),
        ModelClient::Muse => get_home_dir().join(".config/muse/settings.json"),
        ModelClient::CursorAgent => get_home_dir().join(".cursor/cli-config.json"),
        ModelClient::Cursor => {
            #[cfg(target_os = "macos")]
            let dir = get_home_dir().join("Library/Application Support");
            #[cfg(not(target_os = "macos"))]
            let dir = dirs::config_dir().unwrap_or_else(get_home_dir);
            dir.join("Cursor/User/globalStorage/state.vscdb")
        }
    }
}

fn find_executable(client: ModelClient) -> Option<PathBuf> {
    let name = match client {
        ModelClient::Antigravity => "agy",
        ModelClient::Muse => "muse",
        ModelClient::CursorAgent => "cursor-agent",
        ModelClient::Cursor => "cursor",
    };
    let mut candidates = Vec::new();
    if client == ModelClient::CursorAgent {
        candidates.extend([
            get_home_dir().join(".local/bin/cursor-agent-local"),
            PathBuf::from("/usr/local/bin/cursor-agent-local"),
        ]);
        if let Some(path) = std::env::var_os("PATH") {
            candidates.extend(std::env::split_paths(&path).map(|p| p.join("cursor-agent-local")));
        }
    }
    if client == ModelClient::Muse {
        if let Ok(version) = fs::read_to_string(get_home_dir().join(".local/bin/.muse-version")) {
            let version = version.trim();
            if !version.is_empty()
                && version
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
            {
                candidates.push(
                    get_home_dir()
                        .join(".local/bin")
                        .join(format!("muse-bin-{version}")),
                );
            }
        }
    }
    candidates.extend([
        get_home_dir().join(".local/bin").join(name),
        PathBuf::from("/usr/local/bin").join(name),
    ]);
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|p| p.join(name)));
    }
    candidates.into_iter().find(|p| p.is_file())
}

pub fn inspect() -> Vec<ClientInspection> {
    [
        ModelClient::Antigravity,
        ModelClient::Cursor,
        ModelClient::CursorAgent,
        ModelClient::Muse,
    ]
    .into_iter()
    .map(|client| {
        let exe = find_executable(client);
        let version = exe.as_ref().and_then(|p| match client {
            ModelClient::Antigravity => command_version(p).ok(),
            ModelClient::CursorAgent => cursor_runtime(p).ok().map(|runtime| runtime.version),
            _ => None,
        });
        ClientInspection {
            client,
            installed: exe.is_some() || native_config(client).exists(),
            executable: exe.as_ref().map(|p| p.to_string_lossy().into()),
            configuration: native_config(client).to_string_lossy().into(),
            capability: match client {
                ModelClient::Antigravity => "gemini-or-chat",
                ModelClient::Cursor => "public-chat-endpoint",
                ModelClient::CursorAgent
                    if exe.as_ref().is_some_and(|p| {
                        cursor_runtime(p).is_ok_and(|runtime| runtime.local_api)
                    }) =>
                {
                    "cursor-local-api"
                }
                ModelClient::CursorAgent => "cursor-cloud-byok",
                ModelClient::Muse => "responses-bridge",
            },
            version,
        }
    })
    .collect()
}

fn executable(client: ModelClient, p: &ClientProfile) -> Result<PathBuf, String> {
    let path = if p.executable.is_empty() {
        find_executable(client).ok_or("Client executable was not found")?
    } else {
        PathBuf::from(&p.executable)
    };
    if !path.is_absolute() || !path.is_file() {
        return Err("Select an existing absolute executable path".into());
    }
    if path.starts_with(get_app_config_dir().join("bin")) {
        return Err("Select the native executable, not a CC Switch launcher".into());
    }
    Ok(path)
}

pub fn preflight(client: ModelClient, p: &ClientProfile) -> Result<(), String> {
    if client != ModelClient::CursorAgent {
        resolve_key(p)?;
    } else if p.key_env.is_empty() && p.api_key.trim().is_empty() {
        return Err(
            "Enter this Cursor Agent connection's dedicated API key or environment variable".into(),
        );
    }
    if client == ModelClient::Cursor {
        if !native_config(client).is_file() {
            return Err("Start Cursor IDE once to create its native configuration".into());
        }
        ensure_cursor_closed()?;
    } else {
        let path = executable(client, p)?;
        if client == ModelClient::CursorAgent {
            cursor_runtime(&path)?;
        }
        if client == ModelClient::Antigravity {
            let version = command_version(&path)?;
            let parsed: Vec<u32> = version
                .trim_start_matches('v')
                .split('.')
                .map(|n| n.parse::<u32>().unwrap_or(0))
                .collect();
            if parsed.as_slice() < [1, 2, 16].as_slice() {
                return Err(format!("Antigravity {version} does not support this custom-model API-key configuration. Update the existing CLI to 1.2.16 or newer, then apply again."));
            }
        }
        #[cfg(not(unix))]
        return Err("Managed CLI launchers currently require macOS or Linux".into());
    }
    Ok(())
}

pub(crate) fn ensure_cursor_closed() -> Result<(), String> {
    #[cfg(unix)]
    {
        let check = std::process::Command::new("pgrep")
            .args(["-x", "Cursor|cursor"])
            .output()
            .map_err(|_| "Cannot check whether Cursor is running")?;
        match check.status.code(){Some(0)=>return Err("Quit Cursor IDE before applying model settings so its running process cannot overwrite them".into()),Some(1)=>{},_=>return Err("Cannot confirm that Cursor is closed; native configuration has not been changed".into())}
    }
    #[cfg(windows)]
    {
        let check = std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq Cursor.exe", "/NH"])
            .output()
            .map_err(|_| "Cannot check whether Cursor is running")?;
        if !check.status.success()
            || String::from_utf8_lossy(&check.stdout)
                .to_lowercase()
                .contains("cursor.exe")
        {
            return Err("Quit Cursor IDE before applying model settings".into());
        }
    }
    Ok(())
}

fn command_version(executable: &Path) -> Result<String, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut child = Command::new(executable)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Cannot inspect native client version")?;
    let started = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if !status.success() {
                return Err("Native client version check failed".into());
            }
            let mut version = String::new();
            if let Some(stdout) = child.stdout.take() {
                stdout
                    .take(4096)
                    .read_to_string(&mut version)
                    .map_err(|e| e.to_string())?;
            }
            return Ok(version.trim().to_string());
        }
        if started.elapsed() > Duration::from_secs(3) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Native client version check timed out".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

struct CursorRuntime {
    directory: PathBuf,
    version: String,
    local_api: bool,
}

/// Only the pinned official local runtime relaxes the cloud endpoint constraint.
pub(crate) fn cursor_agent_is_local(selected_executable: &str) -> bool {
    let path = if selected_executable.is_empty() {
        find_executable(ModelClient::CursorAgent)
    } else {
        Some(PathBuf::from(selected_executable))
    };
    path.is_some_and(|path| cursor_runtime(&path).is_ok_and(|runtime| runtime.local_api))
}

fn cursor_runtime(executable: &Path) -> Result<CursorRuntime, String> {
    let real = fs::canonicalize(executable).map_err(|e| e.to_string())?;
    let directory = real
        .parent()
        .ok_or("Invalid Cursor Agent executable path")?
        .to_path_buf();
    let entry = fs::read(directory.join("index.js")).map_err(|_| {
        "Select the native cursor-agent executable with its bundled index.js and Node runtime"
    })?;
    if !directory.join("node").is_file() {
        return Err("Cursor Agent's bundled Node runtime is missing".into());
    }
    let manifests: Value = serde_json::from_str(CURSOR_MANIFEST)
        .map_err(|_| "Invalid bundled Cursor compatibility manifest")?;
    let hash = format!("{:x}", Sha256::digest(&entry));
    let manifest=manifests.get(&hash).ok_or("This Cursor Agent version has not been verified with the model adapter. Update ModelDeck's adapter before applying.")?;
    for (file, expected) in manifest["chunks"]
        .as_object()
        .ok_or("Invalid Cursor compatibility manifest")?
    {
        let actual =
            fs::read(directory.join(file)).map_err(|_| "Cursor model module is missing")?;
        if Some(format!("{:x}", Sha256::digest(&actual)).as_str()) != expected.as_str() {
            return Err("Cursor model module changed; native files have not been modified".into());
        }
    }
    Ok(CursorRuntime {
        directory,
        version: manifest["version"].as_str().unwrap_or_default().to_string(),
        local_api: manifest["mode"].as_str() == Some("local-api"),
    })
}

fn cursor_local_launcher(
    runtime: &CursorRuntime,
    profile_path: &Path,
    origin_path: &Path,
    adapter_path: &Path,
    default_model: &str,
) -> String {
    format!(
        "#!/bin/sh\n# Managed by ModelDeck: official Cursor local API runtime.\nset -eu\nexport CCSWITCH_CURSOR_PROFILE_FILE={}\nexport MODELDECK_CURSOR_ORIGIN_FILE={}\nexport CURSOR_INVOKED_AS=cursor-agent-local\nhas_model=0\nfor arg do case \"$arg\" in --) break;; --model|--model=*) has_model=1;; esac; done\nif [ \"$has_model\" = 0 ]; then set -- --model {} \"$@\"; fi\nexec {} --use-system-ca --require {} {} --authless --disable-auto-update \"$@\"\n",
        quote(&profile_path.to_string_lossy()),
        quote(&origin_path.to_string_lossy()),
        quote(default_model),
        quote(&runtime.directory.join("node").to_string_lossy()),
        quote(&adapter_path.to_string_lossy()),
        quote(&runtime.directory.join("index.js").to_string_lossy()),
    )
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

fn read_json(path: &Path) -> Result<Value, String> {
    match fs::read(path) {
        Ok(data) => {
            let v: Value = serde_json::from_slice(&data)
                .map_err(|_| format!("Invalid JSON in {}", path.display()))?;
            if !v.is_object() {
                return Err("Native settings must be a JSON object".into());
            }
            Ok(v)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(format!("Cannot read native settings: {e}")),
    }
}

fn patch_settings(
    client: ModelClient,
    mut value: Value,
    p: &ClientProfile,
    base: &str,
    previous: Option<&ClientProfile>,
) -> Value {
    match client {
        ModelClient::Antigravity => {
            value["modelProvider"] = json!("gemini");
            value["model"] = json!(p.default_model);
            if !value["customModelsConfig"].is_object() {
                value["customModelsConfig"] = json!({});
            }
            if !value["customModelsConfig"]["customModels"].is_object() {
                value["customModelsConfig"]["customModels"] = json!({});
            }
            if let Some(previous) = previous {
                let models = value["customModelsConfig"]["customModels"]
                    .as_object_mut()
                    .expect("model map");
                for model in &previous.models {
                    if !p.models.iter().any(|m| m.id == model.id) {
                        models.remove(&model.id);
                    }
                }
            }
            for model in &p.models {
                value["customModelsConfig"]["customModels"][&model.id] = json!({
                    "modelId": model.id,
                    "modelName": model.id,
                    "label": model.name,
                    "maxTokens": model.context,
                });
            }
        }
        ModelClient::Muse => {
            if value.get("schema_version").is_none() {
                value["schema_version"] = json!(1);
            }
            value["model"] = json!(p.default_model);
            // New Muse releases honor this pin. Older releases use --base-url
            // in the same managed launcher, with no auth.json modification.
            value["endpoint_transport"] = json!({"base_url":base,"auth":"bearer"});
        }
        _ => {}
    }
    value
}

#[cfg(test)]
fn launcher(client: ModelClient, p: &ClientProfile, exe: &Path, base: &str) -> String {
    launcher_at(client, p, exe, base, &gateway_origin_path())
}

fn launcher_at(
    client: ModelClient,
    p: &ClientProfile,
    exe: &Path,
    base: &str,
    origin_path: &Path,
) -> String {
    let mut s =
        String::from("#!/bin/sh\n# Managed by CC Switch: model connection only.\nset -eu\n");
    let token = quote(&p.local_token);
    if matches!(client, ModelClient::Antigravity | ModelClient::Muse) {
        let suffix = url::Url::parse(base)
            .map(|url| url.path().trim_end_matches('/').to_string())
            .unwrap_or_else(|_| format!("/model-clients/{}", client.id()));
        let origin = base.strip_suffix(&suffix).unwrap_or(base);
        s.push_str(&format!("gateway_origin={}\nif [ -s {} ]; then IFS= read -r gateway_origin < {}; fi\ngateway_base=\"$gateway_origin{}\"\n",quote(origin),quote(&origin_path.to_string_lossy()),quote(&origin_path.to_string_lossy()),suffix));
    }
    match client {
        ModelClient::Antigravity => {
            // Google SDK otherwise prefers a process-wide GOOGLE_API_KEY.
            s.push_str("has_model=0\nfor arg do case \"$arg\" in --) break;; --model|--model=*) has_model=1;; esac; done\n");
            if !p.effort.is_empty() {
                s.push_str("has_effort=0\nfor arg do case \"$arg\" in --) break;; --effort|--effort=*) has_effort=1;; esac; done\n");
                s.push_str(&format!(
                    "if [ \"$has_effort\" = 0 ]; then set -- --effort {} \"$@\"; fi\n",
                    quote(if p.effort == "minimal" {
                        "low"
                    } else {
                        &p.effort
                    })
                ));
            }
            s.push_str(&format!("if [ \"$has_model\" = 0 ]; then set -- --model {} \"$@\"; fi\nunset GOOGLE_API_KEY\nexport GEMINI_API_KEY={token}\nexport GOOGLE_GEMINI_BASE_URL=\"$gateway_base\"\nexec {} \"$@\"\n",quote(&p.default_model),quote(&exe.to_string_lossy())));
        }
        ModelClient::Muse => {
            s.push_str(&format!("case \"${{1-}}\" in auth|login|logout|skills|sandbox|trace|export|session-message|init|--help|-h|--version|-V) exec {} \"$@\";; esac\n",quote(&exe.to_string_lossy())));
            s.push_str("headless=0\nif [ \"${1-}\" = exec ]; then headless=1; shift; fi\nhas_model=0\nhas_effort=0\nhas_base=0\nfor arg do case \"$arg\" in --) break;; --model|--model=*) has_model=1;; --reasoning-effort|--reasoning-effort=*) has_effort=1;; --base-url|--base-url=*) has_base=1;; esac; done\n");
            if !p.effort.is_empty() {
                s.push_str(&format!(
                    "if [ \"$has_effort\" = 0 ]; then set -- --reasoning-effort {} \"$@\"; fi\n",
                    quote(if p.effort == "max" {
                        "xhigh"
                    } else {
                        &p.effort
                    })
                ));
            }
            s.push_str(&format!("if [ \"$has_model\" = 0 ]; then set -- --model {} \"$@\"; fi\nif [ \"$has_base\" = 0 ]; then set -- --base-url \"$gateway_base\" \"$@\"; fi\nif [ \"$headless\" = 1 ]; then set -- exec \"$@\"; fi\nexport META_API_KEY={token}\nexec {} \"$@\"\n",quote(&p.default_model),quote(&exe.to_string_lossy())));
        }
        ModelClient::CursorAgent | ModelClient::Cursor => {}
    }
    s
}

fn cursor_patch(
    storage: &mut Value,
    p: &ClientProfile,
    previous: Option<&ClientProfile>,
) -> Result<(), String> {
    if !storage.is_object() {
        return Err("Cursor storage is not an object".into());
    }
    storage["openAIBaseUrl"] = json!(p.endpoint.trim_end_matches('/'));
    storage["useOpenAIKey"] = json!(true);
    if storage.get("aiSettings").is_none() {
        storage["aiSettings"] = json!({});
    }
    let ai = storage["aiSettings"]
        .as_object_mut()
        .ok_or("Invalid Cursor aiSettings")?;
    let added = ai
        .entry("userAddedModels")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or("Invalid Cursor model list")?;
    if let Some(previous) = previous {
        added.retain(|v| {
            !previous.models.iter().any(|m| v.as_str() == Some(&m.id))
                || p.models.iter().any(|m| v.as_str() == Some(&m.id))
        });
    }
    for model in &p.models {
        if !added.iter().any(|v| v.as_str() == Some(&model.id)) {
            added.push(json!(model.id));
        }
    }
    if let Some(disabled) = ai
        .get_mut("modelOverrideDisabled")
        .and_then(Value::as_array_mut)
    {
        disabled.retain(|v| !p.models.iter().any(|m| v.as_str() == Some(&m.id)));
    }
    let configs = ai
        .entry("modelConfig")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("Invalid Cursor model configuration")?;
    for feature in ["composer", "plan-execution"] {
        let selected = configs
            .entry(feature)
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or("Invalid Cursor feature model configuration")?;
        selected.insert("modelName".into(), json!(p.default_model));
        selected.insert(
            "selectedModels".into(),
            json!([{"modelId":p.default_model,"parameters":[]}]),
        );
        selected.insert("maxMode".into(), json!(false));
    }
    Ok(())
}

fn apply_cursor(
    path: &Path,
    p: &ClientProfile,
    previous: Option<&ClientProfile>,
    commit: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let key = resolve_key(p)?;
    if key.trim().is_empty() {
        return Err("Cursor BYOK requires this client's dedicated API key".into());
    }
    let mut conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|e| e.to_string())?;
    conn.busy_timeout(Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let raw: Option<String> = tx
        .query_row(
            "SELECT value FROM ItemTable WHERE key=?1",
            [CURSOR_STORAGE],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let mut storage: Value = serde_json::from_str(
        raw.as_deref()
            .ok_or("Cursor model storage was not found; open Cursor Settings once")?,
    )
    .map_err(|_| "Invalid Cursor model storage")?;
    cursor_patch(&mut storage, p, previous)?;
    tx.execute(
        "UPDATE ItemTable SET value=?2 WHERE key=?1",
        rusqlite::params![CURSOR_STORAGE, storage.to_string()],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT OR REPLACE INTO ItemTable(key,value) VALUES(?1,?2)",
        rusqlite::params!["cursorAuth/openAIKey", key],
    )
    .map_err(|e| e.to_string())?;
    commit()?;
    tx.commit().map_err(|e| e.to_string())
}

struct FileUpdate {
    path: PathBuf,
    bytes: Vec<u8>,
    executable: bool,
}

fn commit_files(
    updates: Vec<FileUpdate>,
    commit: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let mut snapshots = Vec::new();
    for update in &updates {
        let bytes = match fs::read(&update.path) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(format!("Cannot read {}: {e}", update.path.display())),
        };
        let permissions = fs::metadata(&update.path).ok().map(|m| m.permissions());
        snapshots.push((bytes, permissions));
    }
    let mut written = 0;
    let result = (|| {
        for update in &updates {
            atomic_write_private(&update.path, &update.bytes).map_err(|e| e.to_string())?;
            written += 1;
            #[cfg(unix)]
            if update.executable {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&update.path, fs::Permissions::from_mode(0o700))
                    .map_err(|e| e.to_string())?;
            }
        }
        commit()
    })();
    if let Err(error) = result {
        let mut failures = Vec::new();
        for (update, (before, permissions)) in updates.iter().zip(snapshots).take(written).rev() {
            // An external edit after our write must never be overwritten by rollback.
            if fs::read(&update.path).ok().as_ref() != Some(&update.bytes) {
                failures.push(format!("{} changed during rollback", update.path.display()));
                continue;
            }
            let restored = match before {
                Some(bytes) => {
                    atomic_write_private(&update.path, &bytes).map_err(|e| e.to_string())
                }
                None => fs::remove_file(&update.path).map_err(|e| e.to_string()),
            };
            if let Err(e) = restored {
                failures.push(e);
            } else if let Some(permissions) = permissions {
                if let Err(e) = fs::set_permissions(&update.path, permissions) {
                    failures.push(e.to_string());
                }
            }
        }
        return Err(if failures.is_empty() {
            error
        } else {
            format!("{error}; rollback failed: {}", failures.join("; "))
        });
    }
    Ok(())
}

pub fn apply(
    client: ModelClient,
    p: &ClientProfile,
    origin: &str,
    previous: Option<&ClientProfile>,
    commit: impl FnOnce() -> Result<(), String>,
) -> Result<ApplyResult, String> {
    apply_with_routing(
        client,
        p,
        previous,
        &format!("{origin}/model-clients/{}", client.id()),
        &super::config_path(),
        &gateway_origin_path(),
        commit,
    )
}

pub(crate) fn apply_deck(
    client: ModelClient,
    p: &ClientProfile,
    previous: Option<&ClientProfile>,
    commit: impl FnOnce() -> Result<(), String>,
) -> Result<ApplyResult, String> {
    apply_with_routing(
        client,
        p,
        previous,
        &p.endpoint,
        &crate::deck::config_path(),
        &get_app_config_dir().join("deck-gateway-url"),
        commit,
    )
}

fn apply_with_routing(
    client: ModelClient,
    p: &ClientProfile,
    previous: Option<&ClientProfile>,
    base: &str,
    profile_path: &Path,
    origin_path: &Path,
    commit: impl FnOnce() -> Result<(), String>,
) -> Result<ApplyResult, String> {
    if client == ModelClient::Cursor {
        apply_cursor(&native_config(client), p, previous, commit)?;
        return Ok(ApplyResult {
            command: None,
            restart_required: true,
        });
    }
    if client == ModelClient::CursorAgent {
        let native = executable(client, p)?;
        let runtime = cursor_runtime(&native)?;
        let directory = get_app_config_dir().join("adapters");
        let adapter_path = directory.join(if runtime.local_api {
            "cursor-local.cjs"
        } else {
            "cursor-models.cjs"
        });
        let launch_path = get_app_config_dir().join("bin/cursor-agent");
        let script = if runtime.local_api {
            cursor_local_launcher(
                &runtime,
                profile_path,
                origin_path,
                &adapter_path,
                &p.default_model,
            )
        } else {
            format!("#!/bin/sh\n# Managed by ModelDeck: Cursor model catalog and BYOK only.\nset -eu\ncursor_native={}\nif command -v realpath >/dev/null 2>&1; then cursor_runtime_dir=$(dirname \"$(realpath \"$cursor_native\")\"); else cursor_runtime_dir=$(dirname \"$(readlink \"$cursor_native\" || printf '%s' \"$cursor_native\")\"); fi\nexport CCSWITCH_CURSOR_PROFILE_FILE={}\nexport CURSOR_INVOKED_AS=cursor-agent\nhas_model=0\nfor arg do case \"$arg\" in --) break;; --model|--model=*) has_model=1;; esac; done\nif [ \"$has_model\" = 0 ]; then set -- --model {} \"$@\"; fi\nexec \"$cursor_runtime_dir/node\" --use-system-ca --require {} \"$cursor_runtime_dir/index.js\" \"$@\"\n",quote(&native.to_string_lossy()),quote(&profile_path.to_string_lossy()),quote(&p.default_model),quote(&adapter_path.to_string_lossy()))
        };
        commit_files(
            vec![
                FileUpdate {
                    path: adapter_path,
                    bytes: if runtime.local_api {
                        CURSOR_LOCAL_ADAPTER
                    } else {
                        CURSOR_ADAPTER
                    }
                    .as_bytes()
                    .to_vec(),
                    executable: false,
                },
                FileUpdate {
                    path: directory.join("cursor-models.manifest.json"),
                    bytes: CURSOR_MANIFEST.as_bytes().to_vec(),
                    executable: false,
                },
                FileUpdate {
                    path: launch_path.clone(),
                    bytes: script.into_bytes(),
                    executable: true,
                },
            ],
            commit,
        )?;
        return Ok(ApplyResult {
            command: Some(quote(&launch_path.to_string_lossy())),
            restart_required: false,
        });
    }
    let exe = executable(client, p)?;
    let path = native_config(client);
    let settings = patch_settings(client, read_json(&path)?, p, base, previous);
    let launch_path = get_app_config_dir().join("bin").join(client.id());
    let bytes = serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?;
    commit_files(
        vec![
            FileUpdate {
                path,
                bytes,
                executable: false,
            },
            FileUpdate {
                path: launch_path.clone(),
                bytes: launcher_at(client, p, &exe, base, origin_path).into_bytes(),
                executable: true,
            },
        ],
        commit,
    )?;
    Ok(ApplyResult {
        command: Some(quote(&launch_path.to_string_lossy())),
        restart_required: false,
    })
}

fn reset_settings(client: ModelClient, mut value: Value, p: &ClientProfile) -> Value {
    let owns = |id: Option<&str>| id.is_some_and(|id| p.models.iter().any(|model| model.id == id));
    if client == ModelClient::Antigravity {
        if value.get("modelProvider").and_then(Value::as_str) == Some("gemini") {
            value.as_object_mut().unwrap().remove("modelProvider");
        }
        if owns(value.get("model").and_then(Value::as_str)) {
            value.as_object_mut().unwrap().remove("model");
        }
        if let Some(models) = value
            .pointer_mut("/customModelsConfig/customModels")
            .and_then(Value::as_object_mut)
        {
            for model in &p.models {
                models.remove(&model.id);
            }
        }
    } else if client == ModelClient::Muse {
        if value
            .pointer("/endpoint_transport/base_url")
            .and_then(Value::as_str)
            .is_some_and(|s| s.ends_with("/model-clients/muse"))
        {
            value.as_object_mut().unwrap().remove("endpoint_transport");
        }
        if owns(value.get("model").and_then(Value::as_str)) {
            value.as_object_mut().unwrap().remove("model");
        }
    } else if client == ModelClient::CursorAgent {
        if owns(value.pointer("/model/modelId").and_then(Value::as_str)) {
            value["model"] = json!({"modelId":"default","displayModelId":"auto","displayName":"Auto","displayNameShort":"Auto","aliases":["auto"],"maxMode":false});
        }
        if owns(
            value
                .pointer("/selectedModel/modelId")
                .and_then(Value::as_str),
        ) {
            value.as_object_mut().unwrap().remove("selectedModel");
        }
    }
    value
}

pub fn reset(
    client: ModelClient,
    p: &ClientProfile,
    commit: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let path = native_config(client);
    if !path.is_file() {
        return commit();
    }
    if client == ModelClient::Cursor {
        ensure_cursor_closed()?;
        let mut conn =
            Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
                .map_err(|e| e.to_string())?;
        conn.busy_timeout(Duration::from_secs(3))
            .map_err(|e| e.to_string())?;
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let raw: String = tx
            .query_row(
                "SELECT value FROM ItemTable WHERE key=?1",
                [CURSOR_STORAGE],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let mut v: Value =
            serde_json::from_str(&raw).map_err(|_| "Invalid Cursor model storage")?;
        v["useOpenAIKey"] = json!(false);
        if v.get("openAIBaseUrl").and_then(Value::as_str) == Some(p.endpoint.trim_end_matches('/'))
        {
            v["openAIBaseUrl"] = Value::Null;
        }
        if let Some(models) = v
            .pointer_mut("/aiSettings/userAddedModels")
            .and_then(Value::as_array_mut)
        {
            models.retain(|id| !p.models.iter().any(|m| id.as_str() == Some(&m.id)));
        }
        for feature in ["composer", "plan-execution"] {
            if let Some(config) = v.pointer_mut(&format!("/aiSettings/modelConfig/{feature}")) {
                if p.models
                    .iter()
                    .any(|m| config.get("modelName").and_then(Value::as_str) == Some(&m.id))
                {
                    *config = json!({"modelName":"default","maxMode":false,"selectedModels":[{"modelId":"default","parameters":[]}]});
                }
            }
        }
        tx.execute(
            "UPDATE ItemTable SET value=?2 WHERE key=?1",
            rusqlite::params![CURSOR_STORAGE, v.to_string()],
        )
        .map_err(|e| e.to_string())?;
        commit()?;
        return tx.commit().map_err(|e| e.to_string());
    }
    let value = reset_settings(client, read_json(&path)?, p);
    commit_files(
        vec![FileUpdate {
            path,
            bytes: serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
            executable: false,
        }],
        commit,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> ClientProfile {
        serde_json::from_value(json!({"endpoint":"https://openrouter.ai/api/v1","protocol":"openai-chat","models":[{"id":"vendor/model","name":"Model","context":1000000,"output":10000}],"defaultModel":"vendor/model","localToken":"local-only-test-token"})).unwrap()
    }
    #[test]
    fn patch_preserves_non_model_settings_and_auth() {
        let p = profile();
        let original = json!({"tui":{"theme":"dark"},"mcp":{"keep":true},"auth":{"keep":true}});
        for c in [ModelClient::Antigravity, ModelClient::Muse] {
            let updated = patch_settings(
                c,
                original.clone(),
                &p,
                "http://127.0.0.1:15721/model-clients/muse",
                None,
            );
            for k in ["tui", "mcp", "auth"] {
                assert_eq!(updated[k], original[k]);
            }
        }
        let mut v = json!({"cursorCreds":{"keep":true},"aiSettings":{"userAddedModels":["old"],"modelOverrideDisabled":["old","vendor/model"]},"noStorageMode":true});
        cursor_patch(&mut v, &p, None).unwrap();
        assert_eq!(v["cursorCreds"], json!({"keep":true}));
        assert_eq!(
            v["aiSettings"]["userAddedModels"],
            json!(["old", "vendor/model"])
        );
        assert_eq!(v["aiSettings"]["modelOverrideDisabled"], json!(["old"]));
        cursor_patch(&mut v, &p, None).unwrap();
        assert_eq!(
            v["aiSettings"]["userAddedModels"].as_array().unwrap().len(),
            2
        );
    }
    #[test]
    fn launcher_quotes_values_and_never_contains_upstream_key() {
        let mut p = profile();
        p.api_key = "real-upstream-secret-test".into();
        p.default_model = "model'; touch /tmp/never".into();
        let script = launcher(
            ModelClient::Muse,
            &p,
            Path::new("/test/muse"),
            "http://127.0.0.1:15721/model-clients/muse",
        );
        assert!(!script.contains(&p.api_key));
        assert!(script.contains("'model'\"'\"'; touch /tmp/never'"));
        assert!(script.contains("\"$@\""));
    }
    #[test]
    fn local_cursor_launcher_keeps_keys_out_of_arguments_and_native_auth() {
        let runtime = CursorRuntime {
            directory: PathBuf::from("/vendor/Cursor Local"),
            version: "verified-local".into(),
            local_api: true,
        };
        let script = cursor_local_launcher(
            &runtime,
            Path::new("/private/model-deck.json"),
            Path::new("/private/deck-gateway-url"),
            Path::new("/private/cursor-local.cjs"),
            "model'; touch /tmp/never",
        );
        assert!(script.contains("--authless --disable-auto-update"));
        assert!(script.contains("'/vendor/Cursor Local/node'"));
        assert!(script.contains("'model'\"'\"'; touch /tmp/never'"));
        assert!(script.contains("MODELDECK_CURSOR_ORIGIN_FILE"));
        assert!(!script.contains("--local-agent-api-key"));
        assert!(!script.contains("CURSOR_API_KEY"));
        assert!(!script.contains("cli-config.json"));
    }
    #[test]
    fn failed_activation_restores_native_settings_and_launchers() {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join("settings.json");
        let script = dir.path().join("launch");
        fs::write(&settings, b"{\"theme\":\"keep\"}").unwrap();
        let result = commit_files(
            vec![
                FileUpdate {
                    path: settings.clone(),
                    bytes: b"{\"model\":\"new\"}".to_vec(),
                    executable: false,
                },
                FileUpdate {
                    path: script.clone(),
                    bytes: b"#!/bin/sh\nexit 0\n".to_vec(),
                    executable: true,
                },
            ],
            || Err("simulated store failure".into()),
        );
        assert!(result.is_err());
        assert_eq!(fs::read(&settings).unwrap(), b"{\"theme\":\"keep\"}");
        assert!(!script.exists());
    }
    #[cfg(unix)]
    #[test]
    fn launchers_keep_headless_subcommands_overrides_and_dynamic_gateway_ports() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let native = dir.path().join("native cli");
        let script = dir.path().join("launch");
        let origin = dir.path().join("origin");
        fs::write(&native, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        fs::set_permissions(&native, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(&origin, "http://127.0.0.1:19999\n").unwrap();
        let mut p = profile();
        p.effort = "max".into();
        fs::write(
            &script,
            launcher_at(
                ModelClient::Muse,
                &p,
                &native,
                "http://127.0.0.1:15721/model-clients/muse",
                &origin,
            ),
        )
        .unwrap();
        let output = std::process::Command::new("sh")
            .arg(&script)
            .args(["exec", "--model", "override/model", "prompt with spaces"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        let args: Vec<_> = text.lines().collect();
        assert_eq!(args[0], "exec");
        assert!(args.contains(&"http://127.0.0.1:19999/model-clients/muse"));
        assert!(args.contains(&"override/model"));
        assert!(args.contains(&"xhigh"));
        assert_eq!(args.iter().filter(|a| **a == "--model").count(), 1);
        assert_eq!(args.last(), Some(&"prompt with spaces"));
    }
    #[test]
    fn reset_and_provider_switch_preserve_unmanaged_models_and_preferences() {
        let p = profile();
        let original = json!({"modelProvider":"gemini","model":p.default_model,"tui":{"theme":"keep"},"customModelsConfig":{"customModels":{"vendor/model":{"modelName":"vendor/model"},"user-model":{"modelName":"keep"}}}});
        let reset = reset_settings(ModelClient::Antigravity, original.clone(), &p);
        assert!(reset.get("modelProvider").is_none());
        assert_eq!(reset["tui"], original["tui"]);
        assert!(reset["customModelsConfig"]["customModels"]
            .get("vendor/model")
            .is_none());
        assert_eq!(
            reset["customModelsConfig"]["customModels"]["user-model"]["modelName"],
            "keep"
        );
        let mut next = p.clone();
        next.models[0].id = "new/model".into();
        next.default_model = "new/model".into();
        let switched = patch_settings(
            ModelClient::Antigravity,
            original,
            &next,
            "http://127.0.0.1:15721/model-clients/antigravity",
            Some(&p),
        );
        assert!(switched["customModelsConfig"]["customModels"]
            .get("vendor/model")
            .is_none());
        assert!(switched["customModelsConfig"]["customModels"]
            .get("new/model")
            .is_some());
        assert!(switched["customModelsConfig"]["customModels"]
            .get("user-model")
            .is_some());
    }
    #[test]
    fn cursor_store_failure_rolls_back_both_native_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.vscdb");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE ItemTable(key TEXT PRIMARY KEY,value TEXT);")
            .unwrap();
        let old = json!({"aiSettings":{},"cursorCreds":{"keep":true}}).to_string();
        conn.execute(
            "INSERT INTO ItemTable VALUES(?1,?2)",
            rusqlite::params![CURSOR_STORAGE, old],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ItemTable VALUES('cursorAuth/openAIKey','old-test-key')",
            [],
        )
        .unwrap();
        let mut p = profile();
        p.api_key = "new-test-key".into();
        assert!(apply_cursor(&path, &p, None, || Err(
            "simulated persistence failure".into()
        ))
        .is_err());
        let actual: String = conn
            .query_row(
                "SELECT value FROM ItemTable WHERE key=?1",
                [CURSOR_STORAGE],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(actual, old);
        let actual: String = conn
            .query_row(
                "SELECT value FROM ItemTable WHERE key='cursorAuth/openAIKey'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(actual, "old-test-key");
    }
    #[test]
    fn cursor_transaction_only_updates_byok_and_model_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.vscdb");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE ItemTable(key TEXT PRIMARY KEY,value TEXT);")
            .unwrap();
        conn.execute(
            "INSERT INTO ItemTable VALUES(?1,?2)",
            rusqlite::params![
                CURSOR_STORAGE,
                json!({"aiSettings":{},"cursorCreds":{"keep":true}}).to_string()
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ItemTable VALUES('cursorAuth/accessToken','unchanged')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ItemTable VALUES('cursorAuth/stripeSubscriptionStatus','unchanged')",
            [],
        )
        .unwrap();
        let mut p = profile();
        p.api_key = "dedicated-cursor-test-key".into();
        apply_cursor(&path, &p, None, || Ok(())).unwrap();
        for key in [
            "cursorAuth/accessToken",
            "cursorAuth/stripeSubscriptionStatus",
        ] {
            let v: String = conn
                .query_row("SELECT value FROM ItemTable WHERE key=?1", [key], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(v, "unchanged");
        }
    }
}
