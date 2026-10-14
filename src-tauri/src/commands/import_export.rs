//! Local database archives. Restoring usage records never projects old provider
//! settings into native clients or replaces the model resource graph.

use crate::{
    database::{backup::BackupEntry, Database},
    error::AppError,
    store::AppState,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

static RESTORE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn restore_database(
    db: std::sync::Arc<Database>,
    operation: impl FnOnce(&Database) -> Result<String, AppError> + Send + 'static,
) -> Result<String, String> {
    let _restore = RESTORE_LOCK.lock().await;
    let _usage = crate::services::session_usage::session_sync_mutex()
        .lock()
        .await;
    let gateway = db
        .get_global_proxy_config()
        .await
        .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let result = operation(&db)?;
        // A database archive may carry a different port. Keep the live gateway
        // and the active harnesses bound to the address selected in this app.
        futures::executor::block_on(db.update_global_proxy_config(gateway))?;
        if let Err(error) = crate::services::model_pricing::sync_local_model_pricing(&db) {
            log::warn!("Database restored; pricing cache refresh failed: {error}");
        }
        Ok::<_, AppError>(result)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_config_from_file(
    #[allow(non_snake_case)] filePath: String,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    let backup = restore_database(state.db.clone(), move |db| {
        db.import_sql(&PathBuf::from(filePath))
    })
    .await?;
    Ok(json!({"success":true,"message":"Database records imported","backupId":backup}))
}

#[tauri::command]
pub async fn restore_db_backup(
    state: State<'_, AppState>,
    filename: String,
) -> Result<String, String> {
    restore_database(state.db.clone(), move |db| {
        db.restore_from_backup(&filename)
    })
    .await
}

#[tauri::command]
pub async fn export_config_to_file(
    #[allow(non_snake_case)] filePath: String,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let target_path = PathBuf::from(&filePath);
        db.export_sql(&target_path)?;
        Ok::<_, AppError>(json!({
            "success": true,
            "message": "SQL exported successfully",
            "filePath": filePath
        }))
    })
    .await
    .map_err(|e| format!("导出配置失败: {e}"))?
    .map_err(|e: AppError| e.to_string())
}

#[tauri::command]
pub async fn save_file_dialog<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    #[allow(non_snake_case)] defaultName: String,
) -> Result<Option<String>, String> {
    let dialog = app.dialog();
    let result = dialog
        .file()
        .add_filter("SQL", &["sql"])
        .set_file_name(&defaultName)
        .blocking_save_file();

    Ok(result.map(|p| p.to_string()))
}

#[tauri::command]
pub async fn open_file_dialog<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, String> {
    let dialog = app.dialog();
    let result = dialog
        .file()
        .add_filter("SQL", &["sql"])
        .blocking_pick_file();

    Ok(result.map(|p| p.to_string()))
}

#[tauri::command]
pub async fn create_db_backup(state: State<'_, AppState>) -> Result<String, String> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || match db.backup_database_file()? {
        Some(path) => Ok(path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default()),
        None => Err(AppError::Config(
            "Database file not found, backup skipped".to_string(),
        )),
    })
    .await
    .map_err(|e| format!("Backup failed: {e}"))?
    .map_err(|e: AppError| e.to_string())
}

#[tauri::command]
pub fn list_db_backups() -> Result<Vec<BackupEntry>, String> {
    Database::list_backups().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_db_backup(
    #[allow(non_snake_case)] oldFilename: String,
    #[allow(non_snake_case)] newName: String,
) -> Result<String, String> {
    Database::rename_backup(&oldFilename, &newName).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_db_backup(filename: String) -> Result<(), String> {
    Database::delete_backup(&filename).map_err(|e| e.to_string())
}
