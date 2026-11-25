use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::EvalError;

/// The cases file: a list of `[[case]]` tables.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvalFile {
    case: Vec<EvalCase>,
}

/// One question and the rules its answer must follow. Every rule is optional:
/// a case only checks what it lists.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalCase {
    /// Short unique name, used in reports and with `--only`.
    pub id: String,
    pub question: String,

    /// Tools that must be called (each at least once).
    #[serde(default)]
    pub expect_tools: Vec<String>,
    /// Tools that must not be called.
    #[serde(default)]
    pub forbid_tools: Vec<String>,
    /// Per tool: fields its input must contain, e.g. `price_history = { ticker = "AAPL" }`.
    /// At least one call of that tool must match. Text compares case-insensitively.
    #[serde(default)]
    pub expect_tool_input: BTreeMap<String, Value>,
    /// Tool errors are failures unless this is true (e.g. a case about an unknown ticker).
    #[serde(default)]
    pub allow_tool_errors: bool,

    /// Phrases the answer must all contain (case-insensitive).
    #[serde(default)]
    pub must_contain: Vec<String>,
    /// Phrases of which the answer must contain at least one.
    #[serde(default)]
    pub must_contain_any: Vec<String>,
    /// Phrases the answer must not contain.
    #[serde(default)]
    pub must_not_contain: Vec<String>,
    /// The answer must contain a link (news answers must cite their source).
    #[serde(default)]
    pub must_cite_link: bool,

    /// Most LLM calls the question may take.
    pub max_llm_calls: Option<usize>,
    /// Most the question may cost, in US dollars.
    pub max_cost_usd: Option<f64>,
}

/// Reads and validates the cases file.
pub fn load_cases(path: &Path) -> Result<Vec<EvalCase>, EvalError> {
    let text = std::fs::read_to_string(path).map_err(|source| EvalError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse_cases(&text, path)
}

/// Parses and validates cases from text; `path` is only used in error messages.
pub fn parse_cases(text: &str, path: &Path) -> Result<Vec<EvalCase>, EvalError> {
    let file: EvalFile = toml::from_str(text).map_err(|source| EvalError::Parse {
        path: path.to_path_buf(),
        source,
    })?;

    let invalid = |reason: String| EvalError::Invalid {
        path: path.to_path_buf(),
        reason,
    };
    let mut ids = HashSet::new();
    for case in &file.case {
        if case.question.trim().is_empty() {
            return Err(invalid(format!("case `{}` has an empty question", case.id)));
        }
        if !ids.insert(case.id.as_str()) {
            return Err(invalid(format!("case id `{}` is used twice", case.id)));
        }
        for (tool, fields) in &case.expect_tool_input {
            if !fields.is_object() {
                return Err(invalid(format!(
                    "case `{}`: expect_tool_input.{tool} must be a table of fields",
                    case.id
                )));
            }
        }
    }
    Ok(file.case)
}
