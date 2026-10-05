use super::*;
use crate::{
    database::Database,
    model_clients::{ClientModel, ClientProfile},
    provider::Provider,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn id(prefix: &str, value: &Value) -> String {
    let hash = Sha256::digest(value.to_string().as_bytes());
    let hex = hash.iter().map(|b| format!("{b:02x}")).collect::<String>();
    format!("{prefix}-{}", &hex[..20])
}

fn protocol(value: &str) -> Option<Protocol> {
    Some(match value {
        "openai-chat" | "openai_chat" | "chat" | "chat_completions" => Protocol::OpenaiChat,
        "openai-responses" | "openai_responses" | "responses" | "codex_responses" => {
            Protocol::OpenaiResponses
        }
        "anthropic" | "anthropic_messages" => Protocol::Anthropic,
        "gemini" | "gemini_native" => Protocol::Gemini,
        _ => return None,
    })
}

fn add_profile(
    store: &mut DeckStore,
    client: &str,
    profile: &ClientProfile,
) -> Option<HarnessBinding> {
    let proto = protocol(&profile.protocol)?;
    if validate_url(&profile.endpoint).is_err() {
        return None;
    }
    let provider_id = id(
        "provider",
        &json!([
            profile.endpoint.trim_end_matches('/'),
            proto,
            profile.api_key,
            profile.key_env
        ]),
    );
    store
        .providers
        .entry(provider_id.clone())
        .or_insert_with(|| UpstreamProvider {
            id: provider_id.clone(),
            name: if profile.connection_name.is_empty() {
                "Imported provider".into()
            } else {
                profile.connection_name.clone()
            },
            base_url: profile.endpoint.trim_end_matches('/').into(),
            protocol: proto,
            api_key: profile.api_key.clone(),
            key_env: profile.key_env.clone(),
            key_source: Some(
                if profile.key_env.is_empty() {
                    "stored"
                } else {
                    "env"
                }
                .into(),
            ),
        });
    let mut routes = BTreeMap::new();
    for model in &profile.models {
        if model.id.is_empty()
            || model.context == 0
            || model.output == 0
            || model.output > model.context
        {
            continue;
        }
        let model_id = id(
            "model",
            &json!([provider_id, model.id, model.context, model.output]),
        );
        let resource = store
            .models
            .entry(model_id.clone())
            .or_insert_with(|| ModelResource {
                id: model_id.clone(),
                provider_id: provider_id.clone(),
                upstream_id: model.id.clone(),
                name: model.name.clone(),
                context: model.context,
                output: model.output,
                reasoning: Vec::new(),
                reasoning_supported: model.reasoning_supported,
                tools: true,
                images: false,
            });
        if model.reasoning_supported != Some(false) {
            for level in &model.reasoning {
                if LEVELS.contains(&level.as_str()) && !resource.reasoning.contains(level) {
                    resource.reasoning.push(level.clone());
                }
            }
        }
        if !resource.reasoning.is_empty() {
            resource.reasoning_supported = Some(true);
        }
        routes.insert(
            model.id.clone(),
            ModelRoute {
                model_id,
                effort: if resource.reasoning.contains(&profile.effort) {
                    profile.effort.clone()
                } else {
                    String::new()
                },
            },
        );
    }
    let endpoint_id = id("endpoint", &serde_json::to_value(&routes).ok()?);
    let endpoint_id = format!("{endpoint_id}-{}", &provider_id[provider_id.len() - 6..]);
    store
        .endpoints
        .entry(endpoint_id.clone())
        .or_insert_with(|| ExposedEndpoint {
            id: endpoint_id.clone(),
            name: profile
                .connection_name
                .clone()
                .trim()
                .to_string()
                .chars()
                .take(80)
                .collect::<String>(),
            protocols: vec![
                Protocol::OpenaiChat,
                Protocol::OpenaiResponses,
                Protocol::Anthropic,
                Protocol::Gemini,
            ],
            public_url: String::new(),
            token: format!("deck-{}", uuid::Uuid::new_v4().simple()),
            routes,
        });
    let endpoint = store.endpoints.get_mut(&endpoint_id)?;
    if endpoint.name.is_empty() {
        endpoint.name = "Imported endpoint".into();
    }
    let models = compatible_models(store, client, &store.endpoints[&endpoint_id]);
    let model = if models.contains(&profile.default_model) || client == "claude-desktop" {
        profile.default_model.clone()
    } else {
        models.first()?.clone()
    };
    Some(HarnessBinding {
        input_protocol: None,
        endpoint_id,
        model,
        utility_model: if models.contains(&profile.utility_model) {
            profile.utility_model.clone()
        } else {
            String::new()
        },
        executable: profile.executable.clone(),
    })
}

fn str_at<'a>(value: &'a Value, paths: &[&str]) -> &'a str {
    paths
        .iter()
        .find_map(|path| {
            value
                .pointer(path)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
        .unwrap_or("")
}

fn limits(value: &Value) -> (u64, u64) {
    let context = value
        .pointer("/limit/context")
        .or_else(|| value.get("context_length"))
        .or_else(|| value.get("contextWindow"))
        .and_then(Value::as_u64)
        .unwrap_or(200_000)
        .max(1);
    let output = value
        .pointer("/limit/output")
        .or_else(|| value.get("max_output_tokens"))
        .or_else(|| value.get("maxOutputTokens"))
        .and_then(Value::as_u64)
        .unwrap_or(8192)
        .max(1)
        .min(context);
    (context, output)
}

fn model(id: &str, value: &Value) -> ClientModel {
    let (context, output) = limits(value);
    let mut reasoning = value
        .get("variants")
        .and_then(Value::as_object)
        .map(|m| {
            m.keys()
                .filter(|k| LEVELS.contains(&k.as_str()))
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(level) = value
        .pointer("/options/reasoningEffort")
        .or_else(|| value.pointer("/options/reasoning/effort"))
        .and_then(Value::as_str)
    {
        if LEVELS.contains(&level) && !reasoning.iter().any(|s| s == level) {
            reasoning.push(level.into());
        }
    }
    ClientModel {
        id: id.into(),
        name: value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(id)
            .into(),
        context,
        output,
        reasoning,
        reasoning_supported: value.get("reasoning").and_then(Value::as_bool),
    }
}

fn legacy_profile(client: &str, provider: &Provider) -> Option<ClientProfile> {
    if provider.category.as_deref() == Some("official") || provider.uses_managed_account_auth() {
        return None;
    }
    let config = &provider.settings_config;
    let mut base = str_at(
        config,
        &[
            "/options/baseURL",
            "/base_url",
            "/baseUrl",
            "/env/ANTHROPIC_BASE_URL",
        ],
    )
    .to_string();
    let mut key = str_at(
        config,
        &[
            "/options/apiKey",
            "/api_key",
            "/apiKey",
            "/env/ANTHROPIC_AUTH_TOKEN",
            "/env/ANTHROPIC_API_KEY",
            "/auth/OPENAI_API_KEY",
        ],
    )
    .to_string();
    let mut proto = provider
        .meta
        .as_ref()
        .and_then(|m| m.api_format.as_deref())
        .and_then(protocol)
        .or_else(|| {
            config
                .get("api_mode")
                .and_then(Value::as_str)
                .and_then(protocol)
        })
        .unwrap_or(if matches!(client, "claude" | "claude-desktop") {
            Protocol::Anthropic
        } else if matches!(client, "codex" | "grokbuild") {
            Protocol::OpenaiResponses
        } else {
            Protocol::OpenaiChat
        });
    let mut default = str_at(
        config,
        &[
            "/model",
            "/env/ANTHROPIC_MODEL",
            "/env/ANTHROPIC_DEFAULT_SONNET_MODEL",
        ],
    )
    .to_string();
    let mut effort = String::new();
    if let Some(text) = config.get("config").and_then(Value::as_str) {
        if client == "grokbuild" {
            if let Some(parsed) = crate::grok_config::extract_model_config(text) {
                base = parsed.base_url;
                key = parsed.api_key.unwrap_or_default();
                default = parsed.model;
                proto = protocol(&parsed.api_backend).unwrap_or(Protocol::OpenaiResponses);
            }
        } else if let Ok(doc) = text.parse::<toml::Value>() {
            default = doc
                .get("model")
                .and_then(toml::Value::as_str)
                .unwrap_or(&default)
                .into();
            effort = doc
                .get("model_reasoning_effort")
                .and_then(toml::Value::as_str)
                .unwrap_or("")
                .into();
            let selected = doc
                .get("model_provider")
                .and_then(toml::Value::as_str)
                .unwrap_or("custom");
            if let Some(table) = doc.get("model_providers").and_then(|v| v.get(selected)) {
                base = table
                    .get("base_url")
                    .and_then(toml::Value::as_str)
                    .unwrap_or(&base)
                    .into();
                proto = table
                    .get("wire_api")
                    .and_then(toml::Value::as_str)
                    .and_then(protocol)
                    .unwrap_or(proto);
            }
        }
    }
    if base.is_empty() {
        return None;
    }
    let mut models = BTreeMap::new();
    if let Some(catalog) = config.get("models").and_then(Value::as_object) {
        for (id, value) in catalog {
            models.insert(id.clone(), model(id, value));
        }
    }
    for catalog in [config.get("models"), config.pointer("/modelCatalog/models")]
        .into_iter()
        .flatten()
        .filter_map(Value::as_array)
    {
        for entry in catalog {
            if let Some(id) = entry
                .get("id")
                .or_else(|| entry.get("slug"))
                .and_then(Value::as_str)
            {
                models.insert(id.into(), model(id, entry));
            }
        }
    }
    if let Some(env) = config.get("env").and_then(Value::as_object) {
        for (name, value) in env {
            if name == "ANTHROPIC_MODEL"
                || name.starts_with("ANTHROPIC_DEFAULT_") && name.ends_with("_MODEL")
            {
                if let Some(id) = value.as_str().filter(|s| !s.is_empty()) {
                    models
                        .entry(id.into())
                        .or_insert_with(|| model(id, &json!({})));
                }
            }
        }
    }
    if let Some(meta) = provider.meta.as_ref() {
        for route in meta.claude_desktop_model_routes.values() {
            if !route.model.is_empty() {
                models
                    .entry(route.model.clone())
                    .or_insert_with(|| model(&route.model, &json!({})));
            }
        }
    }
    if !default.is_empty() {
        models
            .entry(default.clone())
            .or_insert_with(|| model(&default, &json!({})));
    }
    if default.is_empty() {
        default = models.keys().next().cloned().unwrap_or_default();
    }
    if !effort.is_empty() && LEVELS.contains(&effort.as_str()) {
        if let Some(model) = models.get_mut(&default) {
            if !model.reasoning.contains(&effort) {
                model.reasoning.push(effort.clone());
            }
        }
    }
    Some(ClientProfile {
        connection_name: provider.name.clone(),
        endpoint: base,
        protocol: proto.as_str().into(),
        api_key: key,
        key_env: String::new(),
        models: models.into_values().collect(),
        default_model: default,
        utility_model: String::new(),
        effort,
        executable: String::new(),
        enabled: false,
        local_token: String::new(),
    })
}

pub fn import(db: &Database) -> Result<DeckStore, String> {
    let mut store = DeckStore::default();
    let old = crate::model_clients::read_store()?;
    for (client, profiles) in &old.saved_profiles {
        for profile in profiles.values() {
            add_profile(&mut store, client.id(), profile);
        }
    }
    for (client, profile) in old.profiles.iter().chain(&old.active) {
        if let Some(binding) = add_profile(&mut store, client.id(), profile) {
            store.bindings.entry(client.id().into()).or_insert(binding);
        }
    }
    for client in CLIENTS {
        if crate::model_clients::ModelClient::try_from_id(client).is_some() {
            continue;
        }
        let current = db.get_current_provider(client).map_err(|e| e.to_string())?;
        for provider in db
            .get_all_providers(client)
            .map_err(|e| e.to_string())?
            .values()
        {
            if let Some(profile) = legacy_profile(client, provider) {
                if let Some(mut binding) = add_profile(&mut store, client, &profile) {
                    if client == "claude-desktop" {
                        // Preserve Desktop's public role names while moving its
                        // upstream mappings into the shared endpoint graph.
                        if let Some(meta) = provider.meta.as_ref() {
                            let endpoint = store.endpoints.get_mut(&binding.endpoint_id).unwrap();
                            let old_routes = endpoint.routes.clone();
                            for (alias, route) in &meta.claude_desktop_model_routes {
                                if let Some(target) = old_routes.get(&route.model) {
                                    endpoint.routes.insert(alias.clone(), target.clone());
                                }
                            }
                            binding.model = endpoint
                                .routes
                                .keys()
                                .find(|alias| {
                                    crate::claude_desktop_config::is_claude_safe_model_id(alias)
                                })
                                .cloned()
                                .unwrap_or(binding.model);
                        }
                    }
                    if current.as_ref() == Some(&provider.id)
                        && validate_binding(&store, client, &binding, false).is_ok()
                    {
                        store.bindings.insert(client.into(), binding);
                    }
                }
            } else {
                store.migration.retained.push(format!(
                    "{} / {}: original login or specialized configuration retained",
                    client, provider.name
                ));
            }
        }
    }
    for universal in db
        .get_all_universal_providers()
        .map_err(|e| e.to_string())?
        .values()
    {
        for (client, provider) in [
            ("claude", universal.to_claude_provider()),
            ("codex", universal.to_codex_provider()),
        ] {
            if let Some(profile) = provider.as_ref().and_then(|p| legacy_profile(client, p)) {
                add_profile(&mut store, client, &profile);
            }
        }
    }
    store.migration.imported_providers = store.providers.len();
    store.migration.imported_models = store.models.len();
    // Migration does not activate a gateway or write any native configuration.
    Ok(store)
}

#[cfg(test)]
pub(super) fn import_profiles(profiles: &[(&str, ClientProfile)]) -> DeckStore {
    let mut store = DeckStore::default();
    for (client, profile) in profiles {
        if let Some(binding) = add_profile(&mut store, client, profile) {
            store.bindings.insert((*client).into(), binding);
        }
    }
    store
}
