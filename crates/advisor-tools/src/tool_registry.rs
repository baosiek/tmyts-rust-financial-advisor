use std::collections::BTreeMap;

use serde_json::Value;

use advisor_llm::types::ToolDefinition;

use crate::tool::DynTool;
use crate::{Tool, ToolError};

/// Holds the tools the agent may use and dispatches calls to them by name.
#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, Box<dyn DynTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a tool. A later tool with the same name replaces the earlier one.
    pub fn register<T: Tool>(&mut self, tool: T) {
        self.tools.insert(T::NAME.to_string(), Box::new(tool));
    }

    /// Definitions of all registered tools, sorted by name, to send to the model.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|tool| tool.definition()).collect()
    }

    /// Runs the tool called `name` with the model's JSON `input`.
    pub async fn call(&self, name: &str, input: Value) -> Result<String, ToolError> {
        match self.tools.get(name) {
            Some(tool) => tool.call(input).await,
            None => Err(ToolError::Unknown(name.to_string())),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }
}
