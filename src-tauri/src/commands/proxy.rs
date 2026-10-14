//! Shared model gateway controls. Native files are owned by deck apply/restore.

use crate::error::AppError;
use crate::proxy::types::{GlobalProxyConfig, ProxyServerInfo, ProxyStatus};
use crate::store::AppState;
use tauri::State;

#[tauri::command]
pub async fn start_proxy_server(state: State<'_, AppState>) -> Result<ProxyServerInfo, String> {
    state.proxy_service.start().await
}

#[tauri::command]
pub async fn stop_proxy_server(state: State<'_, AppState>) -> Result<(), String> {
    if crate::deck::needs_gateway() || crate::model_clients::needs_gateway() {
        return Err("Restore the active clients before stopping their shared gateway".into());
    }
    state.proxy_service.stop().await
}

#[tauri::command]
pub async fn get_proxy_status(state: State<'_, AppState>) -> Result<ProxyStatus, String> {
    state.proxy_service.get_status().await
}

#[tauri::command]
pub async fn get_global_proxy_config(
    state: State<'_, AppState>,
) -> Result<GlobalProxyConfig, String> {
    state
        .db
        .get_global_proxy_config()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_global_proxy_config(
    state: State<'_, AppState>,
    mut config: GlobalProxyConfig,
) -> Result<(), String> {
    config
        .listen_address
        .parse::<std::net::IpAddr>()
        .map_err(|_| "Enter an IPv4 or IPv6 gateway listen address")?;
    if config.listen_port == 0 {
        return Err("Choose a fixed gateway port between 1 and 65535".into());
    }
    let previous = state
        .db
        .get_global_proxy_config()
        .await
        .map_err(|e| e.to_string())?;
    let changed = previous.listen_address != config.listen_address
        || previous.listen_port != config.listen_port;
    if changed && (crate::deck::needs_gateway() || crate::model_clients::needs_gateway()) {
        return Err("Restore active clients before changing the gateway address or port".into());
    }
    let running = state.proxy_service.is_running().await;
    config.proxy_enabled = previous.proxy_enabled;
    if changed && running {
        state.proxy_service.stop_for_exit().await?;
    }
    state
        .db
        .update_global_proxy_config(config)
        .await
        .map_err(|e| e.to_string())?;
    if changed && running {
        if let Err(error) = state.proxy_service.start().await {
            let _ = state.db.update_global_proxy_config(previous).await;
            let _ = state.proxy_service.start().await;
            return Err(error);
        }
    }
    Ok(())
}

async fn get_default_cost_multiplier_internal(
    state: &AppState,
    app_type: &str,
) -> Result<String, AppError> {
    let db = &state.db;
    db.get_default_cost_multiplier(app_type).await
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub async fn get_default_cost_multiplier_test_hook(
    state: &AppState,
    app_type: &str,
) -> Result<String, AppError> {
    get_default_cost_multiplier_internal(state, app_type).await
}

/// 获取默认成本倍率
#[tauri::command]
pub async fn get_default_cost_multiplier(
    state: tauri::State<'_, AppState>,
    app_type: String,
) -> Result<String, String> {
    get_default_cost_multiplier_internal(&state, &app_type)
        .await
        .map_err(|e| e.to_string())
}

async fn set_default_cost_multiplier_internal(
    state: &AppState,
    app_type: &str,
    value: &str,
) -> Result<(), AppError> {
    let db = &state.db;
    db.set_default_cost_multiplier(app_type, value).await
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub async fn set_default_cost_multiplier_test_hook(
    state: &AppState,
    app_type: &str,
    value: &str,
) -> Result<(), AppError> {
    set_default_cost_multiplier_internal(state, app_type, value).await
}

/// 设置默认成本倍率
#[tauri::command]
pub async fn set_default_cost_multiplier(
    state: tauri::State<'_, AppState>,
    app_type: String,
    value: String,
) -> Result<(), String> {
    set_default_cost_multiplier_internal(&state, &app_type, &value)
        .await
        .map_err(|e| e.to_string())
}

async fn get_pricing_model_source_internal(
    state: &AppState,
    app_type: &str,
) -> Result<String, AppError> {
    let db = &state.db;
    db.get_pricing_model_source(app_type).await
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub async fn get_pricing_model_source_test_hook(
    state: &AppState,
    app_type: &str,
) -> Result<String, AppError> {
    get_pricing_model_source_internal(state, app_type).await
}

/// 获取计费模式来源
#[tauri::command]
pub async fn get_pricing_model_source(
    state: tauri::State<'_, AppState>,
    app_type: String,
) -> Result<String, String> {
    get_pricing_model_source_internal(&state, &app_type)
        .await
        .map_err(|e| e.to_string())
}

async fn set_pricing_model_source_internal(
    state: &AppState,
    app_type: &str,
    value: &str,
) -> Result<(), AppError> {
    let db = &state.db;
    db.set_pricing_model_source(app_type, value).await
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub async fn set_pricing_model_source_test_hook(
    state: &AppState,
    app_type: &str,
    value: &str,
) -> Result<(), AppError> {
    set_pricing_model_source_internal(state, app_type, value).await
}

/// 设置计费模式来源
#[tauri::command]
pub async fn set_pricing_model_source(
    state: tauri::State<'_, AppState>,
    app_type: String,
    value: String,
) -> Result<(), String> {
    set_pricing_model_source_internal(&state, &app_type, &value)
        .await
        .map_err(|e| e.to_string())
}
