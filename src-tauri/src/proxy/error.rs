use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum ProxyError {
    #[error("配置错误: {0}")]
    ConfigError(String),
    #[error("格式转换错误: {0}")]
    TransformError(String),
    #[error("无效的请求: {0}")]
    InvalidRequest(String),
    #[error("认证失败: {0}")]
    AuthError(String),
}
impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::ConfigError(_) | Self::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            Self::TransformError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::AuthError(_) => StatusCode::UNAUTHORIZED,
        };
        (
            status,
            Json(json!({"error":{"message":self.to_string(),"type":"proxy_error"}})),
        )
            .into_response()
    }
}
