/// The agent's settings. Built by the composition root from the settings file.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub system_prompt: String,
    pub max_tokens: u32,
    /// How many model -> tools -> model rounds one question may take before the
    /// agent gives up. Protects against loops and runaway cost.
    pub max_tool_rounds: u32,
}
