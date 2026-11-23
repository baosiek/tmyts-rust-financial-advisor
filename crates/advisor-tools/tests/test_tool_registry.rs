use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

use advisor_tools::{CurrentDateTool, Tool, ToolError, ToolRegistry};

/// A tool with a real input, to check schema generation and input validation.
struct AddTool;

#[derive(Debug, Deserialize, JsonSchema)]
struct AddInput {
    /// First number.
    a: i64,
    /// Second number.
    b: i64,
}

#[async_trait]
impl Tool for AddTool {
    const NAME: &'static str = "add";
    const DESCRIPTION: &'static str = "Adds two numbers.";
    type Input = AddInput;

    async fn run(&self, input: AddInput) -> Result<String, ToolError> {
        Ok((input.a + input.b).to_string())
    }
}

fn registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(AddTool);
    registry.register(CurrentDateTool);
    registry
}

#[test]
fn definitions_include_generated_schemas() {
    let definitions = registry().definitions();
    assert_eq!(definitions.len(), 2);

    let add = definitions.iter().find(|d| d.name == "add").unwrap();
    assert_eq!(add.description, "Adds two numbers.");
    assert_eq!(add.input_schema["type"], "object");
    assert_eq!(add.input_schema["properties"]["a"]["type"], "integer");
    assert_eq!(add.input_schema["properties"]["a"]["description"], "First number.");
    assert!(add.input_schema.get("$schema").is_none());
}

#[tokio::test]
async fn call_dispatches_by_name() {
    let result = registry().call("add", json!({ "a": 2, "b": 3 })).await.unwrap();
    assert_eq!(result, "5");
}

#[tokio::test]
async fn invalid_input_is_reported() {
    let err = registry().call("add", json!({ "a": "two" })).await.unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput { ref tool, .. } if tool == "add"), "got {err:?}");
}

#[tokio::test]
async fn unknown_tool_is_reported() {
    let err = registry().call("nope", json!({})).await.unwrap_err();
    assert!(matches!(err, ToolError::Unknown(ref name) if name == "nope"));
}

#[test]
fn tool_without_input_still_has_properties() {
    let definitions = registry().definitions();
    let date = definitions.iter().find(|d| d.name == "current_date").unwrap();
    assert_eq!(date.input_schema["type"], "object");
    assert!(date.input_schema["properties"].is_object());
}

#[tokio::test]
async fn current_date_returns_todays_date() {
    let result = registry().call("current_date", json!({})).await.unwrap();
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    assert!(result.starts_with(&today), "got {result}");
}

