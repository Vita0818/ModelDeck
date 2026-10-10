use super::*;
use serde_json::json;

#[test]
#[ignore = "Requires a private prepared resource graph; validation only"]
fn private_prepared_resource_graph_is_valid() {
    let path = std::env::var("MODELDECK_PREPARED_GRAPH").expect("private graph path");
    let store: DeckStore = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    validate(&store).expect("prepared resource graph is invalid");
    eprintln!(
        "Prepared graph validated: {} providers, {} models, {} endpoints, {} selections",
        store.providers.len(),
        store.models.len(),
        store.endpoints.len(),
        store.bindings.len()
    );
    for client in ["codex", "claude", "claude-desktop", "antigravity", "muse"] {
        if let Some(binding) = store.bindings.get(client) {
            let profile =
                project_profile(&store, client, binding, "http://127.0.0.1:15721").unwrap();
            let endpoint = &store.endpoints[&binding.endpoint_id];
            assert_eq!(profile.effort, endpoint.routes[&binding.model].effort);
            for model in &profile.models {
                let route = &endpoint.routes[&model.id];
                let resource = &store.models[&route.model_id];
                assert_eq!(
                    (model.context, model.output),
                    (resource.context, resource.output)
                );
                assert_eq!(model.reasoning, resource.reasoning);
                assert_eq!(model.default_effort, route.effort);
            }
            eprintln!(
                "{client}: {} native models, default tier {}",
                profile.models.len(),
                profile.effort
            );
        }
    }
}

pub(super) fn fixture() -> DeckStore {
    serde_json::from_value(json!({"schemaVersion":1,"revision":0,
        "providers":{"upstream":{"id":"upstream","name":"Provider","baseUrl":"https://example.test/v1","protocol":"openai-chat","apiKey":"upstream-only"}},
        "models":{"resource":{"id":"resource","providerId":"upstream","upstreamId":"real-model","name":"Private model","context":200000,"output":8192,"reasoning":["high","max"],"tools":true,"images":false}},
        "endpoints":{"entry":{"id":"entry","name":"Entry","protocols":["openai-chat","openai-responses","anthropic","gemini"],"token":"gateway-only","routes":{"coding":{"modelId":"resource","effort":"high"},"claude-sonnet-5":{"modelId":"resource","effort":"max"}}}},
        "bindings":{},"activeBindings":{},"migration":{"importedProviders":0,"importedModels":0,"retained":[]}})).unwrap()
}

#[test]
fn all_ten_harnesses_reference_the_same_resource_graph() {
    let mut store = fixture();
    store.endpoints.get_mut("entry").unwrap().public_url =
        "https://gateway.example/deck/entry".into();
    for client in CLIENTS {
        let alias = if client == "claude-desktop" {
            "claude-sonnet-5"
        } else {
            "coding"
        };
        let binding = HarnessBinding {
            endpoint_id: "entry".into(),
            model: alias.into(),
            ..Default::default()
        };
        validate_binding(&store, client, &binding, true).unwrap();
        store.bindings.insert(client.into(), binding.clone());
        let projection =
            project_profile(&store, client, &binding, "http://127.0.0.1:15721").unwrap();
        assert_eq!(projection.api_key, "gateway-only");
        assert!(!serde_json::to_string(&projection)
            .unwrap()
            .contains("upstream-only"));
        let saved = serde_json::to_value(binding).unwrap();
        assert!(saved.get("models").is_none());
        assert!(saved.get("apiKey").is_none());
        assert!(saved.get("effort").is_none());
    }
    assert_eq!(store.providers.len(), 1);
    assert_eq!(store.models.len(), 1);
    validate(&store).unwrap();
}

#[test]
fn shared_model_changes_reach_every_projection() {
    let mut store = fixture();
    let binding = HarnessBinding {
        endpoint_id: "entry".into(),
        model: "coding".into(),
        ..Default::default()
    };
    store.models.get_mut("resource").unwrap().context = 1_000_000;
    for client in [
        "codex",
        "claude",
        "muse",
        "antigravity",
        "opencode",
        "hermes",
    ] {
        assert_eq!(
            project_profile(&store, client, &binding, "http://localhost:15721")
                .unwrap()
                .models[0]
                .context,
            1_000_000
        );
    }
}

