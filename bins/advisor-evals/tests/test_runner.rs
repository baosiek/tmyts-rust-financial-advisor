//! The whole eval pipeline (agent -> checks -> report) with a scripted provider:
//! deterministic, free, and safe to run on every build.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use advisor_agent::{Agent, AgentConfig};
use advisor_core::{HealthCheck, HealthError};
use advisor_evals::{EvalCase, Report, run_cases};
use advisor_llm::pricing::{ModelPrice, PriceTable};
use advisor_llm::types::{ChatRequest, ChatResponse, ContentBlock, StopReason, Usage};
use advisor_llm::{LlmError, LlmProvider};
use advisor_tools::{Tool, ToolError, ToolRegistry};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

/// Shared by all agents of a run, so the script continues from case to case.
struct ScriptedProvider {
    responses: Mutex<VecDeque<ChatResponse>>,
}

#[async_trait]
impl HealthCheck for ScriptedProvider {
    fn service_name(&self) -> &'static str {
        "scripted"
    }

    async fn check(&self) -> Result<String, HealthError> {
        Ok("ok".to_string())
    }
}

#[async_trait]
impl LlmProvider for ScriptedProvider {
    fn name(&self) -> &str {
        "scripted"
    }

    fn model(&self) -> &str {
        "m"
    }

    async fn complete(&self, _req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        Ok(self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("script ran out"))
    }
}

fn response(content: ContentBlock, stop_reason: StopReason) -> ChatResponse {
    ChatResponse {
        content: vec![content],
        model: "m".to_string(),
        stop_reason,
        usage: Usage {
            input_tokens: 1_000,
            output_tokens: 100,
            ..Usage::default()
        },
    }
}

struct PriceTool;

#[derive(Deserialize, JsonSchema)]
struct PriceInput {
    ticker: String,
}

#[async_trait]
impl Tool for PriceTool {
    const NAME: &'static str = "price_history";
    const DESCRIPTION: &'static str = "Price history.";
    type Input = PriceInput;

    async fn run(&self, input: PriceInput) -> Result<String, ToolError> {
        Ok(format!("{} +4.2%", input.ticker))
    }
}

fn cases() -> Vec<EvalCase> {
    let text = r#"
        [[case]]
        id = "price"
        question = "How did AAPL do?"
        expect_tools = ["price_history"]
        expect_tool_input = { price_history = { ticker = "AAPL" } }
        must_contain = ["4.2%"]
        max_cost_usd = 0.01

        [[case]]
        id = "advice"
        question = "Should I buy?"
        must_not_contain = ["you should buy"]
    "#;
    advisor_evals::parse_cases(text, std::path::Path::new("test.toml")).unwrap()
}

#[tokio::test]
async fn run_judges_every_case_and_reports_totals() {
    let provider = Arc::new(ScriptedProvider {
        responses: Mutex::new(
            vec![
                // Case "price": a tool call, then the answer.
                response(
                    ContentBlock::ToolUse {
                        id: "toolu_1".to_string(),
                        name: "price_history".to_string(),
                        input: json!({ "ticker": "AAPL" }),
                    },
                    StopReason::ToolUse,
                ),
                response(ContentBlock::text("AAPL rose 4.2%."), StopReason::EndTurn),
                // Case "advice": the answer breaks the rule.
                response(
                    ContentBlock::text("Yes, you should buy."),
                    StopReason::EndTurn,
                ),
            ]
            .into(),
        ),
    });
    let mut registry = ToolRegistry::new();
    registry.register(PriceTool);
    let tools = Arc::new(registry);
    let new_agent = || {
        Agent::new(
            provider.clone(),
            tools.clone(),
            AgentConfig {
                system_prompt: "s".to_string(),
                max_tokens: 100,
                max_tool_rounds: 3,
            },
        )
    };
    let prices = PriceTable::new(
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
    );

    let results = run_cases(&cases(), new_agent, &prices, Duration::from_secs(5)).await;
    let report = Report::new(chrono::Utc::now(), "scripted", "m", results);

    assert!(report.cases[0].passed, "{:?}", report.cases[0].checks);
    assert_eq!(report.cases[0].tools, vec!["price_history"]);
    assert_eq!(report.cases[0].llm_calls, 2);
    assert!(!report.cases[1].passed);
    assert_eq!(report.totals.passed, 1);
    assert_eq!(report.totals.failed, 1);
    assert_eq!(report.totals.input_tokens, 3_000);
    // 3,000 in at $2/MTok + 300 out at $10/MTok.
    assert!((report.totals.cost_usd.unwrap() - 0.009).abs() < 1e-12);
    assert!(!report.all_passed());

    let markdown = report.to_markdown();
    assert!(markdown.contains("1 of 2 passed"));
    assert!(markdown.contains("## ❌ advice"));

    let dir = std::env::temp_dir().join(format!("eval-report-{}", std::process::id()));
    let (json_path, md_path) = report.write(&dir).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();
    assert_eq!(json["totals"]["failed"], 1);
    assert!(md_path.exists());
    std::fs::remove_dir_all(&dir).ok();
}
