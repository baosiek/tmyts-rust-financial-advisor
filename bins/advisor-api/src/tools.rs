//! Composition root for tools: the only place that knows the concrete tool types.
//! Which tools the agent gets is decided by `[tools] enabled` in the settings file.

use advisor_tools::{CurrentDateTool, ToolRegistry};

use crate::config::{ToolKind, ToolsSettings};

pub fn build_tools(settings: &ToolsSettings) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    for tool in &settings.enabled {
        match tool {
            ToolKind::CurrentDate => registry.register(CurrentDateTool),
        }
    }
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_enabled_tools_are_registered() {
        let none = build_tools(&ToolsSettings { enabled: vec![] });
        assert!(none.is_empty());

        let date = build_tools(&ToolsSettings {
            enabled: vec![ToolKind::CurrentDate],
        });
        let names: Vec<String> = date.definitions().into_iter().map(|d| d.name).collect();
        assert_eq!(names, vec!["current_date"]);
    }
}
