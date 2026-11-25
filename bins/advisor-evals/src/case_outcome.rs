use advisor_agent::RunTrace;

/// What the agent did with one case's question.
#[derive(Debug, Clone)]
pub struct CaseOutcome {
    /// The answer text, when the agent produced one.
    pub answer: Option<String>,
    /// Why there is no answer (agent error or timeout).
    pub error: Option<String>,
    pub trace: RunTrace,
}
