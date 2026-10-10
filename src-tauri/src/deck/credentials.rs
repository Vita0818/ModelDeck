use super::UpstreamProvider;
use regex::Regex;
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

fn references() -> &'static Regex {
    static REFERENCES: OnceLock<Regex> = OnceLock::new();
    REFERENCES.get_or_init(|| Regex::new(r"\{(env|file):([^{}]+)\}").unwrap())
}

fn file_path(value: &str, base: &Path) -> PathBuf {
    let path = if value == "~" {
        crate::config::get_home_dir()
    } else if let Some(relative) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        crate::config::get_home_dir().join(relative)
    } else {
        PathBuf::from(value)
    };
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

/// Preserve dynamic credential sources during migration, never their contents.
pub(super) fn imported(value: &str, key_env: &str, base: &Path) -> (String, String) {
    if key_env.is_empty() {
        if let Some(reference) = references().captures(value.trim()) {
            let matched = reference.get(0).unwrap();
            let name = &reference[2];
            if matched.as_str() == value.trim()
                && &reference[1] == "env"
                && name.chars().enumerate().all(|(i, c)| {
                    c == '_' || c.is_ascii_alphabetic() || i > 0 && c.is_ascii_digit()
                })
            {
                return (String::new(), name.to_string());
            }
        }
    }
    let value = references().replace_all(value, |reference: &regex::Captures<'_>| {
        if &reference[1] == "file" {
            format!("{{file:{}}}", file_path(&reference[2], base).display())
        } else {
            reference.get(0).unwrap().as_str().to_string()
        }
    });
    (value.into_owned(), key_env.into())
}

fn valid(value: String) -> Result<String, String> {
    if value.contains("{file:") || value.contains("{env:") {
        return Err("The API credential contains an unresolved reference".into());
    }
    let value = value.trim().to_string();
    if value.chars().any(char::is_control) {
        return Err(
            "The API credential must contain a single line without control characters".into(),
        );
    }
    Ok(value)
}

fn resolve_from(
    provider: &UpstreamProvider,
    base: &Path,
    mut env: impl FnMut(&str) -> Option<String>,
) -> Result<String, String> {
    if !provider.key_env.is_empty() {
        let value = env(&provider.key_env)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "The credential environment variable is unavailable".to_string())?;
        return valid(value);
    }
    let value = &provider.api_key;
    let mut resolved = String::new();
    let mut offset = 0;
    for reference in references().captures_iter(value) {
        let matched = reference.get(0).unwrap();
        resolved.push_str(&value[offset..matched.start()]);
        let part = if &reference[1] == "env" {
            env(&reference[2])
                .ok_or_else(|| "The credential environment variable is unavailable".to_string())?
        } else {
            std::fs::read_to_string(file_path(&reference[2], base))
                .map_err(|_| "The credential file is missing, unreadable or not UTF-8".to_string())?
                .trim()
                .to_string()
        };
        if part.trim().is_empty() {
            return Err("The referenced API credential is empty".into());
        }
        resolved.push_str(&part);
        offset = matched.end();
    }
    resolved.push_str(&value[offset..]);
    // An empty literal remains valid for local servers such as LM Studio.
    valid(resolved)
}

pub(super) fn resolve(provider: &UpstreamProvider, base: &Path) -> Result<String, String> {
    resolve_from(provider, base, |name| std::env::var(name).ok())
        .map_err(|error| format!("{}: {error}", provider.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn provider(key: &str, env: &str) -> UpstreamProvider {
        serde_json::from_value(json!({"id":"upstream","name":"Test provider",
            "baseUrl":"https://example.test/v1","protocol":"openai-chat",
            "apiKey":key,"keyEnv":env}))
        .unwrap()
    }

    #[test]
    fn literal_and_unauthenticated_local_credentials_remain_supported() {
        for key in ["plain-test-key", ""] {
            assert_eq!(
                resolve_from(&provider(key, ""), Path::new("/unused"), |_| None).unwrap(),
                key
            );
        }
    }

    #[test]
    fn file_references_read_the_current_key_without_persisting_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");
        std::fs::write(&path, "first-test-key\n").unwrap();
        let reference = format!("{{file:{}}}", path.display());
        let p = provider(&reference, "");
        assert_eq!(
            resolve_from(&p, dir.path(), |_| None).unwrap(),
            "first-test-key"
        );
        std::fs::write(&path, "rotated-test-key\n").unwrap();
        assert_eq!(
            resolve_from(&p, dir.path(), |_| None).unwrap(),
            "rotated-test-key"
        );
        assert_eq!(p.api_key, reference);
    }

    #[test]
    fn migrated_relative_file_references_keep_the_original_config_directory() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("token"), "source-test-key\n").unwrap();
        let (key, env) = imported("{file:token}", "", source.path());
        assert!(env.is_empty());
        assert_eq!(
            resolve_from(&provider(&key, ""), destination.path(), |_| None).unwrap(),
            "source-test-key"
        );
        assert!(!key.contains("source-test-key"));
    }

    #[test]
    fn environment_references_and_named_environment_sources_resolve_in_memory() {
        for p in [
            provider("{env:MODELDECK_TEST_KEY}", ""),
            provider("", "MODELDECK_TEST_KEY"),
        ] {
            assert_eq!(
                resolve_from(&p, Path::new("/unused"), |name| (name
                    == "MODELDECK_TEST_KEY")
                    .then(|| "env-test-key".into()))
                .unwrap(),
                "env-test-key"
            );
        }
        let (key, env) = imported("{env:MODELDECK_TEST_KEY}", "", Path::new("/unused"));
        assert!(key.is_empty());
        assert_eq!(env, "MODELDECK_TEST_KEY");
    }

    #[test]
    fn unavailable_empty_unresolved_and_multiline_credentials_fail_locally() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("empty"), "\n").unwrap();
        std::fs::write(dir.path().join("multiline"), "first\nsecond").unwrap();
        for key in [
            "{file:missing}",
            "{file:empty}",
            "{file:multiline}",
            "{env:MISSING}",
            "{env:EMPTY}",
            "{file:unclosed",
            "key\r\nvalue",
        ] {
            assert!(
                resolve_from(&provider(key, ""), dir.path(), |name| (name == "EMPTY")
                    .then(String::new))
                .is_err(),
                "invalid credential was accepted"
            );
        }
    }
}
