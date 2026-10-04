//! Model-only integrations. These clients intentionally do not participate in
//! the skill, prompt, MCP, OAuth, or subscription management code paths.
pub(crate) mod gateway;
mod native;

use crate::config::{atomic_write_private, get_app_config_dir};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    sync::Mutex,
};

static CONFIG_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelClient {
    Antigravity,
    Cursor,
    CursorAgent,
    Muse,
}

impl ModelClient {
    pub fn id(self) -> &'static str {
        match self {
            Self::Antigravity => "antigravity",
            Self::Cursor => "cursor",
            Self::CursorAgent => "cursor-agent",
            Self::Muse => "muse",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientModel {
    pub id: String,
    pub name: String,
    pub context: u64,
    pub output: u64,
    #[serde(default)]
    pub reasoning: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_supported: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientProfile {
    #[serde(default)]
    pub connection_name: String,
    pub endpoint: String,
    /// Supported wire API, not the branding of the provider.
    pub protocol: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub key_env: String,
    pub models: Vec<ClientModel>,
    pub default_model: String,
    #[serde(default)]
    pub utility_model: String,
    #[serde(default)]
    pub effort: String,
    /// An explicitly selected native executable; never replaced by an adapter.
    #[serde(default)]
    pub executable: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub local_token: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientStore {
    #[serde(default)]
    pub profiles: BTreeMap<ModelClient, ClientProfile>,
    #[serde(default)]
    pub active: BTreeMap<ModelClient, ClientProfile>,
    #[serde(default)]
    pub saved_profiles: BTreeMap<ModelClient, BTreeMap<String, ClientProfile>>,
}

pub(crate) fn config_path() -> PathBuf {
    get_app_config_dir().join("model-clients.json")
}

pub(crate) fn needs_gateway() -> bool {
    read_store().is_ok_and(|store| {
        [ModelClient::Antigravity, ModelClient::Muse]
            .iter()
            .any(|client| {
                store
                    .active
                    .get(client)
                    .is_some_and(|profile| profile.enabled)
            })
    })
}

pub(crate) fn publish_gateway_origin(address: &str, port: u16) -> Result<(), String> {
    let host = match address {
        "0.0.0.0" => "127.0.0.1",
        "::" => "::1",
        other => other,
    };
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    native::publish_gateway_origin(&format!("http://{host}:{port}"))
}

pub(crate) fn read_store() -> Result<ClientStore, String> {
    match std::fs::read(config_path()) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|_| "Invalid model client configuration".into())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ClientStore::default()),
        Err(e) => Err(format!("Cannot read model client configuration: {e}")),
    }
}

fn write_store(store: &ClientStore) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(store).map_err(|e| e.to_string())?;
    atomic_write_private(&config_path(), &bytes).map_err(|e| e.to_string())
}

pub(crate) fn validate(client: ModelClient, p: &ClientProfile) -> Result<(), String> {
    if p.connection_name.chars().count() > 80 || p.connection_name.contains(['\r', '\n']) {
        return Err("Use a connection name of at most 80 characters".into());
    }
    let url = url::Url::parse(&p.endpoint).map_err(|_| "Enter a valid endpoint URL")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Use an HTTP(S) endpoint without credentials, query, or fragment".into());
    }
    let protocol_supported = match client {
        ModelClient::Muse => matches!(
            p.protocol.as_str(),
            "openai-responses" | "openai-chat" | "anthropic"
        ),
        ModelClient::Antigravity => matches!(p.protocol.as_str(), "gemini" | "openai-chat"),
        ModelClient::Cursor | ModelClient::CursorAgent => p.protocol == "openai-chat",
    };
    if !protocol_supported {
        return Err("Unsupported API protocol for this client".into());
    }
    if matches!(client, ModelClient::Cursor | ModelClient::CursorAgent) {
        let host = url.host_str().unwrap_or_default().trim_end_matches('.');
        if url.scheme() != "https"
            || host == "localhost"
            || host.ends_with(".localhost")
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| match ip {
                    std::net::IpAddr::V4(ip) => {
                        ip.is_private()
                            || ip.is_loopback()
                            || ip.is_link_local()
                            || ip.is_unspecified()
                    }
                    std::net::IpAddr::V6(ip) => {
                        ip.is_loopback()
                            || ip.is_unspecified()
                            || ip.is_unique_local()
                            || ip.is_unicast_link_local()
                            || ip.to_ipv4_mapped().is_some_and(|ip| {
                                ip.is_private()
                                    || ip.is_loopback()
                                    || ip.is_link_local()
                                    || ip.is_unspecified()
                            })
                    }
                })
        {
            return Err("Cursor cloud BYOK requires a publicly reachable HTTPS endpoint".into());
        }
    }
    if !p.key_env.is_empty()
        && (!p
            .key_env
            .chars()
            .enumerate()
            .all(|(i, c)| c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())))
    {
        return Err("Invalid API key environment variable name".into());
    }
    if !p.api_key.is_empty() && !p.key_env.is_empty() {
        return Err("Choose either a stored key or an environment variable".into());
    }
    let mut ids = HashSet::new();
    for m in &p.models {
        if matches!(client, ModelClient::Cursor | ModelClient::CursorAgent)
            && (m.id.starts_with("claude-") || m.id.starts_with("gemini-"))
        {
            return Err("Cursor reserves bare claude-* and gemini-* IDs for native provider slots. Use the provider-qualified OpenAI-compatible ID (for example anthropic/claude-opus-5.5).".into());
        }
        if m.id.trim().is_empty()
            || m.name.trim().is_empty()
            || m.id.contains(['\n', '\r', '?', '#'])
            || !ids.insert(&m.id)
        {
            return Err("Model IDs and names must be non-empty and IDs must be unique".into());
        }
        if m.context == 0 || m.output == 0 || m.output > m.context {
            return Err(format!("Invalid context/output limit for {}", m.id));
        }
        if m.reasoning.iter().any(|e| {
            !matches!(
                e.as_str(),
                "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra"
            )
        }) {
            return Err("Invalid reasoning level".into());
        }
    }
    let model = p
        .models
        .iter()
        .find(|m| m.id == p.default_model)
        .ok_or("Select a default model from the catalog")?;
    if !p.utility_model.is_empty() && !p.models.iter().any(|m| m.id == p.utility_model) {
        return Err("Select the auxiliary model from this connection's catalog".into());
    }
    if !p.effort.is_empty() && !model.reasoning.contains(&p.effort) {
        return Err("The selected model does not support this reasoning level".into());
    }
    Ok(())
}

