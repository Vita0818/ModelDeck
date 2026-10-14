//! Preserve native reasoning controls; fill the route default only when absent.
use super::{ModelResource, Protocol, LEVELS};
use serde_json::{json, Value};

pub(super) fn preferred(levels: &[String]) -> &str {
    if levels.iter().any(|level| level == "max") {
        "max"
    } else {
        LEVELS
            .iter()
            .rev()
            .find(|level| levels.iter().any(|candidate| candidate == **level))
            .copied()
            .unwrap_or("")
    }
}

fn effort(input: Protocol, body: &Value) -> Option<&Value> {
    match input {
        Protocol::OpenaiResponses => body.pointer("/reasoning/effort"),
        Protocol::OpenaiChat => body
            .pointer("/reasoning/effort")
            .or_else(|| body.get("reasoning_effort")),
        Protocol::Anthropic => body.pointer("/output_config/effort"),
        Protocol::Gemini => body
            .pointer("/generationConfig/thinkingConfig/thinkingLevel")
            .or_else(|| body.pointer("/generation_config/thinking_config/thinking_level")),
    }
}

fn has_reasoning_controls(body: &Value) -> bool {
    match body.get("reasoning") {
        None => false,
        Some(Value::Object(fields)) => fields.keys().any(|key| key != "summary"),
        Some(_) => true,
    }
}

