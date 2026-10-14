mod app_config;
mod app_store;
mod auto_launch;
mod claude_desktop_config;
mod codex_config;
mod commands;
mod config;
mod database;
mod deck;
mod deeplink;
mod error;
mod grok_config;
pub mod hermes_config;
mod init_status;
#[cfg(target_os = "linux")]
mod linux_fix;
mod model_capabilities;
mod model_clients;
mod opencode_config;
mod panic_hook;
mod product;
mod provider;
mod proxy;
mod services;
mod settings;
mod store;
mod tray;
mod usage_events;

pub use app_config::{AppType, MultiAppConfig};
pub use codex_config::{get_codex_auth_path, get_codex_config_path};
pub use commands::proxy::{
    get_default_cost_multiplier_test_hook, get_pricing_model_source_test_hook,
    set_default_cost_multiplier_test_hook, set_pricing_model_source_test_hook,
};
pub use config::{get_claude_settings_path, read_json_file};
pub use database::Database;
pub use deeplink::{import_provider_from_deeplink, parse_deeplink_url, DeepLinkImportRequest};
pub use error::AppError;
pub use grok_config::get_grok_config_path;
pub use provider::{Provider, ProviderMeta};
pub use settings::{update_settings, AppSettings};
pub use store::AppState;

use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
#[cfg(target_os = "macos")]
use tauri::image::Image;
use tauri::{Emitter, Manager, RunEvent};
#[cfg(any(target_os = "windows", target_os = "linux"))]
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

#[derive(Debug, PartialEq, Eq)]
enum ExitRequestAction {
    PreventSystemExit,
    DeferToTauriRestart,
    CleanupAndExit,
}

fn classify_exit_request(code: Option<i32>) -> ExitRequestAction {
    match code {
        None => ExitRequestAction::PreventSystemExit,
        Some(tauri::RESTART_EXIT_CODE) => ExitRequestAction::DeferToTauriRestart,
        Some(_) => ExitRequestAction::CleanupAndExit,
    }
}

#[tauri::command]
async fn update_tray_menu(app: tauri::AppHandle) -> Result<bool, String> {
    tray::refresh_tray_menu(&app);
    Ok(true)
}

pub async fn cleanup_before_exit(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Err(error) = state.proxy_service.stop_for_exit().await {
            log::warn!("Cannot stop the ModelDeck gateway: {error}");
        }
    }
}