pub(crate) fn resolve_key(p: &ClientProfile) -> Result<String, String> {
    if !p.key_env.is_empty() {
        std::env::var(&p.key_env).ok().filter(|v| !v.trim().is_empty())
            .ok_or_else(|| format!("{} is not available in CC Switch's environment. Enter this client's dedicated key, or launch CC Switch with that variable set.", p.key_env))
    } else {
        Ok(p.api_key.clone())
    }
}

#[tauri::command]
pub fn get_model_clients() -> Result<ClientStore, String> {
    read_store()
}

#[tauri::command]
pub fn save_model_client(client: ModelClient, mut profile: ClientProfile) -> Result<(), String> {
    profile.connection_name = profile.connection_name.trim().to_string();
    validate(client, &profile)?;
    let _lock = CONFIG_LOCK
        .lock()
        .map_err(|_| "Model client configuration lock failed")?;
    let mut store = read_store()?;
    // Runtime token/activation are never accepted from renderer input.
    let previous = store.profiles.get(&client);
    profile.local_token = previous
        .map(|p| p.local_token.clone())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    profile.enabled = previous.is_some_and(|p| p.enabled);
    if !profile.connection_name.is_empty() {
        store
            .saved_profiles
            .entry(client)
            .or_default()
            .insert(profile.connection_name.clone(), profile.clone());
    }
    store.profiles.insert(client, profile);
    write_store(&store)
}

#[tauri::command]
pub async fn apply_model_client(
    client: ModelClient,
    state: tauri::State<'_, crate::store::AppState>,
) -> Result<native::ApplyResult, String> {
    // Preflight before starting the server or changing a native file.
    let mut profile = read_store()?
        .profiles
        .get(&client)
        .cloned()
        .ok_or("Save a profile first")?;
    validate(client, &profile)?;
    native::preflight(client, &profile)?;
    let origin = if matches!(client, ModelClient::Cursor | ModelClient::CursorAgent) {
        String::new()
    } else {
        let info = state.proxy_service.start().await?;
        publish_gateway_origin(&info.address, info.port)?;
        format!("http://127.0.0.1:{}", info.port)
    };
    let _lock = CONFIG_LOCK
        .lock()
        .map_err(|_| "Model client configuration lock failed")?;
    let previous = read_store()?;
    if previous.profiles.get(&client) != Some(&profile) {
        return Err("The model connection changed while applying; apply it again".into());
    }
    let mut store = previous.clone();
    profile.enabled = true;
    store.profiles.insert(client, profile.clone());
    store.active.insert(client, profile.clone());
    let mut persisted = false;
    let result = native::apply(
        client,
        &profile,
        &origin,
        previous.active.get(&client),
        || {
            write_store(&store)?;
            persisted = true;
            Ok(())
        },
    );
    if result.is_err() && persisted {
        if let Err(rollback) = write_store(&previous) {
            return Err(format!(
                "Native update failed; restoring the active connection also failed: {rollback}"
            ));
        }
    }
    result
}

