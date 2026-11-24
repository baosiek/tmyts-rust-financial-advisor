//! Prompt-injection defense for text that comes from third parties (news, web
//! pages) before it reaches the model. Adapted from the PromptWarden approach in
//! Sharda, "Rust for AI Engineers", ch. 7: pattern rules with weights, a combined
//! score, and a sanitizer, so suspicious text is cleaned or withheld.
//!
//! This is a filter, not a guarantee: it catches known attack shapes and makes
//! attacks visible in the logs. The model is also told to treat such text as
//! data, and tools are read-only, so a missed attack has limited reach.

mod category;
mod rules;
mod screening;
mod warden;
mod warden_config;

pub use category::Category;
pub use screening::{Action, Screening};
pub use warden::PromptWarden;
pub use warden_config::{WardenConfig, WardenConfigError};
