use async_trait::async_trait;
use chrono::Utc;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{Tool, ToolError};

/// Gives the model today's date. Trivial on purpose: it exercises the whole
/// tool-calling loop without needing a database.
pub struct CurrentDateTool;

/// No input needed. (Doc comments on input structs and fields become the
/// "description" entries of the generated JSON schema, which the model reads.)
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CurrentDateInput {}

#[async_trait]
impl Tool for CurrentDateTool {
    const NAME: &'static str = "current_date";
    const DESCRIPTION: &'static str = "Returns today's date and the current time in UTC. \
        Use it whenever the answer depends on today's date.";
    type Input = CurrentDateInput;

    async fn run(&self, _input: CurrentDateInput) -> Result<String, ToolError> {
        Ok(Utc::now().format("%Y-%m-%d (%A), %H:%M UTC").to_string())
    }
}