pub fn run() {
    panic_hook::setup_panic_hook();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            for arg in args {
                let _ = handle_deeplink_url(app, &arg, false, "single instance");
            }
            tray::show_main_window(app);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(window_state_flags())
                .build(),
        )
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let app = window.app_handle();
                if settings::get_settings().minimize_to_tray_on_close
                    && app.tray_by_id(tray::TRAY_ID).is_some()
                {
                    let _ = window.hide();
                    #[cfg(target_os = "windows")]
                    let _ = window.set_skip_taskbar(true);
                    #[cfg(target_os = "macos")]
                    tray::apply_tray_policy(app, false);
                } else {
                    app.exit(0);
                }
            }
        })
        .setup(|app| {
            app_store::refresh_app_config_dir_override(app.handle());
            panic_hook::init_app_config_dir(config::get_app_config_dir());
            let log_dir = panic_hook::get_log_dir();
            std::fs::create_dir_all(&log_dir)?;
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Trace)
                    .filter(|metadata| runtime_log_level_allows(metadata.level(), log::max_level()))
                    .targets([
                        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Folder {
                            path: log_dir,
                            file_name: Some("modeldeck".into()),
                        }),
                    ])
                    .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(4))
                    .max_file_size(20 * 1024 * 1024)
                    .build(),
            )?;
            log::set_max_level(log::LevelFilter::Info);
            log::info!("ModelDeck v{}", env!("CARGO_PKG_VERSION"));
            usage_events::init(app.handle().clone());
            let db_path = config::get_app_config_dir().join("cc-switch.db");
            if let Ok(Some(version)) = Database::stored_user_version_exceeds_supported(&db_path) {
                init_status::set_init_error(init_status::InitErrorPayload {
                    path: db_path.display().to_string(),
                    error: "A compatible ModelDeck build is required for this database".into(),
                    kind: Some("db_version_too_new".into()),
                    db_version: Some(version),
                    supported_version: Some(database::SCHEMA_VERSION),
                });
                tray::show_main_window(app.handle());
                return Ok(());
            }
            let initialized = (|| -> Result<Arc<Database>, AppError> {
                let json_path = config::get_app_config_dir().join("config.json");
                let legacy = if !db_path.exists() && json_path.exists() {
                    Some(MultiAppConfig::load()?)
                } else {
                    None
                };
                let db = Arc::new(Database::init()?);
                if let Some(legacy) = legacy {
                    db.migrate_from_json(&legacy)?;
                    init_status::set_migration_success();
                }
                if let Ok(config) = db.get_log_config() {
                    log::set_max_level(config.to_level_filter());
                }
                let proxy_url = db.get_setting("global_proxy_url")?;
                proxy::http_client::init(proxy_url.as_deref()).map_err(AppError::Message)?;
                Ok(db)
            })();
            let db = match initialized {
                Ok(db) => db,
                Err(error) => {
                    init_status::set_init_error(init_status::InitErrorPayload {
                        path: db_path.display().to_string(),
                        error: error.to_string(),
                        kind: None,
                        db_version: None,
                        supported_version: None,
                    });
                    tray::show_main_window(app.handle());
                    return Ok(());
                }
            };
            let state = AppState::new(db.clone());
            app.manage(state.clone());
            if settings::get_settings().show_in_tray {
                let menu = tray::create_tray_menu(app.handle(), &state)?;
                let mut icon = tauri::tray::TrayIconBuilder::with_id(tray::TRAY_ID)
                    .tooltip(product::APP_NAME)
                    .menu(&menu)
                    .on_menu_event(|app, event| {
                        tray::handle_tray_menu_event(app, event.id.as_ref())
                    });
                #[cfg(target_os = "macos")]
                if let Some(image) = macos_tray_icon() {
                    icon = icon.icon(image).icon_as_template(true);
                }
                #[cfg(not(target_os = "macos"))]
                if let Some(image) = app.default_window_icon() {
                    icon = icon.icon(image.clone());
                }
                icon.build(app)?;
            }
            if !settings::get_settings().silent_startup {
                tray::show_main_window(app.handle());
            }
            let startup = state.clone();
            tauri::async_runtime::spawn(async move {
                let manually_enabled = startup
                    .db
                    .get_global_proxy_config()
                    .await
                    .map(|config| config.proxy_enabled)
                    .unwrap_or(false);
                if manually_enabled || deck::needs_gateway() || model_clients::needs_gateway() {
                    if let Err(error) = startup.proxy_service.start().await {
                        log::warn!("Gateway startup: {error}");
                    }
                }
            });
            tauri::async_runtime::spawn(async move {
                loop {
                    let db = db.clone();
                    let _ = tauri::async_runtime::spawn_blocking(move || {
                        let _guard = services::session_usage::session_sync_mutex().blocking_lock();
                        if settings::get_settings().session_auto_sync_enabled {
                            for error in services::session_usage::sync_all_unlocked(&db).errors {
                                log::warn!("Native usage synchronization: {error}");
                            }
                        }
                        if let Err(error) = db.periodic_backup_if_needed() {
                            log::warn!("Local database backup: {error}");
                        }
                    })
                    .await;
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
            });
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            if let Err(error) = app.deep_link().register_all() {
                log::warn!("Deep link registration: {error}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            deck::get_deck,
            deck::save_deck,
            deck::apply_deck_binding,
            deck::reset_deck_binding,
            update_tray_menu,
            commands::get_settings,
            commands::save_settings,
            commands::restart_app,
            commands::get_app_config_dir_override,
            commands::set_app_config_dir_override,
            commands::set_auto_launch,
            commands::get_auto_launch_status,
            commands::get_log_config,
            commands::set_log_config,
            commands::get_config_dir,
            commands::get_app_config_path,
            commands::get_claude_code_config_path,
            commands::open_config_folder,
            commands::open_app_config_folder,
            commands::pick_directory,
            commands::check_env_conflicts,
            commands::delete_env_vars,
            commands::restore_env_backup,
            commands::get_global_proxy_url,
            commands::set_global_proxy_url,
            commands::scan_local_proxies,
            commands::test_proxy_url,
            commands::get_upstream_proxy_status,
            commands::start_proxy_server,
            commands::stop_proxy_server,
            commands::get_proxy_status,
            commands::get_global_proxy_config,
            commands::update_global_proxy_config,
            commands::get_default_cost_multiplier,
            commands::set_default_cost_multiplier,
            commands::get_pricing_model_source,
            commands::set_pricing_model_source,
            commands::get_usage_summary,
            commands::get_usage_summary_by_app,
            commands::check_provider_limits,
            commands::get_usage_trends,
            commands::get_provider_stats,
            commands::get_model_stats,
            commands::get_request_logs,
            commands::get_request_detail,
            commands::get_model_pricing,
            commands::update_model_pricing,
            commands::update_model_pricing_batch,
            commands::delete_model_pricing,
            commands::get_usage_data_sources,
            commands::sync_session_usage,
            commands::rebuild_codex_usage,
            commands::get_models_dev_sync_config,
            commands::save_models_dev_sync_config,
            commands::record_models_dev_sync_result,
            commands::get_providers,
            commands::get_current_provider,
            commands::parse_deeplink,
            commands::merge_deeplink_config,
            commands::import_from_deeplink_unified,
            commands::export_config_to_file,
            commands::import_config_from_file,
            commands::save_file_dialog,
            commands::open_file_dialog,
            commands::create_db_backup,
            commands::list_db_backups,
            commands::restore_db_backup,
            commands::rename_db_backup,
            commands::delete_db_backup,
            commands::open_external,
            commands::copy_text_to_clipboard,
            commands::is_portable_mode,
            commands::get_init_error,
            commands::get_migration_result,
            commands::set_window_theme,
        ]);
    let app = builder
        .build(tauri::generate_context!())
        .expect("Cannot initialize ModelDeck");
    let quitting = AtomicBool::new(false);
    app.run(move |app, event| match event {
        RunEvent::ExitRequested { code, api, .. } => {
            match classify_exit_request(code) {
                ExitRequestAction::PreventSystemExit => {
                    api.prevent_exit();
                    return;
                }
                ExitRequestAction::DeferToTauriRestart => return,
                ExitRequestAction::CleanupAndExit => {}
            }
            if quitting.swap(true, Ordering::SeqCst) {
                return;
            }
            api.prevent_exit();
            save_window_state_before_exit(app);
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                cleanup_before_exit(&app).await;
                app.exit(0);
            });
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => tray::show_main_window(app),
        #[cfg(target_os = "macos")]
        RunEvent::Opened { urls } => {
            for url in urls {
                let _ = handle_deeplink_url(app, url.as_str(), true, "open URL");
            }
        }
        _ => {}
    });
}

