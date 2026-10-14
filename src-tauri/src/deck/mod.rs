//! ModelDeck's two independent layers: upstream resources and public routes.
//! Harnesses store references into the graph; native catalogs are projections.
mod credentials;
pub(crate) mod gateway;
mod migration;
mod native;
mod reasoning;
mod types;

use crate::config::{atomic_write_private, get_app_config_dir};
use crate::model_clients::{ClientModel, ClientProfile};
use crate::store::AppState;
use std::{collections::HashSet, path::PathBuf, sync::Mutex};
pub use types::*;

static STORE_LOCK: Mutex<()> = Mutex::new(());
pub const CLIENTS: [&str; 10] = [
    "codex",
    "claude",
    "claude-desktop",
    "antigravity",
    "cursor-agent",
    "cursor",
    "grokbuild",
    "muse",
    "opencode",
    "hermes",
];
pub const LEVELS: [&str; 8] = [
    "none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra",
];

pub fn config_path() -> PathBuf {
    get_app_config_dir().join("model-deck.json")
}

pub fn read_store() -> Result<DeckStore, String> {
    let bytes = std::fs::read(config_path())
        .map_err(|e| format!("Cannot read ModelDeck resources: {e}"))?;
    let store: DeckStore =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid ModelDeck resource store")?;
    if store.schema_version != 1 {
        return Err("Unsupported ModelDeck resource schema".into());
    }
    Ok(store)
}