#[test]
fn deleting_referenced_resources_or_tiers_is_rejected() {
    let mut store = fixture();
    store.models.remove("resource");
    assert!(validate(&store).is_err());
    let mut store = fixture();
    store.providers.remove("upstream");
    assert!(validate(&store).is_err());
    let mut store = fixture();
    store.models.get_mut("resource").unwrap().reasoning.clear();
    assert!(validate(&store).is_err());
    let mut store = fixture();
    store.active_bindings.insert(
        "codex".into(),
        HarnessBinding {
            endpoint_id: "entry".into(),
            model: "coding".into(),
            ..Default::default()
        },
    );
    store.endpoints.remove("entry");
    assert!(validate(&store).is_err());
}

#[test]
fn compatibility_excludes_unimplemented_conversions_and_native_constraints() {
    let mut store = fixture();
    store.providers.get_mut("upstream").unwrap().protocol = Protocol::Anthropic;
    assert_eq!(
        compatible_models(&store, "codex", &store.endpoints["entry"]).len(),
        2
    );
    assert!(compatible_models(&store, "antigravity", &store.endpoints["entry"]).is_empty());
    assert!(compatible_models(&store, "cursor", &store.endpoints["entry"]).is_empty());
    store.providers.get_mut("upstream").unwrap().protocol = Protocol::OpenaiChat;
    assert_eq!(
        compatible_models(&store, "claude-desktop", &store.endpoints["entry"]),
        vec!["claude-sonnet-5"]
    );
    let binding = HarnessBinding {
        endpoint_id: "entry".into(),
        model: "coding".into(),
        ..Default::default()
    };
    assert!(validate_binding(&store, "cursor", &binding, false).is_ok());
    assert!(validate_binding(&store, "cursor", &binding, true).is_err());
    let unverified_local = HarnessBinding {
        executable: "/does-not-exist/cursor-agent-local".into(),
        ..binding.clone()
    };
    assert!(validate_binding(&store, "cursor-agent", &unverified_local, true).is_err());
    store.endpoints.get_mut("entry").unwrap().public_url =
        "https://[::ffff:127.0.0.1]/deck/entry".into();
    assert!(validate_binding(&store, "cursor", &binding, true).is_err());
}

#[test]
fn migration_deduplicates_shared_credentials_and_preserves_distinct_accounts() {
    let profile:ClientProfile=serde_json::from_value(json!({"connectionName":"Shared","endpoint":"https://example.test/v1","protocol":"openai-chat","apiKey":"same-key","models":[{"id":"model","name":"Model","context":200000,"output":8192,"reasoning":["high"]}],"defaultModel":"model","effort":"high"})).unwrap();
    let store =
        migration::import_profiles(&[("muse", profile.clone()), ("antigravity", profile.clone())]);
    assert_eq!(store.providers.len(), 1);
    assert_eq!(store.models.len(), 1);
    assert_eq!(store.endpoints.len(), 1);
    assert_eq!(store.bindings.len(), 2);
    assert!(store.active_bindings.is_empty());
    validate(&store).unwrap();
    let mut other = profile.clone();
    other.api_key = "another-account".into();
    let store = migration::import_profiles(&[("muse", profile), ("antigravity", other)]);
    assert_eq!(store.providers.len(), 2);
    assert_eq!(store.models.len(), 2);
}

#[test]
fn endpoint_cannot_advertise_only_incompatible_protocols() {
    let mut store = fixture();
    store.providers.get_mut("upstream").unwrap().protocol = Protocol::Gemini;
    store.endpoints.get_mut("entry").unwrap().protocols = vec![Protocol::OpenaiChat];
    assert!(validate(&store).is_err());
}

#[test]
fn free_tier_model_names_are_valid_without_allowing_url_delimiters() {
    let mut store = fixture();
    let route = store
        .endpoints
        .get_mut("entry")
        .unwrap()
        .routes
        .remove("coding")
        .unwrap();
    store
        .endpoints
        .get_mut("entry")
        .unwrap()
        .routes
        .insert("thinkingmachines/inkling:free".into(), route.clone());
    validate(&store).expect("provider-qualified free-tier names must remain usable");
    for invalid in [
        "model?query",
        "model#fragment",
        "model\nheader",
        "model[tier]",
    ] {
        let mut invalid_store = store.clone();
        invalid_store
            .endpoints
            .get_mut("entry")
            .unwrap()
            .routes
            .insert(invalid.into(), route.clone());
        assert!(validate(&invalid_store).is_err());
    }
}
