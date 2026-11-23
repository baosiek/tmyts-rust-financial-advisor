use serde_json::Value;

/// Describes a tool to the model: its name, what it does, and the JSON schema
/// of its input. The model reads all three to decide when and how to call it.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}
