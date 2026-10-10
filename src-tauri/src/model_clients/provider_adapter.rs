//! Bridge the resource graph to CC Switch's existing provider adapters.
use super::ClientProfile;
use crate::provider::{CodexChatReasoningConfig, Provider, ProviderMeta};
use crate::proxy::{
    error::ProxyError,
    providers::{self, ClaudeAdapter, CodexAdapter, GeminiAdapter, ProviderAdapter},
};
use serde_json::{json, Value};
use std::sync::LazyLock;

static PRESETS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../resources/provider-adapters.json"))
        .expect("bundled original provider metadata")
});

pub(crate) fn preserved_meta(provider: &Provider) -> Option<Value> {
    let original = provider
        .meta
        .as_ref()
        .and_then(|m| serde_json::to_value(m).ok())
        .unwrap_or(json!({}));
    let mut result = serde_json::Map::new();
    for field in [
        "apiKeyField",
        "isFullUrl",
        "codexChatReasoning",
        "promptCacheKey",
        "promptCacheRouting",
    ] {
        if let Some(value) = original.get(field) {
            result.insert(field.into(), value.clone());
        }
    }
    if !result.contains_key("apiKeyField") {
        let env = &provider.settings_config["env"];
        if env.get("ANTHROPIC_AUTH_TOKEN").is_some() {
            result.insert("apiKeyField".into(), json!("ANTHROPIC_AUTH_TOKEN"));
        } else if env.get("ANTHROPIC_API_KEY").is_some() {
            result.insert("apiKeyField".into(), json!("ANTHROPIC_API_KEY"));
        }
    }
    (!result.is_empty()).then_some(Value::Object(result))
}

pub(crate) fn project(p: &ClientProfile, key: &str) -> Provider {
    let mut meta: ProviderMeta = p
        .provider_meta
        .clone()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    if meta.codex_chat_reasoning.is_none() {
        if let Some(preset) = PRESETS.as_array().unwrap().iter().find(|entry| {
            entry["protocol"].as_str() == Some(p.protocol.as_str())
                && entry["baseUrl"].as_str().is_some_and(|url| {
                    url.trim_end_matches('/') == p.endpoint.trim_end_matches('/')
                })
        }) {
            if let Ok(defaults) = serde_json::from_value::<ProviderMeta>(preset["meta"].clone()) {
                meta.codex_chat_reasoning = defaults.codex_chat_reasoning;
                if meta.prompt_cache_routing.is_none() {
                    meta.prompt_cache_routing = defaults.prompt_cache_routing;
                }
            }
        }
    }
    meta.api_format = Some(
        match p.protocol.as_str() {
            "openai-chat" => "openai_chat",
            "openai-responses" => "openai_responses",
            "gemini" => "gemini_native",
            _ => "anthropic",
        }
        .into(),
    );
    let key_field = meta.api_key_field.as_deref().unwrap_or("ANTHROPIC_API_KEY");
    let mut env = json!({"ANTHROPIC_BASE_URL":p.endpoint,"GEMINI_API_KEY":key});
    env[key_field] = json!(key);
    let mut provider = Provider::with_id(
        "modeldeck-upstream".into(),
        p.connection_name.clone(),
        json!({
            "base_url":p.endpoint,"baseUrl":p.endpoint,"api_key":key,"apiKey":key,
            "auth":{"OPENAI_API_KEY":key},"env":env,
            "modelCatalog":{"models":p.models.iter().map(|m|json!({"model":m.id,"reasoningLevels":m.reasoning})).collect::<Vec<_>>()}
        }),
        None,
    );
    provider.meta = Some(meta);
    provider
}

pub(crate) fn reasoning_config(
    provider: &Provider,
    body: &Value,
) -> Option<CodexChatReasoningConfig> {
    let mut config = providers::resolve_codex_chat_reasoning_config(provider, body)?;
    // The inherited OpenRouter enum predates current model catalogs supporting
    // max. Keep its field selection, while preserving today's native values.
    if config.effort_value_mode.as_deref() == Some("openrouter") {
        config.effort_value_mode = Some("passthrough".into());
    }
    Some(config)
}

