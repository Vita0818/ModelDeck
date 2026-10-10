//! Deep link module tests

use super::parser::parse_deeplink_url;
use super::provider::parse_and_merge_config;
use super::utils::{infer_homepage_from_endpoint, validate_url};
use super::DeepLinkImportRequest;
use crate::AppType;
use base64::prelude::*;
// =============================================================================
// Parser Tests
// =============================================================================

#[test]
fn modeldeck_links_preserve_legacy_import_contract() {
    let query = "v1/import?resource=provider&app=codex&name=ModelDeck%20Connection&endpoint=https%3A%2F%2Fexample.com%2Fv1";
    let current = parse_deeplink_url(&format!("modeldeck://{query}")).unwrap();
    let legacy = parse_deeplink_url(&format!("ccswitch://{query}")).unwrap();
    assert_eq!(
        serde_json::to_value(current).unwrap(),
        serde_json::to_value(legacy).unwrap()
    );
}

#[test]
fn test_parse_valid_claude_deeplink() {
    let url = "ccswitch://v1/import?resource=provider&app=claude&name=Test%20Provider&homepage=https%3A%2F%2Fexample.com&endpoint=https%3A%2F%2Fapi.example.com&apiKey=sk-test-123&icon=claude";

    let request = parse_deeplink_url(url).unwrap();

    assert_eq!(request.version, "v1");
    assert_eq!(request.resource, "provider");
    assert_eq!(request.app, Some("claude".to_string()));
    assert_eq!(request.name, Some("Test Provider".to_string()));
    assert_eq!(request.homepage, Some("https://example.com".to_string()));
    assert_eq!(
        request.endpoint,
        Some("https://api.example.com".to_string())
    );
    assert_eq!(request.api_key, Some("sk-test-123".to_string()));
    assert_eq!(request.icon, Some("claude".to_string()));
}

#[test]
fn test_parse_deeplink_with_notes() {
    let url = "ccswitch://v1/import?resource=provider&app=codex&name=Codex&homepage=https%3A%2F%2Fcodex.com&endpoint=https%3A%2F%2Fapi.codex.com&apiKey=key123&notes=Test%20notes";

    let request = parse_deeplink_url(url).unwrap();

    assert_eq!(request.notes, Some("Test notes".to_string()));
}

#[test]
fn pi_provider_deeplink_is_not_a_second_add_provider_entry() {
    let url = "ccswitch://v1/import?resource=provider&app=pi&name=Pi";
    assert!(parse_deeplink_url(url).is_err());
}

#[test]
fn test_parse_grokbuild_provider() {
    use super::provider::build_provider_from_request;

    let url = "ccswitch://v1/import?resource=provider&app=grokbuild&name=Grok%20Relay&endpoint=https%3A%2F%2Fapi.example.com%2Fv1&apiKey=secret&model=grok-4.5";

    let request = parse_deeplink_url(url).unwrap();

    assert_eq!(request.app.as_deref(), Some("grokbuild"));
    assert_eq!(request.name.as_deref(), Some("Grok Relay"));
    assert_eq!(
        request.endpoint.as_deref(),
        Some("https://api.example.com/v1")
    );
    assert_eq!(request.api_key.as_deref(), Some("secret"));
    assert_eq!(request.model.as_deref(), Some("grok-4.5"));

    let provider = build_provider_from_request(&AppType::GrokBuild, &request).unwrap();
    let config = provider.settings_config["config"].as_str().unwrap();
    let document = config.parse::<toml::Value>().unwrap();
    let model = &document["model"]["grok-4.5"];

    assert_eq!(document["models"]["default"].as_str(), Some("grok-4.5"));
    assert_eq!(
        model["base_url"].as_str(),
        Some("https://api.example.com/v1")
    );
    assert_eq!(model["name"].as_str(), Some("Grok Relay"));
    assert_eq!(model["api_key"].as_str(), Some("secret"));
    assert_eq!(model["api_backend"].as_str(), Some("responses"));
    assert_eq!(model["context_window"].as_integer(), Some(500_000));
}

#[test]
fn test_parse_invalid_scheme() {
    let url = "https://v1/import?resource=provider&app=claude&name=Test";

    let result = parse_deeplink_url(url);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid scheme"));
}

#[test]
fn test_parse_unsupported_version() {
    let url = "ccswitch://v2/import?resource=provider&app=claude&name=Test";

    let result = parse_deeplink_url(url);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Unsupported protocol version"));
}

