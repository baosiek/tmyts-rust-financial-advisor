//! The checks are plain functions of (case, outcome), so they are tested with
//! hand-made outcomes: no model, no network.

use std::path::Path;
use std::time::Duration;

use advisor_agent::{LlmCallRecord, RunTrace, ToolCallRecord};
use advisor_evals::{CaseOutcome, EvalCase, load_cases, parse_cases, run_checks};
use advisor_llm::pricing::{ModelPrice, PriceTable};
use advisor_llm::types::{StopReason, Usage};
use serde_json::json;

fn case(rules: &str) -> EvalCase {
    let text = format!("[[case]]\nid = \"t\"\nquestion = \"q\"\n{rules}\n");
    parse_cases(&text, Path::new("test.toml"))
        .unwrap()
        .remove(0)
}

fn outcome(answer: &str, tools: &[(&str, serde_json::Value, bool)]) -> CaseOutcome {
    CaseOutcome {
        answer: Some(answer.to_string()),
        error: None,
        trace: RunTrace {
            llm_calls: vec![LlmCallRecord {
                model: "m".to_string(),
                usage: Usage {
                    input_tokens: 10_000,
                    output_tokens: 1_000,
                    ..Usage::default()
                },
                stop_reason: StopReason::EndTurn,
            }],
            tool_calls: tools
                .iter()
                .map(|(name, input, is_error)| ToolCallRecord {
                    name: name.to_string(),
                    input: input.clone(),
                    is_error: *is_error,
                })
                .collect(),
            elapsed: Duration::from_millis(5),
        },
    }
}

fn prices() -> PriceTable {
    PriceTable::new(
        [(
            "m".to_string(),
            ModelPrice {
                input_per_mtok: 2.0,
                output_per_mtok: 10.0,
                cache_write_per_mtok: 2.5,
                cache_read_per_mtok: 0.1,
                long_context: None,
            },
        )]
        .into(),
    )
}

/// Names of the failed checks.
fn failed(case: &EvalCase, outcome: &CaseOutcome) -> Vec<String> {
    run_checks(case, outcome, &prices())
        .into_iter()
        .filter(|check| !check.passed)
        .map(|check| check.name)
        .collect()
}

#[test]
fn project_cases_file_is_valid() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals/cases.toml");
    let cases = load_cases(&path).unwrap();
    assert!(!cases.is_empty());
}

#[test]
fn good_answer_passes_every_rule() {
    let case = case(
        r#"expect_tools = ["price_history"]
           expect_tool_input = { price_history = { ticker = "AAPL" } }
           must_contain = ["aapl"]
           must_contain_any = ["%", "percent"]
           must_not_contain = ["you should buy"]
           must_cite_link = true
           max_llm_calls = 2
           max_cost_usd = 0.05"#,
    );
    let outcome = outcome(
        "AAPL rose 4.2% (https://example.com/a).",
        &[(
            "price_history",
            json!({ "ticker": "aapl", "days": 30 }),
            false,
        )],
    );
    assert_eq!(failed(&case, &outcome), Vec::<String>::new());
}

#[test]
fn missing_and_forbidden_tools_fail() {
    let case = case(
        r#"expect_tools = ["search_news"]
                       forbid_tools = ["price_history"]"#,
    );
    let outcome = outcome("text", &[("price_history", json!({}), false)]);
    assert_eq!(
        failed(&case, &outcome),
        vec!["calls search_news", "does not call price_history"]
    );
}

#[test]
fn wrong_tool_input_fails() {
    let case = case(r#"expect_tool_input = { price_history = { ticker = "AAPL" } }"#);
    let outcome = outcome(
        "text",
        &[("price_history", json!({ "ticker": "MSFT" }), false)],
    );
    assert_eq!(failed(&case, &outcome).len(), 1);
}

#[test]
fn tool_errors_fail_unless_allowed() {
    let tools = [("price_history", json!({}), true)];
    assert_eq!(
        failed(&case(""), &outcome("x", &tools)),
        vec!["no tool errors"]
    );
    assert!(failed(&case("allow_tool_errors = true"), &outcome("x", &tools)).is_empty());
}

#[test]
fn forbidden_phrase_fails_case_insensitively() {
    let case = case(r#"must_not_contain = ["you should buy"]"#);
    assert_eq!(failed(&case, &outcome("You Should Buy now!", &[])).len(), 1);
}

#[test]
fn answer_without_link_fails_must_cite_link() {
    let case = case("must_cite_link = true");
    assert_eq!(
        failed(&case, &outcome("no source", &[])),
        vec!["cites a link"]
    );
}

#[test]
fn budget_is_checked_and_unknown_cost_fails() {
    // 10,000 in at $2 + 1,000 out at $10 = $0.03.
    assert!(failed(&case("max_cost_usd = 0.05"), &outcome("x", &[])).is_empty());
    assert_eq!(
        failed(&case("max_cost_usd = 0.01"), &outcome("x", &[])).len(),
        1
    );

    let checks = run_checks(
        &case("max_cost_usd = 1.0"),
        &outcome("x", &[]),
        &PriceTable::default(),
    );
    let budget = checks.iter().find(|c| c.name.starts_with("costs")).unwrap();
    assert!(!budget.passed);
    assert!(budget.detail.contains("unknown"));
}

#[test]
fn agent_error_fails_answered() {
    let mut outcome = outcome("", &[]);
    outcome.answer = None;
    outcome.error = Some("too many tool rounds".to_string());
    assert_eq!(failed(&case(""), &outcome), vec!["answered"]);
}

#[test]
fn duplicate_ids_are_rejected() {
    let text = "[[case]]\nid = \"a\"\nquestion = \"q\"\n[[case]]\nid = \"a\"\nquestion = \"q\"\n";
    let err = parse_cases(text, Path::new("test.toml")).unwrap_err();
    assert!(err.to_string().contains("used twice"), "got: {err}");
}

#[test]
fn misspelled_rule_is_rejected() {
    let text = "[[case]]\nid = \"a\"\nquestion = \"q\"\nmust_contian = [\"x\"]\n";
    let err = parse_cases(text, Path::new("test.toml")).unwrap_err();
    assert!(err.to_string().contains("must_contian"), "got: {err}");
}

#[test]
fn typographic_apostrophes_match_plain_ones() {
    let any = case(r#"must_contain_any = ["couldn't find"]"#);
    assert!(failed(&any, &outcome("I couldn\u{2019}t find ZZZQX.", &[])).is_empty());

    let forbidden = case(r#"must_not_contain = ["you shouldn't sell"]"#);
    assert_eq!(
        failed(&forbidden, &outcome("You shouldn\u{2019}t sell.", &[])).len(),
        1
    );
}
