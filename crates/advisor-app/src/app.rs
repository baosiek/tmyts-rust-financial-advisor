use std::sync::Arc;

use advisor_agent::{Agent, AgentConfig};
use advisor_llm::LlmProvider;
use advisor_llm::pricing::PriceTable;
use advisor_tools::ToolRegistry;
use thiserror::Error;

use crate::config::{ConfigError, Settings};
use crate::providers::{BuildProviderError, build_provider};
use crate::startup::{StartupError, check_services};
use crate::tools::build_tools;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Provider(#[from] BuildProviderError),

    #[error(transparent)]
    Startup(#[from] StartupError),
}

/// Everything built from the settings, ready to hand out agents.
///
/// The provider, the tools and their connection pools are shared (`Arc`), so a
/// new agent is cheap: it only gets its own empty conversation history. The
/// evals runner uses one fresh agent per case; the API will use one per session.
pub struct App {
    provider: Arc<dyn LlmProvider>,
    tools: Arc<ToolRegistry>,
    agent_config: AgentConfig,
    prices: Arc<PriceTable>,
}

impl App {
    /// Builds the provider and the enabled tools, then checks every service the
    /// tools depend on. Secrets are read through `get_key`.
    pub async fn build<F>(settings: &Settings, get_key: F) -> Result<Self, AppError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let provider = build_provider(&settings.llm, &get_key)?;
        let built = build_tools(settings, &get_key)?;
        check_services(
            &built.health_checks,
            settings.startup.health_check_timeout(),
        )
        .await?;

        let prices = settings.price_table();
        if prices.price(provider.model()).is_none() {
            // Not fatal: the agent works, but cost is reported as unknown.
            tracing::warn!(
                model = provider.model(),
                "no [pricing.models] entry for this model: cost will be reported as unknown"
            );
        }

        Ok(Self {
            provider,
            tools: Arc::new(built.registry),
            agent_config: AgentConfig {
                system_prompt: settings.agent.system_prompt.clone(),
                max_tokens: settings.agent.max_tokens,
                max_tool_rounds: settings.agent.max_tool_rounds,
            },
            prices: Arc::new(prices),
        })
    }

    /// A new agent with an empty conversation history.
    pub fn new_agent(&self) -> Agent {
        Agent::new(
            self.provider.clone(),
            self.tools.clone(),
            self.agent_config.clone(),
        )
    }

    pub fn prices(&self) -> &PriceTable {
        &self.prices
    }

    /// The configured model, e.g. for reports.
    pub fn model(&self) -> &str {
        self.provider.model()
    }

    pub fn provider_name(&self) -> &str {
        self.provider.name()
    }
}
