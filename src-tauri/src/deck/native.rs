use super::*;
use crate::{
    config::{get_home_dir, read_json_file},
    model_clients::{native as adapters, ModelClient},
    provider::{ClaudeDesktopModelRoute, Provider, ProviderMeta},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, path::Path};

#[derive(Clone, Serialize, Deserialize)]
struct Snapshot {
    path: PathBuf,
    bytes: Option<Vec<u8>>,
    mode: Option<u32>,
}
#[derive(Serialize, Deserialize)]
struct Backup {
    original: Vec<Snapshot>,
    applied: Vec<Snapshot>,
}

fn backup_path(client: &str) -> Result<PathBuf, String> {
    if !CLIENTS.contains(&client) {
        return Err("Unknown harness".into());
    }
    Ok(get_app_config_dir()
        .join("deck-backups")
        .join(format!("{client}.json")))
}

fn paths(client: &str) -> Result<Vec<PathBuf>, String> {
    let home = get_home_dir();
    let mut paths = match client {
        "claude" => vec![crate::config::get_claude_settings_path()],
        "codex" => vec![
            crate::codex_config::get_codex_config_path(),
            crate::codex_config::get_codex_model_catalog_path(),
        ],
        "grokbuild" => vec![crate::grok_config::get_grok_config_path()],
        "opencode" => vec![crate::opencode_config::get_opencode_config_path()],
        "hermes" => vec![crate::hermes_config::get_hermes_config_path()],
        "claude-desktop" => {
            crate::claude_desktop_config::managed_file_paths().map_err(|e| e.to_string())?
        }
        "antigravity" => vec![home.join(".gemini/antigravity-cli/settings.json")],
        "muse" => vec![home.join(".config/muse/settings.json")],
        "cursor-agent" => vec![],
        "cursor" => {
            #[cfg(target_os = "macos")]
            let dir = home.join("Library/Application Support");
            #[cfg(not(target_os = "macos"))]
            let dir = dirs::config_dir().unwrap_or(home);
            let db = dir.join("Cursor/User/globalStorage/state.vscdb");
            vec![
                db.clone(),
                PathBuf::from(format!("{}-wal", db.display())),
                PathBuf::from(format!("{}-shm", db.display())),
            ]
        }
        _ => return Err("Unknown harness".into()),
    };
    if matches!(client, "claude" | "antigravity" | "muse" | "cursor-agent") {
        paths.push(get_app_config_dir().join("bin").join(client));
    }
    Ok(paths)
}