#[test]
fn test_parse_missing_required_field() {
    // Name is still required even in v3.8+ (only homepage/endpoint/apiKey are optional)
    let url = "ccswitch://v1/import?resource=provider&app=claude";

    let result = parse_deeplink_url(url);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Missing 'name' parameter"));
}

// =============================================================================
// Utils Tests
// =============================================================================

#[test]
fn test_validate_invalid_url() {
    let result = validate_url("not-a-url", "test");
    assert!(result.is_err());
}

#[test]
fn test_validate_invalid_scheme() {
    let result = validate_url("ftp://example.com", "test");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("must be http or https"));
}

#[test]
fn test_infer_homepage() {
    assert_eq!(
        infer_homepage_from_endpoint("https://api.anthropic.com/v1"),
        Some("https://anthropic.com".to_string())
    );
    assert_eq!(
        infer_homepage_from_endpoint("https://api-test.company.com/v1"),
        Some("https://test.company.com".to_string())
    );
    assert_eq!(
        infer_homepage_from_endpoint("https://example.com"),
        Some("https://example.com".to_string())
    );
}

// =============================================================================
// Provider Tests
// =============================================================================

#[test]
fn test_build_gemini_provider_with_model() {
    use super::provider::build_provider_from_request;

    let request = DeepLinkImportRequest {
        version: "v1".to_string(),
        resource: "provider".to_string(),
        app: Some("gemini".to_string()),
        name: Some("Test Gemini".to_string()),
        homepage: Some("https://example.com".to_string()),
        endpoint: Some("https://api.example.com".to_string()),
        api_key: Some("test-api-key".to_string()),
        icon: None,
        model: Some("gemini-2.0-flash".to_string()),
        notes: None,
        haiku_model: None,
        sonnet_model: None,
        opus_model: None,
        config: None,
        config_format: None,
        config_url: None,
        content: None,
        description: None,
        enabled: None,
        usage_enabled: None,
        usage_script: None,
        usage_api_key: None,
        usage_base_url: None,
        usage_access_token: None,
        usage_user_id: None,
        usage_auto_interval: None,
    };

    let provider = build_provider_from_request(&AppType::Gemini, &request).unwrap();

    // Verify provider basic info
    assert_eq!(provider.name, "Test Gemini");
    assert_eq!(
        provider.website_url,
        Some("https://example.com".to_string())
    );

    // Verify settings_config structure
    let env = provider.settings_config["env"].as_object().unwrap();
    assert_eq!(env["GEMINI_API_KEY"], "test-api-key");
    assert_eq!(env["GOOGLE_GEMINI_BASE_URL"], "https://api.example.com");
    assert_eq!(env["GEMINI_MODEL"], "gemini-2.0-flash");
}

#[test]
fn test_build_gemini_provider_without_model() {
    use super::provider::build_provider_from_request;

    let request = DeepLinkImportRequest {
        version: "v1".to_string(),
        resource: "provider".to_string(),
        app: Some("gemini".to_string()),
        name: Some("Test Gemini".to_string()),
        homepage: Some("https://example.com".to_string()),
        endpoint: Some("https://api.example.com".to_string()),
        api_key: Some("test-api-key".to_string()),
        icon: None,
        model: None,
        notes: None,
        haiku_model: None,
        sonnet_model: None,
        opus_model: None,
        config: None,
        config_format: None,
        config_url: None,
        content: None,
        description: None,
        enabled: None,
        usage_enabled: None,
        usage_script: None,
        usage_api_key: None,
        usage_base_url: None,
        usage_access_token: None,
        usage_user_id: None,
        usage_auto_interval: None,
    };

    let provider = build_provider_from_request(&AppType::Gemini, &request).unwrap();

    let env = provider.settings_config["env"].as_object().unwrap();
    assert_eq!(env["GEMINI_API_KEY"], "test-api-key");
    assert_eq!(env["GOOGLE_GEMINI_BASE_URL"], "https://api.example.com");
    // Model should not be present
    assert!(env.get("GEMINI_MODEL").is_none());
}

