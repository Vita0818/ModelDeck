use modeldeck_lib::{AppError, AppType, MultiAppConfig, Provider};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
#[path = "support.rs"]
mod support;
use support::{
    create_test_state, create_test_state_with_config, ensure_test_home, reset_test_fs, test_mutex,
};

#[test]
fn export_sql_writes_to_target_path() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    // Create test state with some data
    let mut config = MultiAppConfig::default();
    {
        let manager = config
            .get_manager_mut(&AppType::Claude)
            .expect("claude manager");
        manager.current = "test-provider".to_string();
        manager.providers.insert(
            "test-provider".to_string(),
            Provider::with_id(
                "test-provider".to_string(),
                "Test Provider".to_string(),
                json!({"env": {"ANTHROPIC_API_KEY": "test-key"}}),
                None,
            ),
        );
    }

    let state = create_test_state_with_config(&config).expect("create test state");

    // Export to SQL file
    let export_path = home.join("test-export.sql");
    state
        .db
        .export_sql(&export_path)
        .expect("export should succeed");

    // Verify file exists and contains data
    assert!(export_path.exists(), "export file should exist");
    let content = fs::read_to_string(&export_path).expect("read exported file");
    assert!(
        content.contains("INSERT INTO") && content.contains("providers"),
        "exported SQL should contain INSERT statements for providers"
    );
    assert!(
        content.contains("test-provider"),
        "exported SQL should contain test data"
    );
}

#[test]
fn export_sql_returns_error_for_invalid_path() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let state = create_test_state().expect("create test state");

    // Try to export to an invalid path (nonexistent parent or invalid name on Windows)
    let invalid_parent = if cfg!(windows) {
        std::env::temp_dir().join("cc-switch-test-invalid<>dir")
    } else {
        PathBuf::from("/nonexistent/directory")
    };
    let invalid_path = invalid_parent.join("export.sql");
    let err = state
        .db
        .export_sql(&invalid_path)
        .expect_err("export to invalid path should fail");
    let invalid_prefix = invalid_parent.to_string_lossy();

    // The error can be either IoContext or Io depending on where it fails
    match err {
        AppError::IoContext { context, .. } => {
            assert!(
                context.contains("原子写入失败") || context.contains("写入失败"),
                "expected IO error message about atomic write failure, got: {context}"
            );
        }
        AppError::Io { path, .. } => {
            assert!(
                path.starts_with(invalid_prefix.as_ref()),
                "expected error for {invalid_parent:?}, got: {path:?}"
            );
        }
        other => panic!("expected IoContext or Io error, got {other:?}"),
    }
}

#[test]
fn import_sql_rejects_non_cc_switch_backup() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let state = create_test_state().expect("create test state");

    let import_path = home.join("not-cc-switch.sql");
    fs::write(&import_path, "CREATE TABLE x (id INTEGER);").expect("write import sql");

    let err = state
        .db
        .import_sql(&import_path)
        .expect_err("non-cc-switch sql should be rejected");

    match err {
        AppError::Localized { key, .. } => {
            assert_eq!(key, "backup.sql.invalid_format");
        }
        other => panic!("expected Localized error, got {other:?}"),
    }
}

#[test]
fn import_sql_accepts_cc_switch_exported_backup() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    // Create a database with some data and export it.
    let mut config = MultiAppConfig::default();
    {
        let manager = config
            .get_manager_mut(&AppType::Claude)
            .expect("claude manager");
        manager.current = "test-provider".to_string();
        manager.providers.insert(
            "test-provider".to_string(),
            Provider::with_id(
                "test-provider".to_string(),
                "Test Provider".to_string(),
                json!({"env": {"ANTHROPIC_API_KEY": "test-key"}}),
                None,
            ),
        );
    }

    let state = create_test_state_with_config(&config).expect("create test state");
    let export_path = home.join("cc-switch-export.sql");
    state
        .db
        .export_sql(&export_path)
        .expect("export should succeed");

    // Reset database, then import into a fresh one.
    reset_test_fs();
    let state = create_test_state().expect("create test state");
    state
        .db
        .import_sql(&export_path)
        .expect("import should succeed");

    let providers = state
        .db
        .get_all_providers(AppType::Claude.as_str())
        .expect("load providers");
    assert!(
        providers.contains_key("test-provider"),
        "imported providers should contain test-provider"
    );
}
