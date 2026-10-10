use tauri::State;

use crate::hermes_config;
use crate::store::AppState;

// ============================================================================
// Hermes Provider Commands
// ============================================================================

/// Import providers from Hermes live config to database.
///
/// Hermes uses additive mode — users may already have providers
/// configured in config.yaml.
#[tauri::command]
pub fn import_hermes_providers_from_live(state: State<'_, AppState>) -> Result<usize, String> {
    crate::services::provider::import_hermes_providers_from_live(state.inner())
        .map_err(|e| e.to_string())
}

/// Get provider names in the Hermes live config.
#[tauri::command]
pub fn get_hermes_live_provider_ids() -> Result<Vec<String>, String> {
    hermes_config::get_providers()
        .map(|providers| providers.keys().cloned().collect())
        .map_err(|e| e.to_string())
}

/// Get a single Hermes provider fragment from live config.
#[tauri::command]
pub fn get_hermes_live_provider(
    #[allow(non_snake_case)] providerId: String,
) -> Result<Option<serde_json::Value>, String> {
    hermes_config::get_provider(&providerId).map_err(|e| e.to_string())
}

// ============================================================================
// Model Configuration Commands
// ============================================================================

/// Get Hermes model config (model section of config.yaml). Read-only — writes
/// happen implicitly through `apply_switch_defaults` when switching providers.
#[tauri::command]
pub fn get_hermes_model_config() -> Result<Option<hermes_config::HermesModelConfig>, String> {
    hermes_config::get_model_config().map_err(|e| e.to_string())
}
