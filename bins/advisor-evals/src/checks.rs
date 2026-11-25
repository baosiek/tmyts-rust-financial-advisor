use advisor_llm::pricing::PriceTable;
use serde_json::Value;

use crate::{CaseOutcome, CheckResult, EvalCase};

/// Applies every rule the case lists to the outcome. Rules the case does not
/// list are not checked, except two that always apply: the agent must answer,
/// and tools must not fail unless the case allows it.
pub fn run_checks(case: &EvalCase, outcome: &CaseOutcome, prices: &PriceTable) -> Vec<CheckResult> {
    let mut checks = Vec::new();
    let answer = outcome.answer.as_deref().unwrap_or("");
    let answer_lower = answer.to_lowercase();
    let called = outcome.trace.tool_names();

    checks.push(match &outcome.error {
        None => CheckResult::new("answered", true, "the agent gave an answer"),
        Some(error) => CheckResult::new("answered", false, error.clone()),
    });

    for tool in &case.expect_tools {
        let passed = called.contains(&tool.as_str());
        checks.push(CheckResult::new(
            format!("calls {tool}"),
            passed,
            format!("tools called: {called:?}"),
        ));
    }

    for tool in &case.forbid_tools {
        let passed = !called.contains(&tool.as_str());
        checks.push(CheckResult::new(
            format!("does not call {tool}"),
            passed,
            format!("tools called: {called:?}"),
        ));
    }

    for (tool, expected) in &case.expect_tool_input {
        let inputs: Vec<&Value> = outcome
            .trace
            .tool_calls
            .iter()
            .filter(|call| &call.name == tool)
            .map(|call| &call.input)
            .collect();
        let passed = inputs.iter().any(|input| contains_fields(input, expected));
        checks.push(CheckResult::new(
            format!("{tool} input has {expected}"),
            passed,
            format!("{tool} inputs: {inputs:?}"),
        ));
    }

    if !case.allow_tool_errors {
        let failed: Vec<&str> = outcome
            .trace
            .tool_calls
            .iter()
            .filter(|call| call.is_error)
            .map(|call| call.name.as_str())
            .collect();
        checks.push(CheckResult::new(
            "no tool errors",
            failed.is_empty(),
            format!("failed tools: {failed:?}"),
        ));
    }

    for phrase in &case.must_contain {
        checks.push(CheckResult::new(
            format!("mentions \"{phrase}\""),
            answer_lower.contains(&phrase.to_lowercase()),
            "",
        ));
    }

    if !case.must_contain_any.is_empty() {
        let found: Vec<&String> = case
            .must_contain_any
            .iter()
            .filter(|phrase| answer_lower.contains(&phrase.to_lowercase()))
            .collect();
        checks.push(CheckResult::new(
            format!("mentions one of {:?}", case.must_contain_any),
            !found.is_empty(),
            format!("found: {found:?}"),
        ));
    }

    for phrase in &case.must_not_contain {
        checks.push(CheckResult::new(
            format!("does not say \"{phrase}\""),
            !answer_lower.contains(&phrase.to_lowercase()),
            "",
        ));
    }

    if case.must_cite_link {
        let passed = answer.contains("https://") || answer.contains("http://");
        checks.push(CheckResult::new("cites a link", passed, ""));
    }

    if let Some(max) = case.max_llm_calls {
        let calls = outcome.trace.llm_calls.len();
        checks.push(CheckResult::new(
            format!("at most {max} llm calls"),
            calls <= max,
            format!("{calls} calls"),
        ));
    }

    if let Some(max) = case.max_cost_usd {
        // Unknown cost cannot prove the budget was kept, so it fails.
        let check = match outcome.trace.cost_usd(prices) {
            Some(cost) => CheckResult::new(
                format!("costs at most ${max}"),
                cost <= max,
                format!("${cost:.6}"),
            ),
            None => CheckResult::new(
                format!("costs at most ${max}"),
                false,
                "cost unknown: a model has no configured price",
            ),
        };
        checks.push(check);
    }

    checks
}

/// True when `actual` (a tool input) has every field of `expected` with an equal
/// value. Text is compared case-insensitively, because "aapl" and "AAPL" are the
/// same request; nested tables are compared field by field.
fn contains_fields(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => expected.iter().all(|(key, want)| {
            actual
                .get(key)
                .is_some_and(|have| contains_fields(have, want))
        }),
        (Value::String(have), Value::String(want)) => have.eq_ignore_ascii_case(want),
        (Value::Number(have), Value::Number(want)) => have.as_f64() == want.as_f64(),
        (have, want) => have == want,
    }
}