pub(crate) fn adapt_chat(body: &mut Value, provider: &Provider) {
    let Some(config) = reasoning_config(provider, body) else {
        return;
    };
    let mut canonical = json!({"model":body["model"]});
    if let Some(reasoning) = body.get("reasoning") {
        canonical["reasoning"] = reasoning.clone();
        if let Some(reasoning) = canonical["reasoning"].as_object_mut() {
            if let Some(effort) = body.get("reasoning_effort") {
                reasoning.entry("effort").or_insert(effort.clone());
            }
        }
    } else if let Some(effort) = body.get("reasoning_effort") {
        canonical["reasoning"] = json!({"effort":effort});
    } else {
        return;
    }
    let mut adapted = json!({});
    providers::transform_codex_chat::apply_reasoning_options(
        &mut adapted,
        &canonical,
        body.get("model").and_then(Value::as_str).unwrap_or(""),
        Some(&config),
    );
    if config.effort_param.as_deref() != Some("reasoning_effort") {
        body.as_object_mut().unwrap().remove("reasoning_effort");
    }
    if config.effort_param.as_deref() != Some("reasoning.effort") {
        if let Some(reasoning) = body.get_mut("reasoning").and_then(Value::as_object_mut) {
            reasoning.remove("effort");
        }
        if body
            .get("reasoning")
            .and_then(Value::as_object)
            .is_some_and(|r| r.is_empty())
        {
            body.as_object_mut().unwrap().remove("reasoning");
        }
    }
    for (field, value) in adapted.as_object().unwrap() {
        if field == "reasoning" {
            if body.get(field).is_none() {
                body[field] = json!({});
            }
            if let (Some(existing), Some(values)) = (body[field].as_object_mut(), value.as_object())
            {
                for (name, value) in values {
                    existing.entry(name).or_insert(value.clone());
                }
            }
        } else {
            body.as_object_mut()
                .unwrap()
                .entry(field)
                .or_insert(value.clone());
        }
    }
}

fn adapter(protocol: &str) -> Box<dyn ProviderAdapter> {
    match protocol {
        "anthropic" => Box::new(ClaudeAdapter::new()),
        "gemini" => Box::new(GeminiAdapter::new()),
        _ => Box::new(CodexAdapter::new()),
    }
}

pub(crate) fn url(p: &ClientProfile, endpoint: &str) -> String {
    let provider = project(p, "");
    if provider
        .meta
        .as_ref()
        .is_some_and(|m| m.is_full_url == Some(true))
    {
        return p.endpoint.clone();
    }
    adapter(&p.protocol).build_url(&p.endpoint, endpoint)
}

pub(crate) fn authenticate(
    request: reqwest::RequestBuilder,
    p: &ClientProfile,
    key: &str,
) -> Result<reqwest::RequestBuilder, ProxyError> {
    let mut request = request;
    if p.protocol == "anthropic" {
        request = request.header("anthropic-version", "2023-06-01");
    }
    if key.is_empty() {
        return Ok(request);
    }
    let provider = project(p, key);
    let adapter = adapter(&p.protocol);
    if let Some(auth) = adapter.extract_auth(&provider) {
        for (name, value) in adapter.get_auth_headers(&auth)? {
            request = request.header(name, value);
        }
    }
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile(protocol: &str, endpoint: &str) -> ClientProfile {
        serde_json::from_value(json!({"connectionName":"Upstream","endpoint":endpoint,"protocol":protocol,"models":[],"defaultModel":"model"})).unwrap()
    }
    #[test]
    fn inherited_auth_distinguishes_openrouter_bearer_and_anthropic_api_key() {
        let client = reqwest::Client::new();
        for (endpoint, field, expected, absent) in [
            (
                "https://openrouter.ai/api/v1",
                None,
                "authorization",
                "x-api-key",
            ),
            (
                "https://api.anthropic.com",
                None,
                "x-api-key",
                "authorization",
            ),
            (
                "https://relay.example/v1",
                Some("ANTHROPIC_AUTH_TOKEN"),
                "authorization",
                "x-api-key",
            ),
        ] {
            let mut profile = profile("anthropic", endpoint);
            profile.provider_meta = field.map(|field| json!({"apiKeyField":field}));
            let request = authenticate(client.post(endpoint), &profile, "adapter-test-key")
                .unwrap()
                .build()
                .unwrap();
            assert!(request.headers().contains_key(expected));
            assert!(!request.headers().contains_key(absent));
        }
    }
    #[test]
    fn original_preset_metadata_is_restored_before_vendor_inference() {
        let profile = profile("openai-chat", "https://api.siliconflow.cn/v1");
        let provider = project(&profile, "");
        let config = reasoning_config(&provider, &json!({"model":"minimax/m2"})).unwrap();
        assert_eq!(config.supports_effort, Some(true));
        assert_eq!(config.thinking_param.as_deref(), Some("enable_thinking"));
        assert_eq!(config.effort_param.as_deref(), Some("reasoning_effort"));
    }
    #[test]
    fn explicit_metadata_overrides_bundled_presets() {
        let mut profile = profile("openai-chat", "https://api.siliconflow.cn/v1");
        profile.provider_meta = Some(
            json!({"codexChatReasoning":{"supportsEffort":true,"thinkingParam":"none","effortParam":"reasoning.effort","effortValueMode":"passthrough"}}),
        );
        let provider = project(&profile, "");
        assert_eq!(
            reasoning_config(&provider, &json!({"model":"any"}))
                .unwrap()
                .effort_param
                .as_deref(),
            Some("reasoning.effort")
        );
    }
}
