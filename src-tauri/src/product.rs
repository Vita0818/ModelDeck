pub const APP_NAME: &str = "ModelDeck";

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
