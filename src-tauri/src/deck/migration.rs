use super::*;
use crate::{
    database::Database,
    model_clients::{ClientModel, ClientProfile},
    provider::Provider,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

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

fn openrouter_suffix(proto: Protocol) -> String {
    let protocol = match proto {
        Protocol::OpenaiChat => "Chat",
        Protocol::OpenaiResponses => "Responses",
        Protocol::Anthropic => "Messages",
        Protocol::Gemini => "Gemini",
    };
    format!(" · OpenRouter（{protocol}）")
}

fn imported_connection_name(client: &str, profile: &ClientProfile, proto: Protocol) -> String {
    let name = profile.connection_name.trim();
    let openrouter =
        validate_url(&profile.endpoint).is_ok_and(|url| url.host_str() == Some("openrouter.ai"));
    if openrouter
        && matches!(
            name,
            "" | "default"
                | "Imported provider"
                | "OpenRouter"
                | "OpenRouter (Codex CLI)"
                | "OpenRouter (Claude Desktop)"
        )
    {
        let client = match client {
            "codex" => "Codex CLI",
            "claude" => "Claude Code",
            "claude-desktop" => "Claude Desktop",
            "antigravity" => "Antigravity CLI",
            "cursor-agent" => "Cursor CLI",
            "cursor" => "Cursor GUI",
            "muse" => "Muse Code",
            "opencode" => "OpenCode",
            "grokbuild" => "Grok",
            "hermes" => "Hermes",
            other => other,
        };
        return format!("{client}{}", openrouter_suffix(proto));
    }
    if name.is_empty() {
        "Imported provider".into()
    } else {
        name.into()
    }
}

fn merge_imported_name(existing: &mut String, incoming: &str, proto: Protocol) {
    let suffix = openrouter_suffix(proto);
    if let (Some(previous), Some(next)) = (
        existing.strip_suffix(&suffix),
        incoming.strip_suffix(&suffix),
    ) {
        let clients = previous
            .split(" / ")
            .chain(next.split(" / "))
            .collect::<BTreeSet<_>>();
        *existing = format!(
            "{}{}",
            clients.into_iter().collect::<Vec<_>>().join(" / "),
            suffix
        );
    }
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
    let source_dir = if client == "opencode" {
        crate::opencode_config::get_opencode_config_path()
            .parent()?
            .to_path_buf()
    } else {
        get_app_config_dir()
    };
    let (api_key, key_env) = credentials::imported(&profile.api_key, &profile.key_env, &source_dir);
    let connection_name = imported_connection_name(client, profile, proto);
    let provider_id = id(
        "provider",
        &json!([
            profile.endpoint.trim_end_matches('/'),
            proto,
            api_key,
            key_env,
            profile.provider_meta
        ]),
    );
    let provider = store
        .providers
        .entry(provider_id.clone())
        .or_insert_with(|| UpstreamProvider {
            id: provider_id.clone(),
            name: connection_name.clone(),
            base_url: profile.endpoint.trim_end_matches('/').into(),
            protocol: proto,
            provider_meta: profile.provider_meta.clone(),
            api_key: api_key.clone(),
            key_env: key_env.clone(),
            key_source: Some(if key_env.is_empty() { "stored" } else { "env" }.into()),
        });
    merge_imported_name(&mut provider.name, &connection_name, proto);
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
            name: connection_name.clone(),
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
    merge_imported_name(&mut endpoint.name, &connection_name, proto);
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
        input_protocol: client_protocols(client).contains(&proto).then_some(proto),
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
        default_effort: String::new(),
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
    let mut key_env = String::new();
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
                if let Some(token) = table
                    .get("experimental_bearer_token")
                    .and_then(toml::Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                {
                    key = token.into();
                } else if let Some(name) = table
                    .get("env_key")
                    .and_then(toml::Value::as_str)
                    .filter(|value| !value.is_empty())
                {
                    // The selected provider's credentials are independent of
                    // auth.json, which may retain an unrelated official login.
                    key.clear();
                    key_env = name.into();
                }
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
                .or_else(|| entry.get("model"))
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
        provider_meta: crate::model_clients::provider_adapter::preserved_meta(provider),
        api_key: key,
        key_env,
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
    let opencode_selection = crate::opencode_config::read_opencode_config()
        .ok()
        .and_then(|config| {
            config
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
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
                    let selected_in_opencode = client == "opencode"
                        && opencode_selection.as_deref().is_some_and(|selection| {
                            selection
                                .strip_prefix(&format!("{}/", provider.id))
                                .is_some_and(|model| {
                                    if store.endpoints[&binding.endpoint_id]
                                        .routes
                                        .contains_key(model)
                                    {
                                        binding.model = model.into();
                                        true
                                    } else {
                                        false
                                    }
                                })
                        });
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
                    if (current.as_ref() == Some(&provider.id) || selected_in_opencode)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_preserves_free_tier_names_and_saved_selections() {
        let profile: ClientProfile = serde_json::from_value(json!({
            "connectionName":"OpenRouter", "endpoint":"https://example.test/v1", "protocol":"openai-chat",
            "keyEnv":"OR_TEST_KEY", "models":[{"id":"thinkingmachines/inkling:free","name":"Inkling","context":200000,"output":8192}],
            "defaultModel":"thinkingmachines/inkling:free", "utilityModel":"thinkingmachines/inkling:free"
        })).unwrap();
        let store = import_profiles(&[
            ("antigravity", profile.clone()),
            ("cursor-agent", profile.clone()),
            ("muse", profile),
        ]);
        validate(&store)
            .expect("one free-tier model must not prevent all legacy resources loading");
        assert_eq!(store.models.len(), 1);
        assert_eq!(
            store.providers.values().next().unwrap().key_env,
            "OR_TEST_KEY"
        );
        for client in ["antigravity", "cursor-agent", "muse"] {
            assert_eq!(
                store.bindings[client].model,
                "thinkingmachines/inkling:free"
            );
            assert_eq!(
                store.bindings[client].utility_model,
                "thinkingmachines/inkling:free"
            );
        }
        assert!(store.active_bindings.is_empty());
    }

    #[test]
    fn codex_migration_retains_selected_provider_environment_credentials() {
        let config = "model = \"model\"\nmodel_provider = \"router\"\n[model_providers.router]\nbase_url = \"https://example.test/v1\"\nwire_api = \"responses\"\nenv_key = \"CX_TEST_KEY\"\n";
        let provider = Provider::with_id(
            "legacy".into(),
            "Codex".into(),
            json!({"auth":{},"config":config}),
            None,
        );
        let profile = legacy_profile("codex", &provider).unwrap();
        assert_eq!(profile.key_env, "CX_TEST_KEY");
        let store = import_profiles(&[("codex", profile)]);
        validate(&store).unwrap();
        assert_eq!(
            store.providers.values().next().unwrap().key_env,
            "CX_TEST_KEY"
        );
        assert!(store.providers.values().next().unwrap().api_key.is_empty());
    }

    #[test]
    fn codex_migration_uses_provider_token_instead_of_preserved_login_auth() {
        let config = "model = \"model\"\nmodel_provider = \"router\"\n[model_providers.router]\nbase_url = \"https://example.test/v1\"\nwire_api = \"responses\"\nexperimental_bearer_token = \"provider-only-test-key\"\n";
        let provider = Provider::with_id(
            "legacy".into(),
            "Codex".into(),
            json!({"auth":{"OPENAI_API_KEY":"unrelated-login-test-key"},"config":config}),
            None,
        );
        let profile = legacy_profile("codex", &provider).unwrap();
        assert_eq!(profile.api_key, "provider-only-test-key");
        assert!(profile.key_env.is_empty());
    }

    #[test]
    #[ignore = "Requires a private snapshot of existing legacy settings; no network or native writes"]
    fn private_legacy_snapshot_migrates_without_losing_model_routes() {
        let home = PathBuf::from(std::env::var("MODELDECK_LEGACY_MIGRATION_HOME").unwrap());
        assert_eq!(
            crate::config::get_home_dir(),
            home,
            "use only the private snapshot"
        );
        let db = Database::init().unwrap();
        let original = crate::model_clients::read_store().unwrap();
        let store = import(&db).unwrap();
        validate(&store).expect("the complete legacy snapshot must migrate successfully");
        for (client, profile) in &original.profiles {
            let binding = &store.bindings[client.id()];
            assert_eq!(binding.model, profile.default_model);
            let endpoint = &store.endpoints[&binding.endpoint_id];
            for model in &profile.models {
                assert!(
                    endpoint.routes.contains_key(&model.id),
                    "a saved model route was lost"
                );
                assert_eq!(
                    store.models[&endpoint.routes[&model.id].model_id].upstream_id,
                    model.id
                );
            }
            let resource = &store.models[&endpoint.routes[&binding.model].model_id];
            assert_eq!(
                store.providers[&resource.provider_id].key_env,
                profile.key_env
            );
        }
        assert!(store.active_bindings.is_empty());
        if let Some(selected) = crate::opencode_config::read_opencode_config()
            .ok()
            .and_then(|config| {
                config
                    .get("model")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
        {
            if let Some((_, model)) = selected.split_once('/') {
                let binding = store
                    .bindings
                    .get("opencode")
                    .expect("preserve the native OpenCode selection");
                assert_eq!(binding.model, model);
                assert_eq!(binding.input_protocol, Some(Protocol::OpenaiChat));
            }
        }
        eprintln!("Legacy snapshot restored: {} providers, {} model resources, {} endpoints, {} saved harness selections",store.providers.len(),store.models.len(),store.endpoints.len(),store.bindings.len());
    }

    fn provider(reference: &str) -> Provider {
        Provider::with_id(
            "legacy".into(),
            "Legacy OpenCode".into(),
            json!({
                "options":{"baseURL":"https://example.test/v1","apiKey":reference},
                "models":{"router":{"name":"Router","limit":{"context":8192,"output":1024}}}
            }),
            None,
        )
    }

    #[test]
    fn legacy_environment_references_share_the_named_environment_resource() {
        let original = provider("{env:MODELDECK_TEST_KEY}");
        let profile = legacy_profile("opencode", &original).unwrap();
        let mut named = profile.clone();
        named.api_key.clear();
        named.key_env = "MODELDECK_TEST_KEY".into();
        let store = import_profiles(&[("opencode", profile), ("muse", named)]);
        assert_eq!(store.providers.len(), 1);
        let imported = store.providers.values().next().unwrap();
        assert!(imported.api_key.is_empty());
        assert_eq!(imported.key_env, "MODELDECK_TEST_KEY");
        assert_eq!(imported.key_source.as_deref(), Some("env"));
        assert_eq!(
            original.settings_config["options"]["apiKey"],
            "{env:MODELDECK_TEST_KEY}"
        );
        validate(&store).unwrap();
    }

    #[test]
    #[serial_test::serial]
    fn legacy_file_references_keep_the_original_opencode_directory() {
        let original = provider("{file:keys/token}");
        let profile = legacy_profile("opencode", &original).unwrap();
        let store = import_profiles(&[("opencode", profile)]);
        let imported = store.providers.values().next().unwrap();
        let expected = crate::opencode_config::get_opencode_config_path()
            .parent()
            .unwrap()
            .join("keys/token");
        assert_eq!(imported.api_key, format!("{{file:{}}}", expected.display()));
        assert!(imported.key_env.is_empty());
        assert_eq!(
            original.settings_config["options"]["apiKey"],
            "{file:keys/token}"
        );
        assert!(store.active_bindings.is_empty());
        validate(&store).unwrap();
    }

    #[test]
    #[ignore = "Explicitly authorized read-only audit of existing OpenCode credentials; no network or native writes"]
    fn existing_opencode_reference_credentials_resolve_without_copying_keys() {
        use rusqlite::{Connection, OpenFlags};
        let home = PathBuf::from(std::env::var("MODELDECK_CREDENTIAL_AUDIT_HOME").unwrap());
        let db_path = home.join(".cc-switch/cc-switch.db");
        let config_path = home.join(".config/opencode/opencode.json");
        let token_path = home.join(".arena/token");
        let protected = [
            db_path.clone(),
            config_path,
            token_path.clone(),
            home.join(".codex/auth.json"),
        ];
        let before: Vec<_> = protected
            .iter()
            .map(|path| std::fs::read(path).unwrap())
            .collect();
        let db = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let mut statement = db
            .prepare("SELECT id,name,settings_config FROM providers WHERE app_type='opencode'")
            .unwrap();
        let values = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .unwrap();
        let mut verified = 0;
        for value in values {
            let (id, name, config) = value.unwrap();
            if !matches!(id.as_str(), "arena" | "OpenRouter" | "LM Studio") {
                continue;
            }
            let original = Provider::with_id(
                id.clone(),
                name,
                serde_json::from_str(&config).unwrap(),
                None,
            );
            let profile = legacy_profile("opencode", &original).unwrap();
            let store = import_profiles(&[("opencode", profile)]);
            let imported = store.providers.values().next().unwrap();
            let resolved = resolve_key(imported).unwrap();
            let expected = match id.as_str() {
                "arena" => std::fs::read_to_string(&token_path)
                    .unwrap()
                    .trim()
                    .to_string(),
                "OpenRouter" => std::env::var("OC_OR_KEY").unwrap(),
                _ => String::new(),
            };
            assert!(
                resolved == expected,
                "The migrated credential did not match its original source"
            );
            if !expected.is_empty() {
                assert!(
                    !serde_json::to_string(&store).unwrap().contains(&expected),
                    "Migration copied a credential value into the resource graph"
                );
            }
            verified += 1;
        }
        assert_eq!(verified, 3);
        for (path, original) in protected.iter().zip(before) {
            assert!(
                std::fs::read(path).unwrap() == original,
                "The read-only credential audit changed a protected file"
            );
        }
    }
}
