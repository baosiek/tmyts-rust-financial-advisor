use std::time::Duration;

use advisor_agent::{Agent, RunTrace};
use advisor_core::{TimedError, timed};
use advisor_llm::pricing::PriceTable;

use crate::{CaseOutcome, CaseResult, EvalCase, run_checks};

/// Runs one case on `agent` (which should be fresh: an empty history, so earlier
/// cases cannot influence this one) and judges the outcome.
pub async fn run_case(
    agent: &mut Agent,
    case: &EvalCase,
    prices: &PriceTable,
    limit: Duration,
) -> CaseResult {
    let pending = timed("eval case", limit, agent.ask(case.question.clone())).await;

    let outcome = match pending.result {
        Ok(answer) => CaseOutcome {
            answer: Some(answer.text),
            error: None,
            trace: answer.trace,
        },
        Err(TimedError::Failed(e)) => CaseOutcome {
            answer: None,
            error: Some(e.to_string()),
            trace: agent.last_trace().clone(),
        },
        // The question was cancelled mid-flight, so its trace was never stored:
        // tokens spent before the timeout are missing from this case's numbers.
        Err(e @ TimedError::TimedOut(_)) => CaseOutcome {
            answer: None,
            error: Some(e.to_string()),
            trace: RunTrace {
                elapsed: pending.elapsed,
                ..RunTrace::default()
            },
        },
    };

    let checks = run_checks(case, &outcome, prices);
    let passed = checks.iter().all(|check| check.passed);
    let usage = outcome.trace.usage();

    CaseResult {
        id: case.id.clone(),
        question: case.question.clone(),
        passed,
        checks,
        answer: outcome.answer,
        error: outcome.error,
        llm_calls: outcome.trace.llm_calls.len(),
        tools: outcome
            .trace
            .tool_names()
            .into_iter()
            .map(String::from)
            .collect(),
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_read_tokens: usage.cache_read_input_tokens,
        cost_usd: outcome.trace.cost_usd(prices),
        elapsed_ms: u64::try_from(outcome.trace.elapsed.as_millis()).unwrap_or(u64::MAX),
    }
}

/// Runs the cases one after the other, each on a new agent from `new_agent`.
///
/// Sequential on purpose: results stay readable in the log, and a run does not
/// hit the provider's rate limits. Parallel runs can come later if the suite grows.
pub async fn run_cases<F>(
    cases: &[EvalCase],
    new_agent: F,
    prices: &PriceTable,
    limit: Duration,
) -> Vec<CaseResult>
where
    F: Fn() -> Agent,
{
    let mut results = Vec::with_capacity(cases.len());
    for (n, case) in cases.iter().enumerate() {
        tracing::info!(case = %case.id, "running case {} of {}", n + 1, cases.len());
        let mut agent = new_agent();
        let result = run_case(&mut agent, case, prices, limit).await;
        if result.passed {
            tracing::info!(case = %case.id, cost_usd = ?result.cost_usd, "PASS");
        } else {
            let failed: Vec<&str> = result
                .checks
                .iter()
                .filter(|check| !check.passed)
                .map(|check| check.name.as_str())
                .collect();
            tracing::warn!(case = %case.id, ?failed, "FAIL");
        }
        results.push(result);
    }
    results
}