pub(super) fn prepare(
    input: Protocol,
    body: &mut Value,
    model: &ModelResource,
    default: &str,
) -> String {
    let configured = match input {
        Protocol::OpenaiResponses => has_reasoning_controls(body),
        Protocol::OpenaiChat => {
            has_reasoning_controls(body)
                || body.get("reasoning_effort").is_some()
                || body.get("thinking").is_some()
                || body.get("enable_thinking").is_some()
                || body.get("reasoning_split").is_some()
        }
        Protocol::Anthropic => body.get("thinking").is_some() || effort(input, body).is_some(),
        Protocol::Gemini => {
            body.pointer("/generationConfig/thinkingConfig").is_some()
                || body.pointer("/generation_config/thinking_config").is_some()
        }
    };
    if !configured && !default.is_empty() && model.reasoning_supported != Some(false) {
        match input {
            Protocol::OpenaiChat => body["reasoning_effort"] = json!(default),
            Protocol::OpenaiResponses => {
                if body.get("reasoning").is_none() {
                    body["reasoning"] = json!({});
                }
                if let Some(reasoning) = body["reasoning"].as_object_mut() {
                    reasoning.insert("effort".into(), json!(default));
                }
            }
            Protocol::Anthropic => {
                if body.get("output_config").is_none() {
                    body["output_config"] = json!({});
                }
                if let Some(config) = body["output_config"].as_object_mut() {
                    config.insert("effort".into(), json!(default));
                }
            }
            Protocol::Gemini => {
                let (config, thinking, level) = if body.get("generation_config").is_some() {
                    ("generation_config", "thinking_config", "thinking_level")
                } else {
                    ("generationConfig", "thinkingConfig", "thinkingLevel")
                };
                if body.get(config).is_none() {
                    body[config] = json!({});
                }
                if let Some(config) = body[config].as_object_mut() {
                    config.insert(thinking.into(), json!({level:default.to_uppercase()}));
                }
            }
        }
    }
    let selected = effort(input, body).and_then(Value::as_str).unwrap_or("");
    // Only the bridge's field spelling/casing changes; the native body is intact.
    if input == Protocol::Gemini {
        selected.to_ascii_lowercase()
    } else {
        selected.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_preferences_preserve_the_summary_and_fill_the_missing_route_effort() {
        let model = crate::deck::tests::fixture()
            .models
            .remove("resource")
            .unwrap();
        for summary in ["auto", "concise", "detailed"] {
            let mut body = json!({"reasoning":{"summary":summary}});
            assert_eq!(
                prepare(Protocol::OpenaiResponses, &mut body, &model, "high"),
                "high"
            );
            assert_eq!(
                body,
                json!({"reasoning":{"summary":summary,"effort":"high"}})
            );
        }
        for mut body in [
            json!({"reasoning":{"summary":"auto","effort":"low"}}),
            json!({"reasoning":{"summary":"auto","enabled":false}}),
            json!({"reasoning":{"summary":"auto","max_tokens":1024}}),
            json!({"reasoning":{"summary":"auto","vendor_option":true}}),
        ] {
            let original = body.clone();
            prepare(Protocol::OpenaiResponses, &mut body, &model, "high");
            assert_eq!(body, original);
        }
    }

    #[test]
    fn explicit_native_levels_are_not_clamped_or_renamed() {
        let mut model = crate::deck::tests::fixture()
            .models
            .remove("resource")
            .unwrap();
        for supported in [None, Some(false), Some(true)] {
            model.reasoning_supported = supported;
            for level in ["none", "low", "xhigh", "ultra", "auto", "vendor-level"] {
                for (protocol, mut body) in [
                    (
                        Protocol::OpenaiResponses,
                        json!({"reasoning":{"effort":level,"summary":"auto","vendor_option":true}}),
                    ),
                    (Protocol::OpenaiChat, json!({"reasoning_effort":level})),
                    (
                        Protocol::OpenaiChat,
                        json!({"reasoning":{"effort":level,"enabled":true}}),
                    ),
                    (
                        Protocol::Anthropic,
                        json!({"output_config":{"effort":level,"format":"text"},"thinking":{"type":"adaptive"}}),
                    ),
                ] {
                    let original = body.clone();
                    assert_eq!(prepare(protocol, &mut body, &model, "max"), level);
                    assert_eq!(body, original);
                }
            }
        }
    }

    #[test]
    fn route_defaults_only_fill_missing_controls() {
        let model = crate::deck::tests::fixture()
            .models
            .remove("resource")
            .unwrap();
        for protocol in [
            Protocol::OpenaiResponses,
            Protocol::OpenaiChat,
            Protocol::Anthropic,
            Protocol::Gemini,
        ] {
            let mut body = json!({});
            assert_eq!(prepare(protocol, &mut body, &model, "max"), "max");
        }
        let mut body = json!({"output_config":{"format":"text"}});
        prepare(Protocol::Anthropic, &mut body, &model, "max");
        assert_eq!(
            body,
            json!({"output_config":{"format":"text","effort":"max"}})
        );
    }

    #[test]
    fn budgets_disable_flags_and_native_casing_are_preserved() {
        let model = crate::deck::tests::fixture()
            .models
            .remove("resource")
            .unwrap();
        for (protocol, mut body) in [
            (
                Protocol::OpenaiResponses,
                json!({"reasoning":{"enabled":false}}),
            ),
            (
                Protocol::OpenaiResponses,
                json!({"reasoning":{"max_tokens":2048}}),
            ),
            (
                Protocol::Anthropic,
                json!({"thinking":{"type":"enabled","budget_tokens":2048}}),
            ),
            (Protocol::Anthropic, json!({"thinking":{"type":"disabled"}})),
            (
                Protocol::Gemini,
                json!({"generationConfig":{"thinkingConfig":{"thinkingBudget":-1,"includeThoughts":false}}}),
            ),
        ] {
            let original = body.clone();
            assert_eq!(prepare(protocol, &mut body, &model, "max"), "");
            assert_eq!(body, original);
        }
        let mut body = json!({"generationConfig":{"thinkingConfig":{"thinkingLevel":"LOW","includeThoughts":true}}});
        let original = body.clone();
        assert_eq!(prepare(Protocol::Gemini, &mut body, &model, "max"), "low");
        assert_eq!(body, original);
    }

    #[test]
    fn preferred_default_still_uses_max_or_the_highest_available_level() {
        assert_eq!(
            preferred(&["ultra".into(), "max".into(), "high".into()]),
            "max"
        );
        assert_eq!(
            preferred(&["low".into(), "xhigh".into(), "high".into()]),
            "xhigh"
        );
    }
}
