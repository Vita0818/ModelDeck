//! Model-only integrations. These clients intentionally do not participate in
//! the skill, prompt, MCP, OAuth, or subscription management code paths.
pub(crate) mod gateway;
pub(crate) mod native;
pub(crate) mod provider_adapter;

use crate::config::get_app_config_dir;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::collections::HashSet;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelClient {
    Antigravity,
    Cursor,
    CursorAgent,
    Muse,
}

impl ModelClient {
    pub fn try_from_id(id: &str) -> Option<Self> {
        Some(match id {
            "antigravity" => Self::Antigravity,
            "cursor" => Self::Cursor,
            "cursor-agent" => Self::CursorAgent,
            "muse" => Self::Muse,
            _ => return None,
        })
    }
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
    pub images: bool,
    #[serde(default)]
    pub reasoning: Vec<String>,
    /// Default from the selected resource route, used only in native projections.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub default_effort: String,
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
    /// Upstream adapter metadata, independent of native harness settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_meta: Option<serde_json::Value>,
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

#[cfg(test)]
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
    if client == ModelClient::Cursor
        || (client == ModelClient::CursorAgent && !native::cursor_agent_is_local(&p.executable))
    {
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
            .ok_or_else(|| format!("{} is not available in ModelDeck's environment. Enter this client's dedicated key, or launch ModelDeck with that variable set.", p.key_env))
    } else {
        Ok(p.api_key.clone())
    }
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