#[tauri::command]
pub fn reset_model_client(client: ModelClient) -> Result<(), String> {
    let _lock = CONFIG_LOCK
        .lock()
        .map_err(|_| "Model client configuration lock failed")?;
    let previous = read_store()?;
    let Some(profile) = previous.active.get(&client) else {
        return Ok(());
    };
    let mut store = previous.clone();
    store.active.remove(&client);
    if let Some(draft) = store.profiles.get_mut(&client) {
        draft.enabled = false;
    }
    let mut persisted = false;
    let result = native::reset(client, profile, || {
        write_store(&store)?;
        persisted = true;
        Ok(())
    });
    if result.is_err() && persisted {
        write_store(&previous)?;
    }
    result
}

#[tauri::command]
pub fn inspect_model_clients() -> Vec<native::ClientInspection> {
    native::inspect()
}

#[tauri::command]
pub fn import_model_client_catalog(
    provider_id: String,
    state: tauri::State<'_, crate::store::AppState>,
) -> Result<Vec<ClientModel>, String> {
    // Read model metadata only. Keys and provider activation never cross clients.
    let providers = state
        .db
        .get_all_providers("opencode")
        .map_err(|e| e.to_string())?;
    let mut models = indexmap::IndexMap::new();
    for provider in providers.values().filter(|p| p.id == provider_id) {
        if let Some(catalog) = provider
            .settings_config
            .get("models")
            .and_then(serde_json::Value::as_object)
        {
            for (id, m) in catalog {
                let context = m
                    .pointer("/limit/context")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| format!("Missing context limit for {id}"))?;
                let output = m
                    .pointer("/limit/output")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| format!("Missing output limit for {id}"))?;
                let mut reasoning: Vec<String> = m
                    .get("variants")
                    .and_then(serde_json::Value::as_object)
                    .map(|v| {
                        v.keys()
                            .filter(|k| {
                                matches!(
                                    k.as_str(),
                                    "none"
                                        | "minimal"
                                        | "low"
                                        | "medium"
                                        | "high"
                                        | "xhigh"
                                        | "max"
                                        | "ultra"
                                )
                            })
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(configured) = m
                    .pointer("/options/reasoningEffort")
                    .or_else(|| m.pointer("/options/reasoning_effort"))
                    .or_else(|| m.pointer("/options/reasoning/effort"))
                    .and_then(serde_json::Value::as_str)
                {
                    if matches!(
                        configured,
                        "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra"
                    ) && !reasoning.iter().any(|level| level == configured)
                    {
                        reasoning.push(configured.to_string());
                    }
                }
                models.entry(id.clone()).or_insert(ClientModel {
                    id: id.clone(),
                    name: m
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or(id)
                        .into(),
                    context,
                    output,
                    reasoning,
                    reasoning_supported: m.get("reasoning").and_then(serde_json::Value::as_bool),
                });
            }
        }
    }
    Ok(models.into_values().collect())
}

#[tauri::command]
pub fn get_model_client_catalog_sources(
    state: tauri::State<'_, crate::store::AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let providers = state
        .db
        .get_all_providers("opencode")
        .map_err(|e| e.to_string())?;
    Ok(providers
        .values()
        .filter_map(|p| {
            p.settings_config
                .get("models")
                .and_then(serde_json::Value::as_object)
                .map(|m| serde_json::json!({"id":p.id,"name":p.name,"count":m.len()}))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> ClientProfile {
        serde_json::from_value(serde_json::json!({"endpoint":"https://openrouter.ai/api/v1","protocol":"openai-chat","models":[{"id":"a/model","name":"Model","context":1048576,"output":32768,"reasoning":["high","max"]}],"defaultModel":"a/model","effort":"max"})).unwrap()
    }
    #[test]
    fn validates_limits_ids_and_effort() {
        let mut p = profile();
        assert!(validate(ModelClient::Antigravity, &p).is_ok());
        p.models.push(p.models[0].clone());
        assert!(validate(ModelClient::Antigravity, &p).is_err());
        p.models.pop();
        p.models[0].output = 2000000;
        assert!(validate(ModelClient::Antigravity, &p).is_err());
        p.models[0].output = 100;
        p.effort = "xhigh".into();
        assert!(validate(ModelClient::Antigravity, &p).is_err());
    }
    #[test]
    fn rejects_private_cursor_endpoint_and_ambiguous_keys() {
        let mut p = profile();
        for endpoint in [
            "https://127.0.0.1/v1",
            "https://[::1]/v1",
            "https://localhost./v1",
        ] {
            p.endpoint = endpoint.into();
            assert!(validate(ModelClient::Cursor, &p).is_err());
        }
        p = profile();
        p.key_env = "MUSE_KEY".into();
        p.api_key = "dedicated-test-key".into();
        assert!(validate(ModelClient::Muse, &p).is_err());
    }
}
