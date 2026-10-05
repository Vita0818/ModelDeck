pub const APP_NAME: &str = "ModelDeck";
pub const APP_UPDATES_ENABLED: bool = false;
pub const UPSTREAM_URL: &str = "https://github.com/farion1231/cc-switch";
pub const MANUAL_UPDATE_MESSAGE: &str =
    "ModelDeck uses local builds; an automatic update channel is not configured.";

/// Supported provider-backed clients. Legacy enum values remain readable for
/// database compatibility, but are excluded from automatic client operations.
pub fn supports_provider_app(app: &crate::app_config::AppType) -> bool {
    use crate::app_config::AppType;
    matches!(
        app,
        AppType::Codex
            | AppType::Claude
            | AppType::ClaudeDesktop
            | AppType::GrokBuild
            | AppType::OpenCode
            | AppType::Hermes
    )
}

pub const MODEL_CLIENTS: [(&str, &str); 4] = [
    ("antigravity", "Antigravity CLI"),
    ("cursor-agent", "Cursor CLI"),
    ("cursor", "Cursor GUI"),
    ("muse", "Muse Code"),
];
