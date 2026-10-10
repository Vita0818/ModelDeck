//! Import existing model connections without managing native MCP configuration.
use cc_switch_lib::{
    get_claude_settings_path, get_grok_config_path, import_default_config_test_hook, AppError,
    AppType, MultiAppConfig, ProviderService,
};
use serde_json::json;
use std::fs;
#[path = "support.rs"]
mod support;
use support::{create_test_state_with_config, ensure_test_home, reset_test_fs, test_mutex};

#[test]
fn import_default_config_claude_persists_provider() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let settings_path = get_claude_settings_path();
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent).expect("create claude settings dir");
    }
    let settings = json!({
        "env": {
            "ANTHROPIC_AUTH_TOKEN": "test-key",
            "ANTHROPIC_BASE_URL": "https://api.test"
        }
    });
    fs::write(
        &settings_path,
        serde_json::to_string_pretty(&settings).expect("serialize settings"),
    )
    .expect("seed claude settings.json");

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Claude);
    let state = create_test_state_with_config(&config).expect("create test state");

    import_default_config_test_hook(&state, AppType::Claude)
        .expect("import default config succeeds");

    // 验证内存状态
    let providers = state
        .db
        .get_all_providers(AppType::Claude.as_str())
        .expect("get all providers");
    let current_id = state
        .db
        .get_current_provider(AppType::Claude.as_str())
        .expect("get current provider");
    assert_eq!(current_id.as_deref(), Some("default"));
    let default_provider = providers.get("default").expect("default provider");
    assert_eq!(
        default_provider.settings_config, settings,
        "default provider should capture live settings"
    );

    // 验证数据已持久化到数据库（v3.7.0+ 使用 SQLite 而非 config.json）
    let db_path = home.join(".cc-switch").join("cc-switch.db");
    assert!(
        db_path.exists(),
        "importing default config should persist to cc-switch.db"
    );
}

#[test]
fn import_default_config_grokbuild_seeds_official_alongside_default() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let config_path = get_grok_config_path();
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).expect("create grok config dir");
    }
    fs::write(
        &config_path,
        r#"[models]
default = "grok-4.5"

[model."grok-4.5"]
model = "grok-4.5"
base_url = "https://example.com/v1"
name = "Example"
api_key = "secret"
api_backend = "responses"
context_window = 500000
"#,
    )
    .expect("seed grok config.toml");

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::GrokBuild);
    let state = create_test_state_with_config(&config).expect("create test state");

    import_default_config_test_hook(&state, AppType::GrokBuild)
        .expect("import default config succeeds");

    let providers = state
        .db
        .get_all_providers(AppType::GrokBuild.as_str())
        .expect("get all providers");
    assert!(
        providers.get("default").is_some(),
        "live imported as default"
    );

    // 初次导入已有配置时应同时补出官方入口（其它应用靠首启动主播种，
    // grokbuild 种子晚于该 flag，挂在导入动作上）
    let official = providers
        .get("grokbuild-official")
        .expect("official seed ensured alongside import");
    assert_eq!(official.category.as_deref(), Some("official"));

    // 激活的仍是导入的原配置，官方入口只是备选
    let current_id = state
        .db
        .get_current_provider(AppType::GrokBuild.as_str())
        .expect("get current provider");
    assert_eq!(current_id.as_deref(), Some("default"));
}

