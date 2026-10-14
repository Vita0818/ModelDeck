//! Provider-specific authentication, HTTP transport and protocol conversion.
pub(crate) mod content_encoding;
mod request_body;
pub(crate) use request_body::decode_codex_request_body;
pub mod error;
#[cfg(test)]
pub mod gemini_url;
pub mod http_client;
pub(crate) mod json_canonical;
pub mod providers;
pub mod session;
pub(crate) mod sse;
pub mod thinking_optimizer;
pub(crate) mod tool_media;
pub(crate) mod types;
pub mod usage;
