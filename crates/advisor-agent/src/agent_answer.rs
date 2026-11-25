pub use crate::RunTrace;

/// The agent's answer to one question, with the record of how it got there.
#[derive(Debug, Clone)]
pub struct AgentAnswer {
    /// The text a person reads.
    pub text: String,
    pub trace: RunTrace,
}
