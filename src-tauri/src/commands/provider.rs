//! Read-only names for historical usage records. Model resources are edited
//! through the resource graph rather than the inherited provider controller.

use crate::{provider::Provider, store::AppState};
use indexmap::IndexMap;
use tauri::State;

#[tauri::command]
pub fn get_providers(
    state: State<'_, AppState>,
    app: String,
) -> Result<IndexMap<String, Provider>, String> {
    let mut providers = state
        .db
        .get_all_providers(&app)
        .map_err(|e| e.to_string())?;
    for provider in providers.values_mut() {
        provider.settings_config = serde_json::json!({});
        provider.meta = None;
    }
    Ok(providers)
}

#[tauri::command]
pub fn get_current_provider(state: State<'_, AppState>, app: String) -> Result<String, String> {
    state
        .db
        .get_current_provider(&app)
        .map(|id| id.unwrap_or_default())
        .map_err(|e| e.to_string())
}
