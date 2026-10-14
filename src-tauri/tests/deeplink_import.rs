use modeldeck_lib::{import_provider_from_deeplink, parse_deeplink_url, AppState, Database};
use serde_json::Value;
use std::{fs, sync::Arc};
#[path = "support.rs"]
mod support;
use support::{ensure_test_home, reset_test_fs, test_mutex};

fn graph() -> Value {
    serde_json::from_slice(
        &fs::read(ensure_test_home().join(".cc-switch/model-deck.json")).unwrap(),
    )
    .unwrap()
}

#[test]
fn provider_links_import_resources_without_writing_native_configs() {
    let _guard = test_mutex().lock().unwrap();
    reset_test_fs();
    let home = ensure_test_home();
    let auth = home.join(".codex/auth.json");
    let config = home.join(".codex/config.toml");
    let claude = home.join(".claude/settings.json");
    fs::create_dir_all(auth.parent().unwrap()).unwrap();
    fs::create_dir_all(claude.parent().unwrap()).unwrap();
    fs::write(
        &auth,
        b"{\"auth_mode\":\"chatgpt\",\"tokens\":{\"access_token\":\"official-login\"}}",
    )
    .unwrap();
    fs::write(
        &config,
        b"model_provider = \"openai\"\nmodel = \"official\"\n",
    )
    .unwrap();
    fs::write(&claude, b"{\"permissions\":{\"allow\":[\"Read\"]}}").unwrap();
    let originals: Vec<_> = [&auth, &config, &claude]
        .iter()
        .map(|path| fs::read(path).unwrap())
        .collect();
    let db = Arc::new(Database::memory().unwrap());
    let state = AppState::new(db.clone());
    let link = "modeldeck://v1/import?resource=provider&app=codex&name=Imported%20Codex&endpoint=https%3A%2F%2Fmodels.example%2Fv1&apiKey=dedicated-key&model=vendor%2Fmodel&enabled=true";
    let id = import_provider_from_deeplink(&state, parse_deeplink_url(link).unwrap()).unwrap();
    let store = graph();
    assert_eq!(store["providers"][&id]["name"], "Imported Codex");
    assert_eq!(
        store["providers"][&id]["baseUrl"],
        "https://models.example/v1"
    );
    assert_eq!(store["providers"][&id]["apiKey"], "dedicated-key");
    assert!(store["models"]
        .as_object()
        .unwrap()
        .values()
        .any(|m| m["upstreamId"] == "vendor/model"));
    assert!(store["bindings"].get("codex").is_some());
    assert!(store["activeBindings"].as_object().unwrap().is_empty());
    assert!(db.get_all_providers("codex").unwrap().is_empty());
    for (path, original) in [&auth, &config, &claude].iter().zip(originals) {
        assert_eq!(fs::read(path).unwrap(), original);
    }
}

#[test]
fn model_less_links_preserve_every_endpoint_without_empty_catalogs() {
    let _guard = test_mutex().lock().unwrap();
    reset_test_fs();
    let state = AppState::new(Arc::new(Database::memory().unwrap()));
    let request = parse_deeplink_url("modeldeck://v1/import?resource=provider&app=claude&name=Two%20endpoints&endpoint=https%3A%2F%2Fone.example%2Fv1%2Chttps%3A%2F%2Ftwo.example%2Fv1&apiKey=dedicated-key").unwrap();
    import_provider_from_deeplink(&state, request).unwrap();
    let store = graph();
    let providers = store["providers"].as_object().unwrap();
    assert_eq!(providers.len(), 2);
    for url in ["https://one.example/v1", "https://two.example/v1"] {
        assert!(providers.values().any(|p| p["baseUrl"] == url));
    }
    assert!(store["endpoints"].as_object().unwrap().is_empty());
    assert!(store["models"].as_object().unwrap().is_empty());
}