#[test]
fn import_default_config_grokbuild_official_live_imports_official_as_current() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    // 官方登录态的 live：无自定义模型表（允许 MCP 等其它内容）。
    // 导入的正确结果 = Grok Official 成为当前供应商，而非报错。
    let config_path = get_grok_config_path();
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).expect("create grok config dir");
    }
    fs::write(&config_path, "[mcp_servers.echo]\ncommand = \"echo\"\n")
        .expect("seed official-mode grok config.toml");

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::GrokBuild);
    let state = create_test_state_with_config(&config).expect("create test state");

    let imported = import_default_config_test_hook(&state, AppType::GrokBuild)
        .expect("official-mode live imports as the official provider");
    assert!(imported, "official-mode import should report success");

    let providers = state
        .db
        .get_all_providers(AppType::GrokBuild.as_str())
        .expect("get all providers");
    let official = providers
        .get("grokbuild-official")
        .expect("official entry ensured by import");
    assert_eq!(official.category.as_deref(), Some("official"));
    assert!(
        providers.get("default").is_none(),
        "official-mode live must not be imported as a custom default"
    );

    let current_id = state
        .db
        .get_current_provider(AppType::GrokBuild.as_str())
        .expect("get current provider");
    assert_eq!(
        current_id.as_deref(),
        Some("grokbuild-official"),
        "official entry should become current to mirror the live state"
    );
}

#[test]
fn startup_import_grokbuild_official_live_does_not_resurrect_official() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    // 启动自动导入走 service 层（lib.rs 启动循环直接调用它）：官方态 live
    // 必须报错且不产出任何条目——全项目惯例是启动自动导入只产出 default、
    // 从不产出官方条目，否则删掉的官方条目每次重启都会复活。
    // 官方态的成功导入只挂在手动导入的命令层。
    let config_path = get_grok_config_path();
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).expect("create grok config dir");
    }
    fs::write(&config_path, "").expect("seed empty official-mode grok config.toml");

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::GrokBuild);
    let state = create_test_state_with_config(&config).expect("create test state");

    ProviderService::import_default_config(&state, AppType::GrokBuild)
        .expect_err("startup auto-import must not import official-mode live");

    let providers = state
        .db
        .get_all_providers(AppType::GrokBuild.as_str())
        .expect("get all providers");
    assert!(
        providers.is_empty(),
        "startup auto-import must not create any provider from official-mode live"
    );
}

#[test]
fn import_default_config_grokbuild_broken_custom_live_still_errors() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    // 有自定义痕迹但残缺（[models] 存在、缺 [model.*]）：必须报真实错误，
    // 不能被误判成官方态静默吞掉；官方入口仍由命令层前置 ensure 补出。
    let config_path = get_grok_config_path();
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).expect("create grok config dir");
    }
    fs::write(&config_path, "[models]\ndefault = \"grok-4.5\"\n")
        .expect("seed broken custom grok config.toml");

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::GrokBuild);
    let state = create_test_state_with_config(&config).expect("create test state");

    import_default_config_test_hook(&state, AppType::GrokBuild)
        .expect_err("broken custom config should surface a validation error");

    let providers = state
        .db
        .get_all_providers(AppType::GrokBuild.as_str())
        .expect("get all providers");
    assert!(
        providers.get("grokbuild-official").is_some(),
        "official entry still appears via the pre-import ensure"
    );
    assert!(providers.get("default").is_none(), "nothing was imported");
    let current_id = state
        .db
        .get_current_provider(AppType::GrokBuild.as_str())
        .expect("get current provider");
    assert_ne!(
        current_id.as_deref(),
        Some("grokbuild-official"),
        "failed import must not silently activate the official entry"
    );
}

#[test]
fn import_default_config_without_live_file_returns_error() {
    use support::create_test_state;

    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let state = create_test_state().expect("create test state");

    let err = import_default_config_test_hook(&state, AppType::Claude)
        .expect_err("missing live file should error");
    match err {
        AppError::Localized { zh, .. } => assert!(
            zh.contains("Claude Code 配置文件不存在"),
            "unexpected error message: {zh}"
        ),
        AppError::Message(msg) => assert!(
            msg.contains("Claude Code 配置文件不存在"),
            "unexpected error message: {msg}"
        ),
        other => panic!("unexpected error variant: {other:?}"),
    }

    // 使用数据库架构，不再检查 config.json
    // 失败的导入不应该向数据库写入任何供应商
    let providers = state
        .db
        .get_all_providers(AppType::Claude.as_str())
        .expect("get all providers");
    assert!(
        providers.is_empty(),
        "failed import should not create any providers in database"
    );
}
