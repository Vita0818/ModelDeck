//! Model-only launch configurations for profile-based and BYOK clients.
//! Files belong to ModelDeck and participate in native::apply's transaction.
use crate::{
    config::{atomic_write_private, get_app_config_dir, get_home_dir},
    model_clients::{ClientModel, ClientProfile},
};
use serde_json::{json, Value};
use std::{fs, path::PathBuf};

pub(super) fn supports(client: &str) -> bool {
    matches!(client, "dsh" | "copilot")
}

fn config_path(client: &str) -> PathBuf {
    get_app_config_dir()
        .join("deck-clients")
        .join(match client {
            "dsh" => "dsh.patch.yml",
            "copilot" => "copilot.providers.json",
            _ => unreachable!("unsupported managed CLI"),
        })
}

pub(super) fn paths(client: &str) -> Vec<PathBuf> {
    vec![
        config_path(client),
        get_app_config_dir().join("bin").join(client),
    ]
}

pub(super) fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(super) fn preflight(client: &str, profile: &ClientProfile) -> Result<PathBuf, String> {
    if !cfg!(unix) {
        return Err("Managed CLI launchers currently require macOS or Linux".into());
    }
    let mut candidates = Vec::new();
    if profile.executable.is_empty() {
        if let Some(path) = std::env::var_os("PATH") {
            candidates.extend(std::env::split_paths(&path).map(|dir| dir.join(client)));
        }
        candidates.extend([
            get_home_dir().join(".local/bin").join(client),
            PathBuf::from("/opt/homebrew/bin").join(client),
            PathBuf::from("/usr/local/bin").join(client),
        ]);
    } else {
        candidates.push(PathBuf::from(&profile.executable));
    }
    let managed = get_app_config_dir().join("bin");
    let managed = managed.canonicalize().unwrap_or(managed);
    candidates
        .into_iter()
        .find(|path| {
            let Ok(canonical) = path.canonicalize() else { return false };
            if !path.is_absolute() || canonical.starts_with(&managed) || !canonical.is_file() {
                return false;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
            }
            #[cfg(not(unix))]
            { false }
        })
        .ok_or_else(|| format!("Select the original {client} executable, or install it on PATH; a ModelDeck launcher cannot launch itself"))
}

fn dsh_effort(effort: &str) -> Option<&str> {
    match effort {
        "none" => Some("off"),
        "minimal" | "low" | "medium" | "high" | "xhigh" | "max" => Some(effort),
        _ => None,
    }
}

fn dsh_model(model: &ClientModel) -> Value {
    let efforts: serde_json::Map<String, Value> = model
        .reasoning
        .iter()
        .filter_map(|effort| {
            dsh_effort(effort).map(|native| {
                (
                    native.into(),
                    if native == "off" {
                        Value::Null
                    } else {
                        json!(effort)
                    },
                )
            })
        })
        .collect();
    let reasoning = if model.reasoning_supported == Some(false)
        || !efforts.keys().any(|level| level != "off")
    {
        json!(false)
    } else {
        Value::Object(efforts)
    };
    json!({
        "id":model.id, "name":model.name,
        "contextWindow":model.context, "maxTokens":model.output,
        "input":if model.images {vec!["text", "image"]} else {vec!["text"]},
        "reasoningEfforts":reasoning,
    })
}

fn dsh_config(profile: &ClientProfile) -> Result<String, String> {
    let api = match profile.protocol.as_str() {
        "openai-chat" => "openai-completions",
        "openai-responses" => "openai-responses",
        "anthropic" => "anthropic-messages",
        _ => return Err("Unsupported DeepSeek Harness protocol".into()),
    };
    let route = json!({
        "displayName":"ModelDeck", "apiKeyEnv":"MODELDECK_DSH_TOKEN", "api":api,
        "models":profile.models.iter().map(dsh_model).collect::<Vec<_>>(),
    });
    let route = serde_yaml::to_string(&route).map_err(|e| e.to_string())?;
    // DSH's native expression tag reads the launcher's current gateway origin.
    // User-controlled names and model IDs are always emitted by the serializer.
    let mut patch = String::from("- id: llm-pi-ai\n  config:\n    providers:\n      modeldeck:\n        baseURL: !!js process.env.MODELDECK_DSH_BASE_URL\n");
    for line in route.lines() {
        patch.push_str(&format!("        {line}\n"));
    }
    let utility = if profile.utility_model.is_empty() {
        &profile.default_model
    } else {
        &profile.utility_model
    };
    patch.push_str(&serde_yaml::to_string(&json!([
        {"id":"agent-default-model","config":{"provider":"modeldeck","model":profile.default_model}},
        {"id":"session-title-llm","config":{"provider":"modeldeck","model":utility}},
    ])).map_err(|e| e.to_string())?);
    // Clear an inherited effort when only the gateway can represent it (ultra),
    // or when the selected resource has no reasoning capability.
    let selected = profile
        .models
        .iter()
        .find(|m| m.id == profile.default_model)
        .ok_or("Default model is unavailable")?;
    let effort = if selected.reasoning_supported == Some(false) {
        None
    } else {
        dsh_effort(&profile.effort)
    };
    patch.push_str("- id: agent-default-model\n  config:\n    reasoningEffort: ");
    patch.push_str(effort.unwrap_or("!!js undefined"));
    patch.push('\n');
    Ok(patch)
}