pub(crate) struct RedactedUrl<'a> {
    url: &'a str,
    known_secrets: &'a [String],
}

impl fmt::Display for RedactedUrl<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&redact_url_for_log_with_secrets(
            self.url,
            self.known_secrets,
        ))
    }
}

/// 为日志提供惰性 URL 脱敏包装；只有日志实际输出时才解析和重建 URL。
pub(crate) fn url_for_log(url: &str) -> RedactedUrl<'_> {
    RedactedUrl {
        url,
        known_secrets: &[],
    }
}

/// 已知密钥参与子串脱敏的最短长度：过短的值(如 "api")当作子串会误伤无关文本，
/// 所以只对足够长、几乎不可能是普通词的值做替换。
const MIN_KNOWN_SECRET_LEN: usize = 8;

/// 唯一的密钥脱敏原语：把字符串里出现的、我们确切握有的密钥值替换为 [REDACTED]。
/// 不做任何“看起来像密钥”的形状猜测——只隐藏已知值，天然收敛、不误伤正常路径。
pub(crate) fn redact_known_secrets(text: &str, known_secrets: &[String]) -> String {
    redact_known_secrets_with_min_length(text, known_secrets, MIN_KNOWN_SECRET_LEN)
}

fn redact_known_secrets_with_min_length(
    text: &str,
    known_secrets: &[String],
    minimum_chars: usize,
) -> String {
    let mut output = text.to_string();
    for secret in known_secrets {
        if secret.chars().count() >= minimum_chars {
            output = output.replace(secret.as_str(), "[REDACTED]");
        }
    }
    output
}

/// 无 scheme 的裸 authority 形态(如 `user:pass@host/path`)剥掉 userinfo：
/// 仅当 `@` 出现在第一个 `/` 之前时才视为凭据。
fn strip_bare_userinfo(input: &str) -> &str {
    let authority_end = input.find('/').unwrap_or(input.len());
    match input[..authority_end].rfind('@') {
        Some(at) => &input[at + 1..],
        None => input,
    }
}

#[cfg(test)]
pub(crate) fn redact_url_for_log(url_str: &str) -> String {
    redact_url_for_log_with_secrets(url_str, &[])
}

