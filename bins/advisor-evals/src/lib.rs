//! Evals: questions with known-good behaviour, run against the agent, each
//! answer checked by rules (which tools, which inputs, what the answer must and
//! must not say, cost and call limits).
//!
//! The checks are deterministic code, so the same answer always gets the same
//! verdict. The answers are not: the model may phrase things differently on each
//! run, which is why the rules check behaviour and key facts, not exact wording.

mod case_outcome;
mod case_result;
mod checks;
mod eval_case;
mod eval_error;
mod report;
mod runner;

pub use case_outcome::CaseOutcome;
pub use case_result::{CaseResult, CheckResult};
pub use checks::run_checks;
pub use eval_case::{EvalCase, load_cases, parse_cases};
pub use eval_error::EvalError;
pub use report::{Report, Totals};
pub use runner::{run_case, run_cases};
