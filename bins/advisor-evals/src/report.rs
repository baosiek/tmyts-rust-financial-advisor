use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{CaseResult, EvalError};

/// One eval run: what was run against, the verdict on every case, and totals.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub started_at: DateTime<Utc>,
    pub provider: String,
    pub model: String,
    pub totals: Totals,
    pub cases: Vec<CaseResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Totals {
    pub cases: usize,
    pub passed: usize,
    pub failed: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// `None` if any case's cost is unknown.
    pub cost_usd: Option<f64>,
    pub elapsed_ms: u64,
}

impl Report {
    pub fn new(
        started_at: DateTime<Utc>,
        provider: impl Into<String>,
        model: impl Into<String>,
        cases: Vec<CaseResult>,
    ) -> Self {
        let passed = cases.iter().filter(|case| case.passed).count();
        let totals = Totals {
            cases: cases.len(),
            passed,
            failed: cases.len() - passed,
            input_tokens: cases.iter().map(|case| u64::from(case.input_tokens)).sum(),
            output_tokens: cases.iter().map(|case| u64::from(case.output_tokens)).sum(),
            cost_usd: cases.iter().map(|case| case.cost_usd).sum(),
            elapsed_ms: cases.iter().map(|case| case.elapsed_ms).sum(),
        };
        Self {
            started_at,
            provider: provider.into(),
            model: model.into(),
            totals,
            cases,
        }
    }

    pub fn all_passed(&self) -> bool {
        self.totals.failed == 0
    }

    /// Writes `<stem>.json` (for tools and for comparing runs) and `<stem>.md`
    /// (for people) into `dir`, and returns both paths.
    pub fn write(&self, dir: &Path) -> Result<(PathBuf, PathBuf), EvalError> {
        let write_error = |path: &Path| {
            let path = path.to_path_buf();
            move |source| EvalError::Write { path, source }
        };
        std::fs::create_dir_all(dir).map_err(write_error(dir))?;

        let stem = format!("eval-{}", self.started_at.format("%Y%m%d-%H%M%S"));
        let json_path = dir.join(format!("{stem}.json"));
        let md_path = dir.join(format!("{stem}.md"));

        let json = serde_json::to_string_pretty(self).expect("a report always serializes");
        std::fs::write(&json_path, json).map_err(write_error(&json_path))?;
        std::fs::write(&md_path, self.to_markdown()).map_err(write_error(&md_path))?;
        Ok((json_path, md_path))
    }

    /// A summary table, then the failed checks of every failed case.
    pub fn to_markdown(&self) -> String {
        let t = &self.totals;
        let mut md = String::new();
        // `write!` to a String cannot fail, hence the ignored results.
        let _ = writeln!(md, "# Eval report {}\n", self.started_at.to_rfc3339());
        let _ = writeln!(
            md,
            "Provider `{}`, model `{}`.\n",
            self.provider, self.model
        );
        let _ = writeln!(
            md,
            "**{} of {} passed** · {} input / {} output tokens · cost {} · {:.1} s\n",
            t.passed,
            t.cases,
            t.input_tokens,
            t.output_tokens,
            format_cost(t.cost_usd),
            t.elapsed_ms as f64 / 1000.0
        );
        let _ = writeln!(
            md,
            "| Case | Result | LLM calls | Tools | Tokens in/out | Cost | Time |"
        );
        let _ = writeln!(md, "|---|---|---|---|---|---|---|");
        for case in &self.cases {
            let _ = writeln!(
                md,
                "| {} | {} | {} | {} | {}/{} | {} | {:.1} s |",
                case.id,
                if case.passed { "✅ pass" } else { "❌ fail" },
                case.llm_calls,
                case.tools.join(", "),
                case.input_tokens,
                case.output_tokens,
                format_cost(case.cost_usd),
                case.elapsed_ms as f64 / 1000.0
            );
        }

        for case in self.cases.iter().filter(|case| !case.passed) {
            let _ = writeln!(md, "\n## ❌ {}\n\n> {}\n", case.id, case.question);
            for check in case.checks.iter().filter(|check| !check.passed) {
                let _ = writeln!(md, "- **{}**: {}", check.name, check.detail);
            }
            if let Some(answer) = &case.answer {
                let _ = writeln!(md, "\nAnswer:\n\n```text\n{answer}\n```");
            }
        }
        md
    }
}

pub(crate) fn format_cost(cost: Option<f64>) -> String {
    match cost {
        Some(usd) => format!("${usd:.4}"),
        None => "unknown".to_string(),
    }
}