#[test]
fn test_deeplink_usage_script_does_not_copy_provider_credentials() {
    use super::provider::build_provider_from_request;

    let request = DeepLinkImportRequest {
        version: "v1".to_string(),
        resource: "provider".to_string(),
        app: Some("claude".to_string()),
        name: Some("Test Claude".to_string()),
        homepage: Some("https://example.com".to_string()),
        endpoint: Some("https://api.example.com/v1/".to_string()),
        api_key: Some("sk-main".to_string()),
        icon: None,
        model: None,
        notes: None,
        haiku_model: None,
        sonnet_model: None,
        opus_model: None,
        config: None,
        config_format: None,
        config_url: None,
        content: None,
        description: None,
        enabled: None,
        usage_enabled: Some(true),
        usage_script: None,
        usage_api_key: None,
        usage_base_url: None,
        usage_access_token: None,
        usage_user_id: None,
        usage_auto_interval: None,
    };

    let provider = build_provider_from_request(&AppType::Claude, &request).unwrap();
    let script = provider
        .meta
        .as_ref()
        .and_then(|meta| meta.usage_script.as_ref())
        .expect("usage script should be created");

    assert!(script.enabled);
    assert_eq!(script.api_key, None);
    assert_eq!(script.base_url, None);
}

/// 构造一个只带用量脚本字段的 provider 请求，其余保持最小。
fn usage_script_request(code: &str, usage_enabled: Option<bool>) -> DeepLinkImportRequest {
    DeepLinkImportRequest {
        version: "v1".to_string(),
        resource: "provider".to_string(),
        app: Some("claude".to_string()),
        name: Some("Test Claude".to_string()),
        homepage: Some("https://example.com".to_string()),
        endpoint: Some("https://api.example.com/v1/".to_string()),
        api_key: Some("sk-main".to_string()),
        icon: None,
        model: None,
        notes: None,
        haiku_model: None,
        sonnet_model: None,
        opus_model: None,
        config: None,
        config_format: None,
        config_url: None,
        content: None,
        description: None,
        enabled: None,
        usage_enabled,
        usage_script: Some(BASE64_STANDARD.encode(code)),
        usage_api_key: None,
        usage_base_url: None,
        usage_access_token: None,
        usage_user_id: None,
        usage_auto_interval: None,
    }
}

#[test]
fn test_deeplink_usage_script_is_not_enabled_merely_by_carrying_code() {
    use super::provider::build_provider_from_request;

    // deeplink 是第三方构造、经浏览器抵达的不可信载荷。「带了代码」不是用户的
    // 启用决定——否则一条链接就能让这段 JS 在用户从未勾选的情况下进入启用态。
    let code = "export async function query() { return { cost: 0 }; }";
    let request = usage_script_request(code, None);

    let provider = build_provider_from_request(&AppType::Claude, &request).unwrap();
    let script = provider
        .meta
        .as_ref()
        .and_then(|meta| meta.usage_script.as_ref())
        .expect("usage script should still be created");

    assert!(
        !script.enabled,
        "缺省必须是未启用；`带了代码`不构成用户的启用决定"
    );
    // 代码本身仍要保留：确认框要展示它，用户之后也可在应用内手动开启。
    assert_eq!(script.code, code);
}

#[test]
fn test_deeplink_usage_script_honors_an_explicit_enable_request_from_the_link() {
    use super::provider::build_provider_from_request;

    // `usageEnabled=true` 是**链接作者**的请求，不是用户的选择——用户的同意体现在
    // 看过确认框里完整的脚本正文与启用状态之后点了导入。收紧默认值不能顺手把这条
    // 正常通路改坏：合作伙伴的预设链接靠它一次性配好用量查询。
    let code = "export async function query() { return { cost: 0 }; }";
    let request = usage_script_request(code, Some(true));

    let provider = build_provider_from_request(&AppType::Claude, &request).unwrap();
    let script = provider
        .meta
        .as_ref()
        .and_then(|meta| meta.usage_script.as_ref())
        .expect("usage script should be created");

    assert!(script.enabled);
    assert_eq!(script.code, code);
}

#[test]
fn test_deeplink_usage_script_omits_explicit_credentials_that_match_provider() {
    use super::provider::build_provider_from_request;

    let request = DeepLinkImportRequest {
        version: "v1".to_string(),
        resource: "provider".to_string(),
        app: Some("claude".to_string()),
        name: Some("Test Claude".to_string()),
        homepage: Some("https://example.com".to_string()),
        endpoint: Some("https://api.example.com/v1/".to_string()),
        api_key: Some("sk-main".to_string()),
        icon: None,
        model: None,
        notes: None,
        haiku_model: None,
        sonnet_model: None,
        opus_model: None,
        config: None,
        config_format: None,
        config_url: None,
        content: None,
        description: None,
        enabled: None,
        usage_enabled: Some(true),
        usage_script: None,
        usage_api_key: Some(" sk-main ".to_string()),
        usage_base_url: Some(" https://api.example.com/v1/ ".to_string()),
        usage_access_token: None,
        usage_user_id: None,
        usage_auto_interval: None,
    };

    let provider = build_provider_from_request(&AppType::Claude, &request).unwrap();
    let script = provider
        .meta
        .as_ref()
        .and_then(|meta| meta.usage_script.as_ref())
        .expect("usage script should be created");

    assert_eq!(script.api_key, None);
    assert_eq!(script.base_url, None);
}

