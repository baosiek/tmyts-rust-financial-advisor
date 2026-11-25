use serde::Serialize;

/// The verdict on one case, as written to the report.
#[derive(Debug, Clone, Serialize)]
pub struct CaseResult {
    pub id: String,
    pub question: String,
    pub passed: bool,
    pub checks: Vec<CheckResult>,
    pub answer: Option<String>,
    pub error: Option<String>,
    pub llm_calls: usize,
    pub tools: Vec<String>,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    /// `None` when a model had no configured price.
    pub cost_usd: Option<f64>,
    pub elapsed_ms: u64,
}

/// One rule of a case and whether the answer followed it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckResult {
    pub name: String,
    pub passed: bool,
    /// What was found, so a failure can be understood without rerunning.
    pub detail: String,
}

impl CheckResult {
    pub fn new(name: impl Into<String>, passed: bool, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed,
            detail: detail.into(),
        }
    }
}
