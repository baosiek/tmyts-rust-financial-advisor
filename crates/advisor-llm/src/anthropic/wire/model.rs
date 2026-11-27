use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Models {
    pub data: Vec<Model>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Model {
    #[serde(rename = "type")]
    pub item_type: String,
    pub id: String,
    pub display_name: String,
}
