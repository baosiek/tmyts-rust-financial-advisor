//! The agent is tested with a mock provider injected in place of a real one.

use std::sync::{Arc, Mutex};

use advisor_agent::Agent;
use advisor_llm::types::{ChatRequest, ChatResponse, Role, Usage};
use advisor_llm::{LlmError, LlmProvider};

/// Answers with a fixed text, or fails, and records every request it receives.
struct MockProvider {
    fail: bool,
    requests: Mutex<Vec<ChatRequest>>,
}

impl MockProvider {
    fn new(fail: bool) -> Arc<Self> {
        Arc::new(Self {
            fail,
            requests: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait::async_trait]
impl LlmProvider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn model(&self) -> &str {
        "mock-model"
    }

    async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        self.requests.lock().unwrap().push(req.clone());
        if self.fail {
            return Err(LlmError::Network("connection refused".into()));
        }
        Ok(ChatResponse {
            text: "Hello!".to_string(),
            model: "mock-model".to_string(),
            stop_reason: Some("end_turn".to_string()),
            usage: Usage::default(),
        })
    }
}

#[tokio::test]
async fn answer_is_returned_and_remembered() {
    let mock = MockProvider::new(false);
    let mut agent = Agent::new(mock.clone(), "system", 100);

    let answer = agent.ask("Hi").await.unwrap();

    assert_eq!(answer, "Hello!");
    let roles: Vec<Role> = agent.history().iter().map(|m| m.role).collect();
    assert_eq!(roles, vec![Role::User, Role::Assistant]);
}

#[tokio::test]
async fn second_question_carries_the_whole_conversation() {
    let mock = MockProvider::new(false);
    let mut agent = Agent::new(mock.clone(), "system", 100);

    agent.ask("Hi").await.unwrap();
    agent.ask("And now?").await.unwrap();

    let requests = mock.requests.lock().unwrap();
    assert_eq!(requests[1].messages.len(), 3); // user, assistant, user
    assert_eq!(requests[1].system.as_deref(), Some("system"));
}

#[tokio::test]
async fn failed_question_is_removed_from_history() {
    let mock = MockProvider::new(true);
    let mut agent = Agent::new(mock, "system", 100);

    assert!(agent.ask("Hi").await.is_err());
    assert!(agent.history().is_empty());
}
