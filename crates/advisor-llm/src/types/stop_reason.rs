/// Why the model stopped writing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// The answer is complete.
    EndTurn,
    /// The model wants one or more tools to be run before it continues.
    ToolUse,
    /// The answer was cut off by `max_tokens`.
    MaxTokens,
    /// A configured stop sequence was reached.
    StopSequence,
    /// A reason this code does not know yet; kept so nothing is lost.
    Other(String),
}