#[test]
fn test_deeplink_usage_script_preserves_distinct_usage_credentials() {
    use super::provider::build_provider_from_request;

    let request = DeepLinkImportRequest {
        version: "v1".to_string(),
        resource: "provider".to_string(),
        app: Some("claude".to_string()),
        name: Some("Test Claude".to_string()),
        homepage: Some("https://example.com".to_string()),
        endpoint: Some("https://api.example.com/v1".to_string()),
        api_key: Some("sk-main".to_string()),
        icon: None,
        model: None,
        notes: None,
        haiku_model: None,
        sonnet_model: None,
        opus_model: None,
        config: None,
        config_format: None,
        config_url: None,
        content: None,
        description: None,
        enabled: None,
        usage_enabled: Some(true),
        usage_script: None,
        usage_api_key: Some(" sk-usage ".to_string()),
        usage_base_url: Some(" https://usage.example/api/ ".to_string()),
        usage_access_token: None,
        usage_user_id: None,
        usage_auto_interval: None,
    };

    let provider = build_provider_from_request(&AppType::Claude, &request).unwrap();
    let script = provider
        .meta
        .as_ref()
        .and_then(|meta| meta.usage_script.as_ref())
        .expect("usage script should be created");

    assert_eq!(script.api_key.as_deref(), Some("sk-usage"));
    assert_eq!(
        script.base_url.as_deref(),
        Some("https://usage.example/api")
    );
}

#[test]
fn test_parse_and_merge_config_claude() {
    // Prepare Base64 encoded Claude config
    let config_json = r#"{"env":{"ANTHROPIC_AUTH_TOKEN":"sk-ant-xxx","ANTHROPIC_BASE_URL":"https://api.anthropic.com/v1","ANTHROPIC_MODEL":"claude-sonnet-4.5"}}"#;
    let config_b64 = BASE64_STANDARD.encode(config_json.as_bytes());

    let request = DeepLinkImportRequest {
        version: "v1".to_string(),
        resource: "provider".to_string(),
        app: Some("claude".to_string()),
        name: Some("Test".to_string()),
        homepage: None,
        endpoint: None,
        api_key: None,
        icon: None,
        model: None,
        notes: None,
        haiku_model: None,
        sonnet_model: None,
        opus_model: None,
        config: Some(config_b64),
        config_format: Some("json".to_string()),
        config_url: None,
        content: None,
        description: None,
        enabled: None,
        usage_enabled: None,
        usage_script: None,
        usage_api_key: None,
        usage_base_url: None,
        usage_access_token: None,
        usage_user_id: None,
        usage_auto_interval: None,
    };

    let merged = parse_and_merge_config(&request).unwrap();

    // Should auto-fill from config
    assert_eq!(merged.api_key, Some("sk-ant-xxx".to_string()));
    assert_eq!(
        merged.endpoint,
        Some("https://api.anthropic.com/v1".to_string())
    );
    assert_eq!(merged.homepage, Some("https://anthropic.com".to_string()));
    assert_eq!(merged.model, Some("claude-sonnet-4.5".to_string()));
}

#[test]
fn test_parse_and_merge_config_codex_uses_bearer_token() {
    let config_toml = r#"model_provider = "rightcode"
model = "gpt-5-codex"

[model_providers.rightcode]
base_url = "https://rightcode.example/v1"
wire_api = "responses"
experimental_bearer_token = "sk-rightcode"
"#;
    let config_json = serde_json::json!({
        "auth": {},
        "config": config_toml,
    })
    .to_string();
    let config_b64 = BASE64_STANDARD.encode(config_json.as_bytes());

    let request = DeepLinkImportRequest {
        version: "v1".to_string(),
        resource: "provider".to_string(),
        app: Some("codex".to_string()),
        name: Some("RightCode".to_string()),
        config: Some(config_b64),
        config_format: Some("json".to_string()),
        ..Default::default()
    };

    let merged = parse_and_merge_config(&request).unwrap();

    assert_eq!(merged.api_key, Some("sk-rightcode".to_string()));
    assert_eq!(
        merged.endpoint,
        Some("https://rightcode.example/v1".to_string())
    );
    assert_eq!(
        merged.homepage,
        Some("https://rightcode.example".to_string())
    );
    assert_eq!(merged.model, Some("gpt-5-codex".to_string()));
}

