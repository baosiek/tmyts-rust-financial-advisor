use serde::Serialize;
use serde_json::Value;

use crate::types::ToolDefinition;

/// A function tool in the Responses API. Flat: there is no nested `function`
/// object as in Chat Completions.
#[derive(Debug, Serialize)]
pub(crate) struct WireTool {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub name: String,
    pub description: String,
    pub parameters: Value,
    /// Strict mode needs every property to be required, which our optional tool
    /// inputs are not. The tools validate their input themselves, so strict
    /// mode is switched off explicitly instead of relying on a silent fallback.
    pub strict: bool,
}

impl From<&ToolDefinition> for WireTool {
    fn from(tool: &ToolDefinition) -> Self {
        Self {
            kind: "function",
            name: tool.name.clone(),
            description: tool.description.clone(),
            parameters: tool.input_schema.clone(),
            strict: false,
        }
    }
}