/// 为日志脱敏 URL：剥掉 userinfo(user:pass@) 与整个 query/fragment，保留
/// scheme/host/port/path 供诊断(如 base_url 配错路径导致 404)，最后再抹掉已知密钥值。
pub(crate) fn redact_url_for_log_with_secrets(url_str: &str, known_secrets: &[String]) -> String {
    let scheme_relative = url_str.starts_with("//");
    let parsed = if scheme_relative {
        url::Url::parse(&format!("https:{url_str}"))
    } else {
        url::Url::parse(url_str)
    };

    let sanitized = match parsed {
        Ok(mut url) if url.has_host() => {
            let _ = url.set_username("");
            let _ = url.set_password(None);
            url.set_query(None);
            url.set_fragment(None);
            let rendered = url.as_str();
            if scheme_relative {
                rendered
                    .strip_prefix("https:")
                    .unwrap_or(rendered)
                    .to_string()
            } else {
                rendered.to_string()
            }
        }
        _ => {
            // 解析失败(相对路径、含裸 userinfo 的非法 URL 等)：丢掉 query/fragment，
            // 尽力剥掉 userinfo，其余原样保留。
            let without_tail = url_str.split(['?', '#']).next().unwrap_or(url_str);
            strip_bare_userinfo(without_tail).to_string()
        }
    };

    redact_known_secrets(&sanitized, known_secrets)
}

/// 只保留 `scheme://host:port`，丢掉 path/query/userinfo。用于我们手里没有任何
/// 已知密钥可脱敏 path 的场景——凭据可能整个内嵌在 base_url 的 path 里，此时
/// 记录 path 无法保证不泄漏，只能退回到 origin。
#[cfg(test)]
pub(crate) fn redact_url_origin_for_log(url_str: &str) -> String {
    let scheme_relative = url_str.starts_with("//");
    let parsed = if scheme_relative {
        url::Url::parse(&format!("https:{url_str}"))
    } else {
        url::Url::parse(url_str)
    };

    match parsed {
        Ok(url) if url.has_host() => {
            let authority = &url[url::Position::BeforeHost..url::Position::AfterPort];
            if scheme_relative {
                format!("//{authority}")
            } else {
                format!("{}://{authority}", url.scheme())
            }
        }
        _ => "[invalid target]".to_string(),
    }
}

fn runtime_log_level_allows(level: log::Level, max_level: log::LevelFilter) -> bool {
    max_level.to_level().is_some_and(|maximum| level <= maximum)
}

fn handle_deeplink_url(
    app: &tauri::AppHandle,
    url_str: &str,
    focus_main_window: bool,
    source: &str,
) -> bool {
    if !url_str.starts_with("modeldeck://") && !url_str.starts_with("ccswitch://") {
        return false;
    }

    log::info!(
        "✓ Deep link URL detected from {source}: {}",
        url_for_log(url_str)
    );

    match crate::deeplink::parse_deeplink_url(url_str) {
        Ok(request) => {
            log::info!(
                "✓ Successfully parsed deep link: resource={}, app={:?}, name={:?}",
                request.resource,
                request.app,
                request.name
            );

            if let Err(e) = app.emit("deeplink-import", &request) {
                log::error!("✗ Failed to emit deeplink-import event: {e}");
            } else {
                log::info!("✓ Emitted deeplink-import event to frontend");
            }

            if focus_main_window {
                if let Some(window) = app.get_webview_window("main") {
                    #[cfg(target_os = "windows")]
                    {
                        let _ = window.set_skip_taskbar(false);
                    }
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                    #[cfg(target_os = "linux")]
                    {
                        linux_fix::nudge_main_window(window.clone());
                    }
                    log::info!("✓ Window shown and focused");
                }
            }
        }
        Err(e) => {
            log::error!("✗ Failed to parse deep link URL: {e}");

            if let Err(emit_err) = app.emit(
                "deeplink-error",
                serde_json::json!({
                    "url": url_str,
                    "error": e.to_string()
                }),
            ) {
                log::error!("✗ Failed to emit deeplink-error event: {emit_err}");
            }
        }
    }

    true
}

#[cfg(target_os = "macos")]
fn macos_tray_icon() -> Option<Image<'static>> {
    const ICON_BYTES: &[u8] = include_bytes!("../icons/tray/macos/statusbar_template_3x.png");

    match Image::from_bytes(ICON_BYTES) {
        Ok(icon) => Some(icon),
        Err(err) => {
            log::warn!("Failed to load macOS tray icon: {err}");
            None
        }
    }
}

