use serde::Deserialize;

/// `GET /v1/models/{model}`: the one model the health check asks about.
#[derive(Debug, Deserialize)]
pub(crate) struct ModelInfo {
    pub id: String,
    #[serde(default)]
    pub owned_by: Option<String>,
}