fn write_store(store: &DeckStore) -> Result<(), String> {
    atomic_write_private(
        &config_path(),
        &serde_json::to_vec_pretty(store).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub fn needs_gateway() -> bool {
    read_store().is_ok_and(|s| !s.active_bindings.is_empty())
}

pub fn binding_active(client: &str) -> Result<bool, String> {
    if !config_path().exists() {
        return Ok(false);
    }
    Ok(read_store()?.active_bindings.contains_key(client))
}

pub fn client_protocol(client: &str) -> Option<Protocol> {
    Some(match client {
        "codex" | "grokbuild" | "muse" => Protocol::OpenaiResponses,
        "claude" | "claude-desktop" => Protocol::Anthropic,
        "antigravity" => Protocol::Gemini,
        "cursor" | "cursor-agent" | "opencode" | "hermes" => Protocol::OpenaiChat,
        _ => return None,
    })
}

pub fn client_protocols(client: &str) -> Vec<Protocol> {
    if matches!(client, "opencode" | "hermes") {
        vec![
            Protocol::Anthropic,
            Protocol::OpenaiChat,
            Protocol::OpenaiResponses,
        ]
    } else {
        client_protocol(client).into_iter().collect()
    }
}

fn binding_protocol(
    store: &DeckStore,
    client: &str,
    binding: &HarnessBinding,
) -> Result<Protocol, String> {
    let endpoint = store
        .endpoints
        .get(&binding.endpoint_id)
        .ok_or("Endpoint is unavailable")?;
    let route = endpoint
        .routes
        .get(&binding.model)
        .ok_or("Model route is unavailable")?;
    let model = store
        .models
        .get(&route.model_id)
        .ok_or("Model resource is unavailable")?;
    let provider = store
        .providers
        .get(&model.provider_id)
        .ok_or("Provider is unavailable")?;
    let candidates = binding
        .input_protocol
        .map(|p| vec![p])
        .unwrap_or_else(|| client_protocols(client));
    candidates
        .into_iter()
        .find(|p| {
            client_protocols(client).contains(p)
                && endpoint.protocols.contains(p)
                && protocols_compatible(*p, provider.protocol)
        })
        .ok_or_else(|| "The harness protocol cannot call this model resource".into())
}

pub fn protocols_compatible(input: Protocol, upstream: Protocol) -> bool {
    input == upstream
        || match input {
            Protocol::OpenaiResponses => {
                matches!(upstream, Protocol::OpenaiChat | Protocol::Anthropic)
            }
            Protocol::Anthropic => matches!(
                upstream,
                Protocol::OpenaiChat | Protocol::OpenaiResponses | Protocol::Gemini
            ),
            Protocol::Gemini => upstream == Protocol::OpenaiChat,
            Protocol::OpenaiChat => false,
        }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
}

pub fn validate_url(value: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(value).map_err(|_| "Enter a valid HTTP(S) URL")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Use an HTTP(S) URL without embedded credentials, query or fragment".into());
    }
    Ok(url)
}

fn public_url(value: &str) -> bool {
    validate_url(value).is_ok_and(|url| {
        let host = url.host_str().unwrap_or("").trim_matches(['[', ']']);
        url.scheme() == "https"
            && host != "localhost"
            && !host.ends_with(".localhost")
            && !host.parse::<std::net::IpAddr>().is_ok_and(|ip| match ip {
                std::net::IpAddr::V4(ip) => {
                    ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_unspecified()
                }
                std::net::IpAddr::V6(ip) => {
                    ip.is_loopback()
                        || ip.is_unique_local()
                        || ip.is_unicast_link_local()
                        || ip.is_unspecified()
                        || ip.to_ipv4_mapped().is_some_and(|ip| {
                            ip.is_private()
                                || ip.is_loopback()
                                || ip.is_link_local()
                                || ip.is_unspecified()
                        })
                }
            })
    })
}

pub fn compatible_models(
    store: &DeckStore,
    client: &str,
    endpoint: &ExposedEndpoint,
) -> Vec<String> {
    let inputs = client_protocols(client);
    endpoint
        .routes
        .iter()
        .filter_map(|(alias, route)| {
            let model = store.models.get(&route.model_id)?;
            let provider = store.providers.get(&model.provider_id)?;
            if !model.tools
                || !inputs.iter().any(|input| {
                    endpoint.protocols.contains(input)
                        && protocols_compatible(*input, provider.protocol)
                })
            {
                return None;
            }
            if matches!(client, "cursor" | "cursor-agent")
                && (alias.starts_with("claude-") || alias.starts_with("gemini-"))
            {
                return None;
            }
            if client == "claude-desktop"
                && !crate::claude_desktop_config::is_claude_safe_model_id(alias)
            {
                return None;
            }
            Some(alias.clone())
        })
        .collect()
}

fn requires_public_endpoint(client: &str, executable: &str) -> bool {
    client == "cursor"
        || (client == "cursor-agent"
            && !crate::model_clients::native::cursor_agent_is_local(executable))
}

fn compatibility(store: &DeckStore) -> Vec<Compatibility> {
    CLIENTS
        .iter()
        .flat_map(|client| {
            store.endpoints.values().map(move |endpoint| {
                let models = compatible_models(store, client, endpoint);
                let executable = store
                    .bindings
                    .get(*client)
                    .map(|b| b.executable.as_str())
                    .unwrap_or("");
                let issue = if requires_public_endpoint(client, executable)
                    && !public_url(&endpoint.public_url)
                {
                    Some(
                        "Cursor requires a public HTTPS address for this ModelDeck endpoint".into(),
                    )
                } else if models.is_empty() {
                    Some("No models compatible with this harness and endpoint".into())
                } else {
                    None
                };
                Compatibility {
                    client: client.to_string(),
                    endpoint_id: endpoint.id.clone(),
                    models,
                    issue,
                }
            })
        })
        .collect()
}

pub fn validate_binding(
    store: &DeckStore,
    client: &str,
    binding: &HarnessBinding,
    applying: bool,
) -> Result<(), String> {
    if !CLIENTS.contains(&client) {
        return Err("Unknown harness".into());
    }
    let endpoint = store
        .endpoints
        .get(&binding.endpoint_id)
        .ok_or("The selected endpoint no longer exists")?;
    let models = compatible_models(store, client, endpoint);
    let input = binding_protocol(store, client, binding)?;
    if !models.contains(&binding.model) {
        return Err(format!("The selected model is incompatible with {client}"));
    }
    if !binding.utility_model.is_empty() && !models.contains(&binding.utility_model) {
        return Err("The auxiliary model is incompatible with this harness".into());
    }
    if !binding.utility_model.is_empty() {
        let model = &store.models[&endpoint.routes[&binding.utility_model].model_id];
        if !protocols_compatible(input, store.providers[&model.provider_id].protocol) {
            return Err("The auxiliary model requires a different harness protocol".into());
        }
    }
    if applying
        && requires_public_endpoint(client, &binding.executable)
        && !public_url(&endpoint.public_url)
    {
        return Err(
            "Configure a public HTTPS address for the ModelDeck endpoint before applying Cursor"
                .into(),
        );
    }
    Ok(())
}

pub fn validate(store: &DeckStore) -> Result<(), String> {
    if store.schema_version != 1 {
        return Err("Unsupported ModelDeck resource schema".into());
    }
    for (id, provider) in &store.providers {
        if provider.provider_meta.as_ref().is_some_and(|meta| {
            serde_json::from_value::<crate::provider::ProviderMeta>(meta.clone()).is_err()
        }) {
            return Err("Invalid upstream provider adapter metadata".into());
        }
        if provider
            .key_source
            .as_deref()
            .is_some_and(|s| !matches!(s, "stored" | "env"))
            || provider.key_source.as_deref() == Some("env") && provider.key_env.is_empty()
        {
            return Err("Enter the credential environment variable name".into());
        }
        if provider.key_source.as_deref() == Some("stored") && !provider.key_env.is_empty() {
            return Err("Choose a single credential source".into());
        }
        if id != &provider.id || !valid_id(id) || provider.name.trim().is_empty() {
            return Err("Providers need a unique ID and a name".into());
        }
        validate_url(&provider.base_url)?;
        if !provider.api_key.is_empty() && !provider.key_env.is_empty() {
            return Err("Choose a stored key or an environment variable".into());
        }
        if !provider.key_env.is_empty()
            && !provider
                .key_env
                .chars()
                .enumerate()
                .all(|(i, c)| c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
        {
            return Err("Invalid API key environment variable name".into());
        }
    }
    for (id, model) in &store.models {
        if id != &model.id
            || !valid_id(id)
            || model.name.trim().is_empty()
            || model.upstream_id.trim().is_empty()
            || model.upstream_id.contains(['\r', '\n', '?', '#'])
        {
            return Err("Models need an ID, an upstream ID and a name".into());
        }
        if !store.providers.contains_key(&model.provider_id) {
            return Err("A model still references the provider being removed".into());
        }
        if model.context == 0
            || model.output == 0
            || model.output > model.context
            || model.context > i64::MAX as u64
        {
            return Err(format!("Invalid context/output limit for {}", model.name));
        }
        if model
            .reasoning
            .iter()
            .any(|level| !LEVELS.contains(&level.as_str()))
            || model.reasoning_supported == Some(false) && !model.reasoning.is_empty()
        {
            return Err(format!("Invalid reasoning levels for {}", model.name));
        }
    }
    for (id, endpoint) in &store.endpoints {
        if id != &endpoint.id || !valid_id(id) || endpoint.name.trim().is_empty() {
            return Err("Endpoints need an ID and a name".into());
        }
        if !endpoint.public_url.is_empty() {
            validate_url(&endpoint.public_url)?;
        }
        let mut protocols = HashSet::new();
        if endpoint.protocols.is_empty()
            || endpoint
                .protocols
                .iter()
                .any(|p| !protocols.insert(p.as_str()))
        {
            return Err("Select distinct endpoint protocols".into());
        }
        for (alias, route) in &endpoint.routes {
            if alias.trim().is_empty()
                || alias.len() > 256
                || alias.contains(['\r', '\n', '?', '#', '[', ']'])
            {
                return Err("Enter a valid public model name".into());
            }
            let model = store
                .models
                .get(&route.model_id)
                .ok_or("An endpoint still references the model being removed")?;
            if !route.effort.is_empty() && !model.reasoning.contains(&route.effort) {
                return Err(format!(
                    "The tier for {alias} is not supported by its model"
                ));
            }
            let provider = &store.providers[&model.provider_id];
            if provider.protocol == Protocol::Gemini
                && !route.effort.is_empty()
                && (!model
                    .upstream_id
                    .trim_start_matches("models/")
                    .starts_with("gemini-3")
                    || !matches!(route.effort.as_str(), "minimal" | "low" | "medium" | "high"))
            {
                return Err(format!("{alias}: this Gemini bridge requires a native Gemini 3 tier or the provider default"));
            }
            if !endpoint
                .protocols
                .iter()
                .any(|input| protocols_compatible(*input, provider.protocol))
            {
                return Err(format!(
                    "{alias} cannot use any protocol exposed by this endpoint"
                ));
            }
        }
    }
    for (client, binding) in store.bindings.iter().chain(&store.active_bindings) {
        validate_binding(store, client, binding, false)?;
    }
    Ok(())
}

pub fn resolve_key(provider: &UpstreamProvider) -> Result<String, String> {
    credentials::resolve(provider, &get_app_config_dir())
}

/// In-memory adapter projection. It is never saved as a second model catalog.
pub fn project_profile(
    store: &DeckStore,
    client: &str,
    binding: &HarnessBinding,
    origin: &str,
) -> Result<ClientProfile, String> {
    validate_binding(store, client, binding, true)?;
    let endpoint = &store.endpoints[&binding.endpoint_id];
    let input = binding_protocol(store, client, binding)?;
    let models = compatible_models(store, client, endpoint)
        .into_iter()
        .filter(|alias| {
            let model = &store.models[&endpoint.routes[alias].model_id];
            protocols_compatible(input, store.providers[&model.provider_id].protocol)
        })
        .map(|alias| {
            let route = &endpoint.routes[&alias];
            let model = &store.models[&route.model_id];
            ClientModel {
                id: alias.clone(),
                name: alias,
                context: model.context,
                output: model.output,
                reasoning: model.reasoning.clone(),
                default_effort: route.effort.clone(),
                reasoning_supported: model.reasoning_supported,
            }
        })
        .collect();
    Ok(ClientProfile {
        connection_name: endpoint.name.clone(),
        endpoint: if requires_public_endpoint(client, &binding.executable) {
            format!("{}/v1", endpoint.public_url.trim_end_matches('/'))
        } else {
            format!("{origin}/deck-clients/{client}")
        },
        protocol: input.as_str().into(),
        provider_meta: None,
        api_key: endpoint.token.clone(),
        key_env: String::new(),
        models,
        default_model: binding.model.clone(),
        utility_model: binding.utility_model.clone(),
        effort: endpoint.routes[&binding.model].effort.clone(),
        executable: binding.executable.clone(),
        enabled: true,
        local_token: endpoint.token.clone(),
    })
}

pub fn origin() -> String {
    std::fs::read_to_string(get_app_config_dir().join("deck-gateway-url"))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "http://127.0.0.1:15721".into())
}

pub fn publish_origin(address: &str, port: u16) -> Result<(), String> {
    let host = match address {
        "0.0.0.0" => "127.0.0.1",
        "::" => "[::1]",
        other => other,
    };
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.into()
    };
    atomic_write_private(
        &get_app_config_dir().join("deck-gateway-url"),
        format!("http://{host}:{port}\n").as_bytes(),
    )
    .map_err(|e| e.to_string())
}