fn capture(paths: &[PathBuf]) -> Result<Vec<Snapshot>, String> {
    paths
        .iter()
        .map(|path| match fs::read(path) {
            Ok(bytes) => {
                #[cfg(unix)]
                let mode = {
                    use std::os::unix::fs::PermissionsExt;
                    Some(
                        fs::metadata(path)
                            .map_err(|e| e.to_string())?
                            .permissions()
                            .mode(),
                    )
                };
                #[cfg(not(unix))]
                let mode = None;
                Ok(Snapshot {
                    path: path.clone(),
                    bytes: Some(bytes),
                    mode,
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Snapshot {
                path: path.clone(),
                bytes: None,
                mode: None,
            }),
            Err(e) => Err(format!("Cannot back up {}: {e}", path.display())),
        })
        .collect()
}

fn restore_files(snapshots: &[Snapshot]) -> Result<(), String> {
    for snapshot in snapshots {
        if let Some(bytes) = &snapshot.bytes {
            atomic_write_private(&snapshot.path, bytes).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            if let Some(mode) = snapshot.mode {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&snapshot.path, fs::Permissions::from_mode(mode))
                    .map_err(|e| e.to_string())?;
            }
        } else if snapshot.path.exists() {
            fs::remove_file(&snapshot.path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn read_backup(client: &str, path: &Path) -> Result<Option<Backup>, String> {
    match fs::read(path) {
        Ok(bytes) => {
            let mut backup: Backup = serde_json::from_slice(&bytes)
                .map_err(|_| "Invalid native configuration backup".to_string())?;
            if client == "codex" {
                // Earlier snapshots included official login files. They are
                // exempt even when reading an old backup or reapplying a route.
                let auth = crate::codex_config::get_codex_auth_path();
                let marker = crate::codex_config::get_codex_managed_oauth_live_auth_marker_path();
                let owned = |snapshot: &Snapshot| {
                    snapshot.path.file_name() != auth.file_name()
                        && snapshot.path.file_name() != marker.file_name()
                };
                backup.original.retain(owned);
                backup.applied.retain(owned);
            }
            Ok(Some(backup))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn json_or_empty(path: &Path) -> Result<Value, String> {
    if path.exists() {
        read_json_file(path).map_err(|e| e.to_string())
    } else {
        Ok(json!({}))
    }
}

pub(super) fn generated_provider(client: &str, p: &ClientProfile) -> Result<Provider, String> {
    let base = format!("{}/v1", p.endpoint.trim_end_matches('/'));
    let config = match client {
        "claude" | "claude-desktop" => {
            let mut value = if client == "claude" {
                json_or_empty(&crate::config::get_claude_settings_path())?
            } else {
                json!({})
            };
            if !value["env"].is_object() {
                value["env"] = json!({});
            }
            value["env"]["ANTHROPIC_BASE_URL"] = json!(p.endpoint);
            value["env"]["ANTHROPIC_AUTH_TOKEN"] = json!(p.local_token);
            value["env"]
                .as_object_mut()
                .unwrap()
                .remove("ANTHROPIC_API_KEY");
            for field in [
                "ANTHROPIC_MODEL",
                "ANTHROPIC_DEFAULT_SONNET_MODEL",
                "ANTHROPIC_DEFAULT_OPUS_MODEL",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL",
                "ANTHROPIC_DEFAULT_FABLE_MODEL",
            ] {
                let role = field
                    .strip_prefix("ANTHROPIC_DEFAULT_")
                    .and_then(|s| s.strip_suffix("_MODEL"))
                    .map(str::to_ascii_lowercase);
                value["env"][field] = json!(if field == "ANTHROPIC_DEFAULT_HAIKU_MODEL"
                    && !p.utility_model.is_empty()
                {
                    &p.utility_model
                } else {
                    role.as_deref()
                        .and_then(|role| {
                            p.models
                                .iter()
                                .filter(|m| {
                                    m.id.contains(&format!("-{role}-")) && !m.id.contains(':')
                                })
                                .max_by_key(|m| {
                                    m.id.split(['-', '.', '/'])
                                        .filter_map(|s| s.parse::<u64>().ok())
                                        .collect::<Vec<_>>()
                                })
                                .map(|m| &m.id)
                        })
                        .unwrap_or(&p.default_model)
                });
            }
            if client == "claude" {
                value["model"] = json!(p.default_model);
                value["modelPicker"] = json!({
                    "replaceBuiltInOptions": true,
                    "options": p.models.iter().map(|m| {
                        let mut row = json!({"model":m.id,"label":m.name});
                        if let Some(id) = m.id.strip_prefix("anthropic/claude-") {
                            row["behavesAs"] = json!(format!("claude-{}", id.split(':').next().unwrap_or(id).replace('.', "-")));
                        }
                        row
                    }).collect::<Vec<_>>()
                });
            }
            value
        }
        "codex" => {
            let mut root = fs::read_to_string(crate::codex_config::get_codex_config_path())
                .ok()
                .filter(|text| !text.trim().is_empty())
                .map(|text| {
                    text.parse::<toml::Value>()
                        .map_err(|_| "Invalid existing Codex config".to_string())
                })
                .transpose()?
                .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
            let table = root.as_table_mut().ok_or("Invalid Codex config root")?;
            table.insert(
                "model_provider".into(),
                toml::Value::String("modeldeck".into()),
            );
            table.insert("model".into(), toml::Value::String(p.default_model.clone()));
            // Each generated catalog row carries its own context window. A
            // previous global override would incorrectly apply it to all rows.
            table.remove("model_context_window");
            if p.effort.is_empty() {
                table.remove("model_reasoning_effort");
            } else {
                table.insert(
                    "model_reasoning_effort".into(),
                    toml::Value::String(p.effort.clone()),
                );
            }
            let providers = table
                .entry("model_providers")
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
            providers.as_table_mut().ok_or("Invalid existing Codex providers")?.insert("modeldeck".into(),
                toml::Value::try_from(json!({"name":"ModelDeck","base_url":base,"wire_api":"responses","requires_openai_auth":true})).map_err(|e| e.to_string())?);
            let text = toml::to_string_pretty(&root).map_err(|e| e.to_string())?;
            json!({"auth":{"OPENAI_API_KEY":p.local_token},"config":text,
                "modelCatalog":{"models":p.models.iter().map(|m|json!({"model":m.id,"displayName":m.name,"contextWindow":m.context,"reasoningLevels":if m.reasoning.is_empty(){vec!["none".to_string()]}else{m.reasoning.clone()},"defaultReasoningLevel":if m.reasoning.is_empty(){"none"}else if m.default_effort.is_empty(){reasoning::preferred(&m.reasoning)}else{&m.default_effort},"supportsSearchTool":false})).collect::<Vec<_>>()}})
        }
        "grokbuild" => {
            let mut root = fs::read_to_string(crate::grok_config::get_grok_config_path())
                .ok()
                .filter(|s| !s.trim().is_empty())
                .map(|s| {
                    s.parse::<toml::Value>()
                        .map_err(|_| "Invalid existing Grok config".to_string())
                })
                .transpose()?
                .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
            let mut entries = toml::map::Map::new();
            for m in &p.models {
                entries.insert(m.id.clone(),toml::Value::try_from(json!({"model":m.id,"name":m.name,"base_url":base,"api_key":p.local_token,"api_backend":"responses","context_window":m.context})).map_err(|e|e.to_string())?);
            }
            let table = root.as_table_mut().ok_or("Invalid Grok config root")?;
            table.insert(
                "models".into(),
                toml::Value::try_from(json!({"default":p.default_model}))
                    .map_err(|e| e.to_string())?,
            );
            table.insert("model".into(), toml::Value::Table(entries));
            json!({"config":toml::to_string_pretty(&root).map_err(|e|e.to_string())?})
        }
        "opencode" => {
            let npm = match p.protocol.as_str() {
                "anthropic" => "@ai-sdk/anthropic",
                "openai-responses" => "@ai-sdk/openai",
                _ => "@ai-sdk/openai-compatible",
            };
            json!({"npm":npm,"name":"ModelDeck","options":{"baseURL":base,"apiKey":p.local_token},"models":p.models.iter().map(|m|(m.id.clone(),json!({"name":m.name,"limit":{"context":m.context,"output":m.output},"tool_call":true}))).collect::<serde_json::Map<_,_>>()})
        }
        "hermes" => {
            let mode = match p.protocol.as_str() {
                "anthropic" => "anthropic_messages",
                "openai-responses" => "codex_responses",
                _ => "chat_completions",
            };
            json!({"name":"modeldeck","base_url":base,"api_key":p.local_token,"api_mode":mode,"models":p.models.iter().map(|m|json!({"id":m.id,"context_length":m.context})).collect::<Vec<_>>() })
        }
        _ => return Err("Unknown provider harness".into()),
    };
    let mut provider = Provider::with_id("modeldeck".into(), "ModelDeck".into(), config, None);
    if client == "claude-desktop" {
        provider.meta = Some(ProviderMeta {
            api_format: Some("anthropic".into()),
            claude_desktop_model_routes: p
                .models
                .iter()
                .map(|m| {
                    (
                        m.id.clone(),
                        ClaudeDesktopModelRoute {
                            model: m.id.clone(),
                            label_override: Some(m.name.clone()),
                            supports_1m: Some(m.context >= 1_000_000),
                        },
                    )
                })
                .collect::<HashMap<_, _>>(),
            ..Default::default()
        });
    }
    Ok(provider)
}

pub fn command(client: &str) -> Option<String> {
    (matches!(client, "antigravity" | "muse" | "cursor-agent")
        || client == "claude" && get_app_config_dir().join("bin/claude").exists())
    .then(|| {
        get_app_config_dir()
            .join("bin")
            .join(client)
            .to_string_lossy()
            .to_string()
    })
}

fn claude_executable(p: &ClientProfile) -> Result<Option<PathBuf>, String> {
    let managed = get_app_config_dir().join("bin/claude");
    let usable = |path: &PathBuf| {
        if path == &managed || !path.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
        }
        #[cfg(not(unix))]
        {
            true
        }
    };
    if !p.executable.is_empty() {
        let path = PathBuf::from(&p.executable);
        return if usable(&path) {
            Ok(Some(path))
        } else {
            Err("Select the original Claude Code executable; managed launchers cannot launch themselves".into())
        };
    }
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|dir| dir.join("claude")));
    }
    candidates.extend([
        get_home_dir().join(".local/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
    ]);
    Ok(candidates.into_iter().find(usable))
}

fn claude_launcher(p: &ClientProfile, executable: &Path) -> String {
    let quote = |s: &str| format!("'{}'", s.replace('\'', "'\"'\"'"));
    let mut script = format!("#!/bin/sh\n# Managed by ModelDeck: model connection only.\nset -eu\nmodel={}\neffort={}\nhas_effort=0\nnext_model=0\nfor arg do\n  if [ \"$next_model\" = 1 ]; then model=$arg; next_model=0; continue; fi\n  case \"$arg\" in\n    --) break;;\n    --model) next_model=1;;\n    --model=*) model=${{arg#--model=}};;\n    --effort|--effort=*) has_effort=1;;\n  esac\ndone\ncase \"$model\" in\n", quote(&p.default_model), quote(&p.effort));
    for model in &p.models {
        script.push_str(&format!(
            "  {}) effort={};;\n",
            quote(&model.id),
            quote(&model.default_effort)
        ));
    }
    script.push_str(&format!("esac\nif [ \"$has_effort\" = 0 ] && [ -n \"$effort\" ]; then set -- --effort \"$effort\" \"$@\"; fi\nexec {} \"$@\"\n", quote(&executable.to_string_lossy())));
    script
}

pub fn apply(
    client: &str,
    p: &ClientProfile,
    previous: Option<&ClientProfile>,
    state: &AppState,
    commit: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let claude_executable = if client == "claude" {
        claude_executable(p)?
    } else {
        None
    };
    let adapter = ModelClient::try_from_id(client);
    if let Some(adapter) = adapter {
        adapters::preflight(adapter, p)?;
    }
    let paths = paths(client)?;
    let before = capture(&paths)?;
    let path = backup_path(client)?;
    let existing = read_backup(client, &path)?;
    let mut original = existing
        .as_ref()
        .map(|b| b.original.clone())
        .unwrap_or_else(|| before.clone());
    // A newer adapter may own an additional file (for example Claude's max
    // launcher). Include its pre-apply state even when an older backup exists.
    for snapshot in &before {
        if !original.iter().any(|old| old.path == snapshot.path) {
            original.push(snapshot.clone());
        }
    }
    let result = (|| {
        if let Some(adapter) = adapter {
            // Native adapters consume an ephemeral catalog; the global graph
            // remains the only model source and is read on every gateway call.
            adapters::apply_deck(adapter, p, previous, || Ok(()))?;
        } else {
            let provider = generated_provider(client, p)?;
            match client {
                "codex" => {
                    let profile =
                        crate::proxy::providers::resolve_codex_catalog_tool_profile(&provider);
                    crate::codex_config::write_codex_deck_live_with_catalog(
                        &provider.settings_config,
                        profile,
                    )
                    .map_err(|e| e.to_string())?;
                }
                "claude" => crate::config::write_json_file(
                    &crate::config::get_claude_settings_path(),
                    &provider.settings_config,
                )
                .map_err(|e| e.to_string())?,
                "claude-desktop" => {
                    crate::claude_desktop_config::apply_provider(&state.db, &provider)
                        .map_err(|e| e.to_string())?
                }
                "grokbuild" => crate::grok_config::write_grok_provider_live(&provider)
                    .map_err(|e| e.to_string())?,
                "opencode" => crate::opencode_config::set_provider(
                    &provider.id,
                    provider.settings_config.clone(),
                )
                .map_err(|e| e.to_string())?,
                "hermes" => {
                    crate::hermes_config::set_provider(
                        &provider.id,
                        provider.settings_config.clone(),
                    )
                    .map_err(|e| e.to_string())?;
                }
                _ => return Err("Unknown provider harness".into()),
            }
            if client == "claude" {
                if let Some(executable) = claude_executable {
                    let path = get_app_config_dir().join("bin/claude");
                    atomic_write_private(&path, claude_launcher(p, &executable).as_bytes())
                        .map_err(|e| e.to_string())?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            if client == "opencode" {
                let mut config =
                    crate::opencode_config::read_opencode_config().map_err(|e| e.to_string())?;
                config["model"] = json!(format!("modeldeck/{}", p.default_model));
                crate::config::write_json_file(
                    &crate::opencode_config::get_opencode_config_path(),
                    &config,
                )
                .map_err(|e| e.to_string())?;
            } else if client == "hermes" {
                let mut model = crate::hermes_config::get_model_config()
                    .map_err(|e| e.to_string())?
                    .unwrap_or_default();
                let selected = p
                    .models
                    .iter()
                    .find(|m| m.id == p.default_model)
                    .ok_or("Default model is unavailable")?;
                model.default = Some(p.default_model.clone());
                model.provider = Some("modeldeck".into());
                model.base_url = Some(format!("{}/v1", p.endpoint.trim_end_matches('/')));
                model.context_length = Some(selected.context);
                model.max_tokens = Some(selected.output);
                for field in ["api_key", "apiKey", "key_env", "reasoning_effort"] {
                    model.extra.remove(field);
                }
                crate::hermes_config::set_model_config(&model).map_err(|e| e.to_string())?;
            }
        }
        let backup = Backup {
            original,
            applied: capture(&paths)?,
        };
        atomic_write_private(
            &path,
            &serde_json::to_vec(&backup).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        commit()
    })();
    if let Err(error) = result {
        restore_files(&before)?;
        if let Some(existing) = existing {
            atomic_write_private(
                &path,
                &serde_json::to_vec(&existing).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        } else if path.exists() {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        return Err(error);
    }
    Ok(())
}

pub fn restore(client: &str, commit: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    if client == "cursor" {
        adapters::ensure_cursor_closed()?;
    }
    let path = backup_path(client)?;
    let backup =
        read_backup(client, &path)?.ok_or("Original native configuration backup is unavailable")?;
    let before = capture(
        &backup
            .applied
            .iter()
            .map(|s| s.path.clone())
            .collect::<Vec<_>>(),
    )?;
    for (current, expected) in before.iter().zip(&backup.applied) {
        let digest = |bytes: &Option<Vec<u8>>| bytes.as_ref().map(|b| Sha256::digest(b).to_vec());
        if digest(&current.bytes) != digest(&expected.bytes) {
            return Err(format!("{} changed after ModelDeck applied it; restore was stopped to preserve those edits",current.path.display()));
        }
    }
    if let Err(error) = restore_files(&backup.original) {
        restore_files(&before)?;
        return Err(error);
    }
    if let Err(error) = commit() {
        restore_files(&before)?;
        return Err(error);
    }
    fs::remove_file(path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests;
