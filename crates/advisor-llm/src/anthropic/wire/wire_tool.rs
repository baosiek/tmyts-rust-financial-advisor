use serde::Serialize;
use serde_json::Value;

use crate::types::ToolDefinition;

/// Anthropic's JSON shape of a tool definition.
#[derive(Debug, Serialize)]
pub(crate) struct WireTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl From<&ToolDefinition> for WireTool {
    fn from(tool: &ToolDefinition) -> Self {
        Self {
            name: tool.name.clone(),
            description: tool.description.clone(),
            input_schema: tool.input_schema.clone(),
        }
    }
}
