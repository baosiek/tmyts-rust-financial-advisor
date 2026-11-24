//! The agent is tested with a scripted mock provider injected in place of a real one.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use advisor_agent::{Agent, AgentConfig, AgentError};
use advisor_llm::types::{ChatRequest, ChatResponse, ContentBlock, Role, StopReason, Usage};
use advisor_llm::{LlmError, LlmProvider};
use advisor_tools::{Tool, ToolError, ToolRegistry};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

/// Returns the scripted responses in order and records every request it receives.
struct ScriptedProvider {
    responses: Mutex<VecDeque<Result<ChatResponse, LlmError>>>,
    requests: Mutex<Vec<ChatRequest>>,
}

impl ScriptedProvider {
    fn new(responses: Vec<Result<ChatResponse, LlmError>>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(responses.into()),
            requests: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait]
impl LlmProvider for ScriptedProvider {
    fn name(&self) -> &str {
        "scripted"
    }

    fn model(&self) -> &str {
        "scripted-model"
    }

    async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        self.requests.lock().unwrap().push(req.clone());
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("the test scripted fewer responses than the agent asked for")
    }
}

fn answer(text: &str) -> Result<ChatResponse, LlmError> {
    Ok(ChatResponse {
        content: vec![ContentBlock::text(text)],
        model: "scripted-model".to_string(),
        stop_reason: StopReason::EndTurn,
        usage: Usage::default(),
    })
}

fn tool_call(id: &str, name: &str, input: serde_json::Value) -> Result<ChatResponse, LlmError> {
    Ok(ChatResponse {
        content: vec![ContentBlock::ToolUse {
            id: id.to_string(),
            name: name.to_string(),
            input,
        }],
        model: "scripted-model".to_string(),
        stop_reason: StopReason::ToolUse,
        usage: Usage::default(),
    })
}

/// A test tool with a fixed answer.
struct PriceTool;

#[derive(Deserialize, JsonSchema)]
struct PriceInput {
    ticker: String,
}

#[async_trait]
impl Tool for PriceTool {
    const NAME: &'static str = "price";
    const DESCRIPTION: &'static str = "Latest price of a ticker.";
    type Input = PriceInput;

    async fn run(&self, input: PriceInput) -> Result<String, ToolError> {
        Ok(format!("{}: 123.45", input.ticker))
    }
}

fn agent(provider: Arc<ScriptedProvider>, max_tool_rounds: u32) -> Agent {
    let mut tools = ToolRegistry::new();
    tools.register(PriceTool);
    Agent::new(
        provider,
        Arc::new(tools),
        AgentConfig {
            system_prompt: "system".to_string(),
            max_tokens: 100,
            max_tool_rounds,
        },
    )
}

#[tokio::test]
async fn plain_answer_needs_one_call() {
    let provider = ScriptedProvider::new(vec![answer("Hello!")]);
    let mut agent = agent(provider.clone(), 5);

    assert_eq!(agent.ask("Hi").await.unwrap(), "Hello!");

    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].tools.len(), 1, "tool definitions are sent");
    assert_eq!(requests[0].system.as_deref(), Some("system"));
}

#[tokio::test]
async fn tool_call_is_run_and_result_sent_back() {
    let provider = ScriptedProvider::new(vec![
        tool_call("toolu_1", "price", json!({ "ticker": "AAPL" })),
        answer("AAPL trades at 123.45."),
    ]);
    let mut agent = agent(provider.clone(), 5);

    assert_eq!(
        agent.ask("Price of AAPL?").await.unwrap(),
        "AAPL trades at 123.45."
    );

    // Second request: question, the assistant's tool call, then the tool result.
    let requests = provider.requests.lock().unwrap();
    let messages = &requests[1].messages;
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[2].role, Role::User);
    assert_eq!(
        messages[2].content[0],
        ContentBlock::ToolResult {
            tool_use_id: "toolu_1".to_string(),
            content: "AAPL: 123.45".to_string(),
            is_error: false,
        }
    );

    // History: user, assistant (tool call), user (tool result), assistant (answer).
    let roles: Vec<Role> = agent.history().iter().map(|m| m.role).collect();
    assert_eq!(
        roles,
        vec![Role::User, Role::Assistant, Role::User, Role::Assistant]
    );
}

#[tokio::test]
async fn failing_tool_is_reported_to_the_model() {
    let provider = ScriptedProvider::new(vec![
        tool_call("toolu_1", "price", json!({ "wrong": "field" })),
        answer("Sorry, I could not get the price."),
    ]);
    let mut agent = agent(provider.clone(), 5);

    agent.ask("Price?").await.unwrap();

    let requests = provider.requests.lock().unwrap();
    assert!(matches!(
        &requests[1].messages[2].content[0],
        ContentBlock::ToolResult { is_error: true, .. }
    ));
}

#[tokio::test]
async fn endless_tool_calls_stop_at_the_limit() {
    let provider = ScriptedProvider::new(vec![
        tool_call("toolu_1", "price", json!({ "ticker": "A" })),
        tool_call("toolu_2", "price", json!({ "ticker": "B" })),
    ]);
    let mut agent = agent(provider, 2);

    let err = agent.ask("Loop").await.unwrap_err();
    assert!(matches!(err, AgentError::TooManyToolRounds(2)));
    assert!(agent.history().is_empty(), "history is rolled back");
}

#[tokio::test]
async fn provider_error_rolls_back_the_history() {
    let provider = ScriptedProvider::new(vec![Err(LlmError::Network("down".into()))]);
    let mut agent = agent(provider, 5);

    let err = agent.ask("Hi").await.unwrap_err();
    assert!(matches!(err, AgentError::Llm(LlmError::Network(_))));
    assert!(agent.history().is_empty());
}

#[tokio::test]
async fn cut_off_answer_is_an_error_not_an_empty_answer() {
    let provider = ScriptedProvider::new(vec![Ok(ChatResponse {
        content: vec![],
        model: "scripted-model".to_string(),
        stop_reason: StopReason::MaxTokens,
        usage: Usage::default(),
    })]);
    let mut agent = agent(provider, 5);

    let err = agent.ask("Long question").await.unwrap_err();
    assert!(
        matches!(err, AgentError::MaxTokensReached { max_tokens: 100 }),
        "got {err:?}"
    );
    assert!(agent.history().is_empty(), "history is rolled back");
}