#[test]
fn test_parse_and_merge_config_grokbuild() {
    let config_toml = r#"[models]
default = "grok-profile"

[model."grok-profile"]
model = "grok-upstream"
base_url = "https://grok.example/v1"
name = "Grok Relay"
api_key = "sk-grok"
api_backend = "responses"
context_window = 500000
"#;
    let config_json = serde_json::json!({ "config": config_toml }).to_string();
    let request = DeepLinkImportRequest {
        version: "v1".to_string(),
        resource: "provider".to_string(),
        app: Some("grokbuild".to_string()),
        name: Some("Grok Relay".to_string()),
        config: Some(BASE64_STANDARD.encode(config_json.as_bytes())),
        config_format: Some("json".to_string()),
        ..Default::default()
    };

    let merged = parse_and_merge_config(&request).expect("merge Grok Build config");

    assert_eq!(merged.api_key.as_deref(), Some("sk-grok"));
    assert_eq!(merged.endpoint.as_deref(), Some("https://grok.example/v1"));
    assert_eq!(merged.model.as_deref(), Some("grok-upstream"));
    assert_eq!(merged.homepage.as_deref(), Some("https://grok.example"));
}

#[test]
fn test_parse_and_merge_config_url_override() {
    let config_json = r#"{"env":{"ANTHROPIC_AUTH_TOKEN":"sk-old","ANTHROPIC_BASE_URL":"https://api.anthropic.com/v1"}}"#;
    let config_b64 = BASE64_STANDARD.encode(config_json.as_bytes());

    let request = DeepLinkImportRequest {
        version: "v1".to_string(),
        resource: "provider".to_string(),
        app: Some("claude".to_string()),
        name: Some("Test".to_string()),
        homepage: None,
        endpoint: None,
        api_key: Some("sk-new".to_string()), // URL param should override
        icon: None,
        model: None,
        notes: None,
        haiku_model: None,
        sonnet_model: None,
        opus_model: None,
        config: Some(config_b64),
        config_format: Some("json".to_string()),
        config_url: None,
        content: None,
        description: None,
        enabled: None,
        usage_enabled: None,
        usage_script: None,
        usage_api_key: None,
        usage_base_url: None,
        usage_access_token: None,
        usage_user_id: None,
        usage_auto_interval: None,
    };

    let merged = parse_and_merge_config(&request).unwrap();

    // URL param should take priority
    assert_eq!(merged.api_key, Some("sk-new".to_string()));
    // Config file value should be used
    assert_eq!(
        merged.endpoint,
        Some("https://api.anthropic.com/v1".to_string())
    );
}

#[test]
fn test_build_claude_provider_preserves_custom_env_fields() {
    // Regression test for: deeplink import dropped non-standard env fields
    // such as ANTHROPIC_CUSTOM_HEADERS, even though the preview dialog
    // showed them. The preview and the actual persisted provider must
    // contain the same env keys.
    use super::provider::build_provider_from_request;

    let config_json = r#"{"env":{
        "ANTHROPIC_AUTH_TOKEN":"sk-ant-xxx",
        "ANTHROPIC_BASE_URL":"https://api.example.com",
        "ANTHROPIC_CUSTOM_HEADERS":"Cookie: session=abc",
        "API_TIMEOUT_MS":"3000000",
        "CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS":"1",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL":"haiku-from-config"
    }}"#;
    let config_b64 = BASE64_STANDARD.encode(config_json.as_bytes());

    let request = DeepLinkImportRequest {
        version: "v1".to_string(),
        resource: "provider".to_string(),
        app: Some("claude".to_string()),
        name: Some("My Provider".to_string()),
        homepage: Some("https://example.com".to_string()),
        endpoint: Some("https://api.example.com".to_string()),
        api_key: Some("sk-ant-xxx".to_string()),
        icon: None,
        // URL param: must win over the same key in config (haiku-from-config)
        model: Some("main-model".to_string()),
        notes: None,
        haiku_model: Some("haiku-from-url".to_string()),
        sonnet_model: None,
        opus_model: None,
        config: Some(config_b64),
        config_format: Some("json".to_string()),
        config_url: None,
        content: None,
        description: None,
        enabled: None,
        usage_enabled: None,
        usage_script: None,
        usage_api_key: None,
        usage_base_url: None,
        usage_access_token: None,
        usage_user_id: None,
        usage_auto_interval: None,
    };

    let provider = build_provider_from_request(&AppType::Claude, &request).unwrap();
    let env = provider.settings_config["env"].as_object().unwrap();

    // Custom env fields from `config` must survive import
    assert_eq!(env["ANTHROPIC_CUSTOM_HEADERS"], "Cookie: session=abc");
    assert_eq!(env["API_TIMEOUT_MS"], "3000000");
    assert_eq!(env["CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS"], "1");

    // Standard fields from URL params win over config
    assert_eq!(env["ANTHROPIC_AUTH_TOKEN"], "sk-ant-xxx");
    assert_eq!(env["ANTHROPIC_BASE_URL"], "https://api.example.com");
    assert_eq!(env["ANTHROPIC_MODEL"], "main-model");
    assert_eq!(env["ANTHROPIC_DEFAULT_HAIKU_MODEL"], "haiku-from-url");
}

