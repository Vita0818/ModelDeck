use tauri::AppHandle;

fn merge_settings_for_save(
    mut incoming: crate::settings::AppSettings,
    existing: &crate::settings::AppSettings,
) -> crate::settings::AppSettings {
    // Retired settings remain passive data so saving the ModelDeck preferences
    // cannot erase old credentials or reactivate native account/history tools.
    incoming.webdav_sync = existing.webdav_sync.clone();
    incoming.s3_sync = existing.s3_sync.clone();
    incoming.local_migrations = existing.local_migrations.clone();
    incoming.unify_codex_session_history = existing.unify_codex_session_history;
    incoming.unify_codex_migrate_existing = existing.unify_codex_migrate_existing;
    incoming
}

#[tauri::command]
pub async fn get_settings() -> Result<crate::settings::AppSettings, String> {
    Ok(crate::settings::get_settings_for_frontend())
}

#[tauri::command]
pub async fn save_settings(settings: crate::settings::AppSettings) -> Result<bool, String> {
    let existing = crate::settings::get_settings();
    for (client, before, after) in [
        (
            "claude",
            &existing.claude_config_dir,
            &settings.claude_config_dir,
        ),
        (
            "codex",
            &existing.codex_config_dir,
            &settings.codex_config_dir,
        ),
        (
            "grokbuild",
            &existing.grok_config_dir,
            &settings.grok_config_dir,
        ),
        (
            "opencode",
            &existing.opencode_config_dir,
            &settings.opencode_config_dir,
        ),
        (
            "hermes",
            &existing.hermes_config_dir,
            &settings.hermes_config_dir,
        ),
    ] {
        if before != after && crate::deck::binding_active(client)? {
            return Err(format!(
                "Restore {client}'s native configuration before changing its directory"
            ));
        }
    }
    crate::settings::update_settings(merge_settings_for_save(settings, &existing))
        .map_err(|error| error.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn set_app_config_dir_override(
    app: AppHandle,
    path: Option<String>,
) -> Result<bool, String> {
    let previous = crate::app_store::get_app_config_dir_override();
    let changed = previous.as_ref().map(|p| p.to_string_lossy().to_string()) != path;
    if changed && (crate::deck::needs_gateway() || crate::model_clients::needs_gateway()) {
        return Err(
            "Restore active client configurations before changing the ModelDeck data directory"
                .into(),
        );
    }
    crate::app_store::set_app_config_dir_to_store(&app, path.as_deref())?;
    Ok(true)
}

#[tauri::command]
pub async fn restart_app(app: AppHandle) -> Result<bool, String> {
    crate::save_window_state_before_exit(&app);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        crate::cleanup_before_exit(&app).await;
        app.restart();
    });
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_preferences_preserve_retired_credentials_and_migration_records() {
        let existing = crate::settings::AppSettings {
            webdav_sync: Some(crate::settings::WebDavSyncSettings {
                password: "existing-secret".into(),
                ..Default::default()
            }),
            unify_codex_session_history: true,
            ..Default::default()
        };
        let incoming = crate::settings::AppSettings {
            language: Some("en".into()),
            ..Default::default()
        };
        let merged = merge_settings_for_save(incoming, &existing);
        assert_eq!(merged.language.as_deref(), Some("en"));
        assert_eq!(merged.webdav_sync.unwrap().password, "existing-secret");
        assert!(merged.unify_codex_session_history);
    }
}

#[tauri::command]
pub async fn get_app_config_dir_override(app: AppHandle) -> Result<Option<String>, String> {
    Ok(crate::app_store::refresh_app_config_dir_override(&app)
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn set_auto_launch(enabled: bool) -> Result<bool, String> {
    if enabled {
        crate::auto_launch::enable_auto_launch().map_err(|e| format!("启用开机自启失败: {e}"))?;
    } else {
        crate::auto_launch::disable_auto_launch().map_err(|e| format!("禁用开机自启失败: {e}"))?;
    }
    Ok(true)
}

#[tauri::command]
pub async fn get_auto_launch_status() -> Result<bool, String> {
    crate::auto_launch::is_auto_launch_enabled().map_err(|e| format!("获取开机自启状态失败: {e}"))
}

#[tauri::command]
pub async fn get_log_config(
    state: tauri::State<'_, crate::AppState>,
) -> Result<crate::proxy::types::LogConfig, String> {
    state.db.get_log_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_log_config(
    state: tauri::State<'_, crate::AppState>,
    config: crate::proxy::types::LogConfig,
) -> Result<bool, String> {
    state
        .db
        .set_log_config(&config)
        .map_err(|e| e.to_string())?;
    log::set_max_level(config.to_level_filter());
    log::info!(
        "日志配置已更新: enabled={}, level={}",
        config.enabled,
        config.level
    );
    Ok(true)
}
