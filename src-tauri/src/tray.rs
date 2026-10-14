//! Navigation for the ten ModelDeck clients. Applying connections remains an
//! explicit operation in the main window.

use crate::{error::AppError, store::AppState};
use tauri::{
    menu::{Menu, MenuBuilder, MenuItem},
    Emitter, Manager,
};
use tauri_plugin_opener::OpenerExt;

pub const TRAY_ID: &str = "modeldeck";

fn client_label(client: &str) -> &str {
    match client {
        "claude" => "Claude Code",
        "claude-desktop" => "Claude Desktop",
        "codex" => "Codex CLI",
        "antigravity" => "Antigravity CLI",
        "cursor-agent" => "Cursor CLI",
        "cursor" => "Cursor GUI",
        "grokbuild" => "Grok",
        "muse" => "Muse Code",
        "opencode" => "OpenCode",
        "hermes" => "Hermes",
        other => other,
    }
}

pub fn create_tray_menu(
    app: &tauri::AppHandle,
    _state: &AppState,
) -> Result<Menu<tauri::Wry>, AppError> {
    let settings = crate::settings::get_settings();
    let locale = settings
        .language
        .clone()
        .or_else(sys_locale::get_locale)
        .unwrap_or_else(|| "en".into())
        .to_lowercase()
        .replace('_', "-");
    let language =
        if locale.starts_with("zh-tw") || locale.starts_with("zh-hk") || locale.contains("hant") {
            "zh-TW"
        } else if locale.starts_with("zh") {
            "zh"
        } else if locale.starts_with("ja") {
            "ja"
        } else {
            "en"
        };
    let (show, project, quit) = match language {
        "zh" => ("打开 ModelDeck", "项目主页", "退出"),
        "zh-TW" => ("開啟 ModelDeck", "專案首頁", "結束"),
        "ja" => ("ModelDeck を開く", "プロジェクト", "終了"),
        _ => ("Open ModelDeck", "Project", "Quit"),
    };
    let failure = |error: tauri::Error| AppError::Message(error.to_string());
    let show_item =
        MenuItem::with_id(app, "show_main", show, true, None::<&str>).map_err(failure)?;
    let project_item =
        MenuItem::with_id(app, "open_project", project, true, None::<&str>).map_err(failure)?;
    let quit_item = MenuItem::with_id(app, "quit", quit, true, None::<&str>).map_err(failure)?;
    let mut menu = MenuBuilder::new(app).item(&show_item).separator();
    let visible = settings.visible_apps.unwrap_or_default();
    for client in crate::deck::CLIENTS {
        let shown = match client.parse::<crate::AppType>() {
            Ok(app_type) => visible.is_visible(&app_type),
            Err(_) => visible.is_model_client_visible(client),
        };
        if shown {
            let item = MenuItem::with_id(
                app,
                format!("modeldeck_client_{client}"),
                client_label(client),
                true,
                None::<&str>,
            )
            .map_err(failure)?;
            menu = menu.item(&item);
        }
    }
    menu.separator()
        .item(&project_item)
        .item(&quit_item)
        .build()
        .map_err(failure)
}

pub fn refresh_tray_menu(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    match create_tray_menu(app, &state) {
        Ok(menu) => {
            if let Some(tray) = app.tray_by_id(TRAY_ID) {
                if let Err(error) = tray.set_menu(Some(menu)) {
                    log::warn!("Cannot refresh the ModelDeck tray: {error}");
                }
            }
        }
        Err(error) => log::warn!("Cannot build the ModelDeck tray: {error}"),
    }
}

pub fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        #[cfg(target_os = "windows")]
        let _ = window.set_skip_taskbar(false);
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        #[cfg(target_os = "linux")]
        crate::linux_fix::nudge_main_window(window);
        #[cfg(target_os = "macos")]
        apply_tray_policy(app, true);
    }
}

#[cfg(target_os = "macos")]
pub fn apply_tray_policy(app: &tauri::AppHandle, dock_visible: bool) {
    let policy = if dock_visible {
        tauri::ActivationPolicy::Regular
    } else {
        tauri::ActivationPolicy::Accessory
    };
    if let Err(error) = app.set_dock_visibility(dock_visible) {
        log::warn!("Set Dock visibility: {error}");
    }
    if let Err(error) = app.set_activation_policy(policy) {
        log::warn!("Set activation policy: {error}");
    }
}

pub fn handle_tray_menu_event(app: &tauri::AppHandle, event_id: &str) {
    if let Some(client) = event_id.strip_prefix("modeldeck_client_") {
        if crate::deck::CLIENTS.contains(&client) {
            show_main_window(app);
            let _ = app.emit(
                "modeldeck-client-selected",
                serde_json::json!({"client":client}),
            );
        }
        return;
    }
    match event_id {
        "show_main" => show_main_window(app),
        "open_project" => {
            let _ = app
                .opener()
                .open_url("https://github.com/Vita0818/ModelDeck", None::<String>);
        }
        "quit" => app.exit(0),
        _ => {}
    }
}