fn view(store: DeckStore) -> DeckView {
    DeckView {
        compatibility: compatibility(&store),
        store,
        origin: origin(),
    }
}

pub(crate) fn import_model_providers(
    db: &crate::database::Database,
    client: &str,
    providers: &[crate::provider::Provider],
    select: bool,
) -> Result<Vec<String>, String> {
    if !CLIENTS.contains(&client) {
        return Err("This client is outside ModelDeck's supported scope".into());
    }
    let _lock = STORE_LOCK
        .lock()
        .map_err(|_| "Resource store lock failed")?;
    let mut store = if config_path().exists() {
        read_store()?
    } else {
        migration::import(db)?
    };
    let mut imported = Vec::new();
    for (index, provider) in providers.iter().enumerate() {
        let (id, binding) = migration::import_provider(&mut store, client, provider)
            .ok_or("This link does not contain a usable API provider")?;
        imported.push(id);
        if let Some(binding) = binding {
            if select && index == 0 {
                store.bindings.insert(client.into(), binding);
            } else {
                store.bindings.entry(client.into()).or_insert(binding);
            }
        }
    }
    validate(&store)?;
    store.revision += 1;
    write_store(&store)?;
    Ok(imported)
}

#[tauri::command]
pub fn get_deck(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<DeckView, String> {
    let _lock = STORE_LOCK
        .lock()
        .map_err(|_| "Resource store lock failed")?;
    if !config_path().exists() {
        let store = migration::import(state.db.as_ref())?;
        validate(&store)?;
        write_store(&store)?;
    }
    let result = view(read_store()?);
    crate::tray::refresh_tray_menu(&app);
    Ok(result)
}

#[tauri::command]
pub fn save_deck(mut store: DeckStore, app: tauri::AppHandle) -> Result<DeckView, String> {
    let _lock = STORE_LOCK
        .lock()
        .map_err(|_| "Resource store lock failed")?;
    let previous = read_store()?;
    if store.revision != previous.revision {
        return Err("Resources changed in another window; reload before saving".into());
    }
    // Activation and endpoint credentials cannot be changed through a graph edit.
    store.active_bindings = previous.active_bindings;
    store.migration = previous.migration;
    for (id, endpoint) in &mut store.endpoints {
        endpoint.token = previous
            .endpoints
            .get(id)
            .map(|e| e.token.clone())
            .unwrap_or_else(|| format!("deck-{}", uuid::Uuid::new_v4().simple()));
    }
    validate(&store)?;
    store.revision += 1;
    write_store(&store)?;
    crate::tray::refresh_tray_menu(&app);
    Ok(view(store))
}

#[tauri::command]
pub async fn apply_deck_binding(
    client: String,
    revision: u64,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    let store = read_store()?;
    if revision != store.revision {
        return Err("Resources changed; reload before applying".into());
    }
    let mut binding = store
        .bindings
        .get(&client)
        .ok_or("Save a harness selection first")?
        .clone();
    binding.input_protocol = Some(binding_protocol(&store, &client, &binding)?);
    validate_binding(&store, &client, &binding, true)?;
    // Check upstream credential availability without exposing it to the harness.
    let endpoint = &store.endpoints[&binding.endpoint_id];
    for alias in [&binding.model, &binding.utility_model]
        .into_iter()
        .filter(|a| !a.is_empty())
    {
        let model = &store.models[&endpoint.routes[alias].model_id];
        resolve_key(&store.providers[&model.provider_id])?;
    }
    let config = state
        .db
        .get_global_proxy_config()
        .await
        .map_err(|e| e.to_string())?;
    if config.listen_port == 0 {
        return Err(
            "Choose a fixed gateway port in Settings before applying harness configurations".into(),
        );
    }
    let info = state.proxy_service.start().await?;
    publish_origin(&info.address, info.port)?;
    let _lock = STORE_LOCK
        .lock()
        .map_err(|_| "Resource store lock failed")?;
    let mut current = read_store()?;
    if current.revision != revision {
        return Err("Resources changed while starting the gateway; apply again".into());
    }
    let profile = project_profile(&current, &client, &binding, &origin())?;
    let previous = current
        .active_bindings
        .get(&client)
        .and_then(|b| project_profile(&current, &client, b, &origin()).ok());
    current.active_bindings.insert(client.clone(), binding);
    current.revision += 1;
    native::apply(&client, &profile, previous.as_ref(), &state, || {
        write_store(&current)
    })?;
    crate::tray::refresh_tray_menu(&app);
    Ok(
        serde_json::json!({"view":view(current),"restartRequired":matches!(client.as_str(),"cursor"|"claude-desktop"),"command":native::command(&client)}),
    )
}

#[tauri::command]
pub fn reset_deck_binding(
    client: String,
    revision: u64,
    app: tauri::AppHandle,
) -> Result<DeckView, String> {
    let _lock = STORE_LOCK
        .lock()
        .map_err(|_| "Resource store lock failed")?;
    let mut store = read_store()?;
    if revision != store.revision {
        return Err("Resources changed; reload before restoring".into());
    }
    if !store.active_bindings.contains_key(&client) {
        return Ok(view(store));
    }
    store.active_bindings.remove(&client);
    store.revision += 1;
    native::restore(&client, || write_store(&store))?;
    crate::tray::refresh_tray_menu(&app);
    Ok(view(store))
}

#[cfg(test)]
mod tests;
