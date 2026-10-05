use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Protocol {
    OpenaiChat,
    OpenaiResponses,
    Anthropic,
    Gemini,
}

impl Protocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenaiChat => "openai-chat",
            Self::OpenaiResponses => "openai-responses",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamProvider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub protocol: Protocol,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub key_env: String,
    #[serde(default)]
    pub key_source: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelResource {
    pub id: String,
    pub provider_id: String,
    pub upstream_id: String,
    pub name: String,
    pub context: u64,
    pub output: u64,
    #[serde(default)]
    pub reasoning: Vec<String>,
    #[serde(default)]
    pub reasoning_supported: Option<bool>,
    #[serde(default = "yes")]
    pub tools: bool,
    #[serde(default)]
    pub images: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRoute {
    pub model_id: String,
    #[serde(default)]
    pub effort: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExposedEndpoint {
    pub id: String,
    pub name: String,
    pub protocols: Vec<Protocol>,
    /// Public ingress to this endpoint, including its /deck/<id> prefix.
    #[serde(default)]
    pub public_url: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub routes: BTreeMap<String, ModelRoute>,
}

/// References only. Provider credentials, model metadata and tiers belong to
/// the global resource graph, never to a harness binding.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessBinding {
    pub endpoint_id: String,
    pub model: String,
    #[serde(default)]
    pub utility_model: String,
    #[serde(default)]
    pub executable: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_protocol: Option<Protocol>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationReport {
    pub imported_providers: usize,
    pub imported_models: usize,
    pub retained: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckStore {
    pub schema_version: u32,
    pub revision: u64,
    pub providers: BTreeMap<String, UpstreamProvider>,
    pub models: BTreeMap<String, ModelResource>,
    pub endpoints: BTreeMap<String, ExposedEndpoint>,
    pub bindings: BTreeMap<String, HarnessBinding>,
    pub active_bindings: BTreeMap<String, HarnessBinding>,
    pub migration: MigrationReport,
}

impl Default for DeckStore {
    fn default() -> Self {
        Self {
            schema_version: 1,
            revision: 0,
            providers: BTreeMap::new(),
            models: BTreeMap::new(),
            endpoints: BTreeMap::new(),
            bindings: BTreeMap::new(),
            active_bindings: BTreeMap::new(),
            migration: MigrationReport::default(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Compatibility {
    pub client: String,
    pub endpoint_id: String,
    pub models: Vec<String>,
    pub issue: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckView {
    pub store: DeckStore,
    pub compatibility: Vec<Compatibility>,
    pub origin: String,
}
