use async_trait::async_trait;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::Value;

use advisor_llm::types::ToolDefinition;

use crate::ToolError;

/// A tool with a typed input. You implement this trait; the registry handles
/// turning the model's JSON into `Input` and generating the JSON schema.
#[async_trait]
pub trait Tool: Send + Sync + 'static {
    /// The name the model uses to call the tool, e.g. "current_date".
    const NAME: &'static str;
    /// What the tool does. The model reads this to decide when to call it.
    const DESCRIPTION: &'static str;
    /// The tool's input. Its JSON schema is generated from the struct by `schemars`.
    type Input: DeserializeOwned + JsonSchema + Send;

    async fn run(&self, input: Self::Input) -> Result<String, ToolError>;
}

/// Object-safe version of `Tool`, so tools with different `Input` types can be
/// stored together as `Box<dyn DynTool>`. Implemented for every `Tool` below;
/// you never implement it yourself.
#[async_trait]
pub(crate) trait DynTool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    async fn call(&self, input: Value) -> Result<String, ToolError>;
}

#[async_trait]
impl<T: Tool> DynTool for T {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: T::NAME.to_string(),
            description: T::DESCRIPTION.to_string(),
            input_schema: input_schema::<T::Input>(),
        }
    }

    async fn call(&self, input: Value) -> Result<String, ToolError> {
        let input: T::Input =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput {
                tool: T::NAME.to_string(),
                reason: e.to_string(),
            })?;
        self.run(input).await
    }
}

/// JSON schema of `I`, without the meta keys the model does not need.
fn input_schema<I: JsonSchema>() -> Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(I))
        .expect("a generated JSON schema is always valid JSON");
    if let Some(object) = schema.as_object_mut() {
        object.remove("$schema");
        object.remove("title");
        // A struct without fields produces no "properties"; the API expects the key.
        object
            .entry("properties")
            .or_insert_with(|| Value::Object(Default::default()));
    }
    schema
}
