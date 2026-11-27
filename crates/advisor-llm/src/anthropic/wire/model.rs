use serde::Deserialize;

/// `GET /v1/models/{model_id}`: the one model the health check asks about.
#[derive(Debug, Deserialize)]
pub(crate) struct ModelInfo {
    pub id: String,
    #[serde(default)]
    pub display_name: Option<String>,
}