fn launcher(
    client: &str,
    profile: &ClientProfile,
    executable: &std::path::Path,
) -> Result<String, String> {
    let q = quote;
    let suffix = format!("/deck-clients/{client}");
    let initial_origin = profile
        .endpoint
        .trim_end_matches('/')
        .strip_suffix(&suffix)
        .ok_or("Invalid managed gateway endpoint")?;
    let mut script = format!("#!/bin/sh\n# Managed by ModelDeck: model connection only.\nset -eu\norigin={}\norigin_file={}\nif [ -r \"$origin_file\" ]; then IFS= read -r origin < \"$origin_file\"; fi\norigin=${{origin%/}}\n",
        q(initial_origin), q(&get_app_config_dir().join("deck-gateway-url").to_string_lossy()));
    match client {
        "dsh" => {
            script.push_str(&format!("export MODELDECK_DSH_BASE_URL=\"$origin/deck-clients/dsh/v1\"\nexport MODELDECK_DSH_TOKEN={}\nif [ \"$#\" = 0 ]; then\n  set -- --profile web\nelif [ \"$1\" = plugin ]; then\n  exec {} \"$@\"\nelif [ \"${{1#-}}\" = \"$1\" ]; then\n  profile=$1; shift\n  set -- --profile \"$profile\" \"$@\"\nfi\nexec {} --patch {} \"$@\"\n",
                q(&profile.local_token), q(&executable.to_string_lossy()), q(&executable.to_string_lossy()), q(&config_path(client).to_string_lossy())));
        }
        "copilot" => {
            script.push_str("export COPILOT_PROVIDER_TRANSPORT=http\n");
            // A managed empty registry deliberately selects the documented
            // environment-based BYOK path, even if the user has a registry.
            script.push_str(&format!("export COPILOT_PROVIDERS_CONFIG={}\nunset COPILOT_PROVIDER_API_KEY_COMMAND COPILOT_PROVIDER_BEARER_TOKEN COPILOT_PROVIDER_HEADERS COPILOT_PROVIDER_AZURE_API_VERSION\nexport COPILOT_PROVIDER_API_KEY={}\nexport COPILOT_PROVIDER_BASE_URL=\"$origin/deck-clients/copilot/v1\"\nexport COPILOT_PROVIDER_TYPE={}\nexport COPILOT_PROVIDER_WIRE_API={}\nmodel={}\nnext_model=0\nskip_value=0\nfor arg do\n  if [ \"$next_model\" = 1 ]; then model=$arg; next_model=0; continue; fi\n  if [ \"$skip_value\" = 1 ]; then skip_value=0; continue; fi\n  case \"$arg\" in\n    --) break;;\n    --model) next_model=1;;\n    --model=*) model=${{arg#--model=}};;\n    -p|--prompt|-i|--interactive|--agent|--resume|--config-dir|--log-dir|--log-level|--additional-mcp-config) skip_value=1;;\n  esac\ndone\ncase \"$model\" in\n",
                q(&config_path(client).to_string_lossy()), q(&profile.local_token),
                q(if profile.protocol == "anthropic" {"anthropic"} else {"openai"}),
                q(if profile.protocol == "openai-responses" {"responses"} else {"completions"}), q(&profile.default_model)));
            for model in &profile.models {
                script.push_str(&format!("  {}) export COPILOT_PROVIDER_MAX_PROMPT_TOKENS={} COPILOT_PROVIDER_MAX_OUTPUT_TOKENS={};;\n",q(&model.id), model.context.saturating_sub(model.output).max(1), model.output));
            }
            script.push_str(&format!("  *) printf '%s\\n' 'Select a model from this ModelDeck connection' >&2; exit 1;;\nesac\nexport COPILOT_MODEL=\"$model\" COPILOT_PROVIDER_MODEL_ID=\"$model\" COPILOT_PROVIDER_WIRE_MODEL=\"$model\"\nexec {} \"$@\"\n",q(&executable.to_string_lossy())));
        }
        _ => unreachable!("unsupported managed CLI"),
    }
    Ok(script)
}

pub(super) fn apply(
    client: &str,
    profile: &ClientProfile,
    executable: &std::path::Path,
) -> Result<(), String> {
    let config = if client == "dsh" {
        dsh_config(profile)?
    } else {
        "{}\n".into()
    };
    atomic_write_private(&config_path(client), config.as_bytes()).map_err(|e| e.to_string())?;
    let path = get_app_config_dir().join("bin").join(client);
    atomic_write_private(&path, launcher(client, profile, executable)?.as_bytes())
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    Ok(())
}
