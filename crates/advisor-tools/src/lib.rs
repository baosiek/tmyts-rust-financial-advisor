//! Tools the agent can call: a typed `Tool` trait, a registry that dispatches
//! calls by name, and the tools themselves.

mod current_date;
mod tool;
mod tool_error;
mod tool_registry;

pub use current_date::CurrentDateTool;
pub use tool::Tool;
pub use tool_error::ToolError;
pub use tool_registry::ToolRegistry;