#[test]
fn test_build_claude_provider_without_config_unchanged() {
    // Backward compatibility: deeplinks without a `config` field still
    // produce exactly the same env shape as before — only the standard
    // ANTHROPIC_* keys, nothing else.
    use super::provider::build_provider_from_request;

    let request = DeepLinkImportRequest {
        version: "v1".to_string(),
        resource: "provider".to_string(),
        app: Some("claude".to_string()),
        name: Some("Plain".to_string()),
        homepage: Some("https://example.com".to_string()),
        endpoint: Some("https://api.example.com".to_string()),
        api_key: Some("sk".to_string()),
        icon: None,
        model: None,
        notes: None,
        haiku_model: None,
        sonnet_model: None,
        opus_model: None,
        config: None,
        config_format: None,
        config_url: None,
        content: None,
        description: None,
        enabled: None,
        usage_enabled: None,
        usage_script: None,
        usage_api_key: None,
        usage_base_url: None,
        usage_access_token: None,
        usage_user_id: None,
        usage_auto_interval: None,
    };

    let provider = build_provider_from_request(&AppType::Claude, &request).unwrap();
    let env = provider.settings_config["env"].as_object().unwrap();

    assert_eq!(env["ANTHROPIC_AUTH_TOKEN"], "sk");
    assert_eq!(env["ANTHROPIC_BASE_URL"], "https://api.example.com");
    // No extras leaked in
    assert_eq!(env.len(), 2);
}

// =============================================================================
// Multiple Endpoints Tests
// =============================================================================

#[test]
fn test_parse_multiple_endpoints_comma_separated() {
    let url = "ccswitch://v1/import?resource=provider&app=claude&name=Test&endpoint=https%3A%2F%2Fapi1.example.com,https%3A%2F%2Fapi2.example.com,https%3A%2F%2Fapi3.example.com&apiKey=sk-test";

    let request = parse_deeplink_url(url).unwrap();

    assert!(request.endpoint.is_some());
    let endpoint = request.endpoint.unwrap();
    // Should contain all endpoints comma-separated
    assert!(endpoint.contains("https://api1.example.com"));
    assert!(endpoint.contains("https://api2.example.com"));
    assert!(endpoint.contains("https://api3.example.com"));
}

#[test]
fn test_parse_single_endpoint_backward_compatible() {
    // Old format with single endpoint should still work
    let url = "ccswitch://v1/import?resource=provider&app=claude&name=Test&endpoint=https%3A%2F%2Fapi.example.com&apiKey=sk-test";

    let request = parse_deeplink_url(url).unwrap();

    assert_eq!(
        request.endpoint,
        Some("https://api.example.com".to_string())
    );
}

#[test]
fn test_parse_endpoints_with_spaces_trimmed() {
    let url = "ccswitch://v1/import?resource=provider&app=claude&name=Test&endpoint=https%3A%2F%2Fapi1.example.com%20,%20https%3A%2F%2Fapi2.example.com&apiKey=sk-test";

    let request = parse_deeplink_url(url).unwrap();

    // Validation should pass (spaces are trimmed during validation)
    assert!(request.endpoint.is_some());
}

#[test]
fn test_infer_homepage_from_endpoint_without_homepage() {
    // Test that homepage is auto-inferred from endpoint when not provided
    assert_eq!(
        infer_homepage_from_endpoint("https://api.cubence.com/v1"),
        Some("https://cubence.com".to_string())
    );
    assert_eq!(
        infer_homepage_from_endpoint("https://cubence.com"),
        Some("https://cubence.com".to_string())
    );
}