fn window_state_flags() -> StateFlags {
    StateFlags::POSITION | StateFlags::SIZE | StateFlags::MAXIMIZED
}

pub fn save_window_state_before_exit(app_handle: &tauri::AppHandle) {
    if let Err(err) = app_handle.save_window_state(window_state_flags()) {
        log::error!("退出前保存窗口状态失败: {err}");
    } else {
        log::info!("已在退出前保存窗口状态");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_exit_request, redact_url_for_log, redact_url_for_log_with_secrets,
        redact_url_origin_for_log, runtime_log_level_allows, ExitRequestAction,
    };

    #[test]
    fn log_url_redaction_strips_credentials_and_query_keeps_path() {
        // userinfo 与整个 query 剥离，path 保留用于诊断 base_url 配错。
        assert_eq!(
            redact_url_for_log(
                "https://user:secret@example.com:8443/v1/models?key=top-secret&alt=sse"
            ),
            "https://example.com:8443/v1/models"
        );
        // scheme-relative 保持形态，userinfo 去掉。
        assert_eq!(
            redact_url_for_log("//user:sk-secret@gw.example.com/v1"),
            "//gw.example.com/v1"
        );
        // 无 scheme 的裸 userinfo。
        assert_eq!(
            redact_url_for_log("user:sk-secret@gw.example.com/v1"),
            "gw.example.com/v1"
        );
        // 无法解析为绝对 URL 时：丢 query，其余原样保留。
        assert_eq!(redact_url_for_log("not-a-url?token=secret"), "not-a-url");
        // 不再对 path 段做“看起来像密钥”的形状猜测，正常路径完整保留。
        assert_eq!(
            redact_url_for_log("https://host.example/v1/models/gemini-2.5-pro"),
            "https://host.example/v1/models/gemini-2.5-pro"
        );
    }

    #[test]
    fn log_url_redaction_replaces_known_secret_values() {
        // 精确匹配已知密钥值：无论它出现在 path 还是别处都被抹掉。
        let secrets = vec!["k-9f3a7c2b1e".to_string()];
        assert_eq!(
            redact_url_for_log_with_secrets("https://gw.example.com/k-9f3a7c2b1e/v1", &secrets),
            "https://gw.example.com/[REDACTED]/v1"
        );
        // 过短(<8)的已知值不参与子串脱敏，避免误伤 /v1/ 之类的正常路径。
        let short_secrets = vec!["api".to_string()];
        assert_eq!(
            redact_url_for_log_with_secrets("https://api.example.com/v1", &short_secrets),
            "https://api.example.com/v1"
        );
    }

    #[test]
    fn log_url_origin_drops_path_for_credential_in_path() {
        // 没有已知密钥可脱敏时，凭据可能整个内嵌在 path，只记 origin。
        assert_eq!(
            redact_url_origin_for_log("https://gw.example.com/k-9f3a7c2b1e/v1"),
            "https://gw.example.com"
        );
        assert_eq!(
            redact_url_origin_for_log("https://user:pass@gw.example.com:8443/secret/v1"),
            "https://gw.example.com:8443"
        );
        assert_eq!(
            redact_url_origin_for_log("//gw.example.com/secret/v1"),
            "//gw.example.com"
        );
    }

    #[test]
    fn runtime_log_filter_honors_dynamic_max_level() {
        assert!(!runtime_log_level_allows(
            log::Level::Error,
            log::LevelFilter::Off
        ));
        assert!(runtime_log_level_allows(
            log::Level::Error,
            log::LevelFilter::Info
        ));
        assert!(runtime_log_level_allows(
            log::Level::Info,
            log::LevelFilter::Info
        ));
        assert!(!runtime_log_level_allows(
            log::Level::Debug,
            log::LevelFilter::Info
        ));
    }

    #[test]
    fn no_code_keeps_app_alive_in_tray() {
        assert_eq!(
            classify_exit_request(None),
            ExitRequestAction::PreventSystemExit
        );
    }

    #[test]
    fn restart_exit_code_defers_to_tauri_default_restart() {
        assert_eq!(
            classify_exit_request(Some(tauri::RESTART_EXIT_CODE)),
            ExitRequestAction::DeferToTauriRestart
        );
    }

    #[test]
    fn user_exit_codes_run_cleanup_then_exit() {
        assert_eq!(
            classify_exit_request(Some(0)),
            ExitRequestAction::CleanupAndExit
        );
        assert_eq!(
            classify_exit_request(Some(1)),
            ExitRequestAction::CleanupAndExit
        );
    }
}
