//! Reads configuration: secrets from environment variables (and `.env`),
//! every other setting from the TOML file named by `ADVISOR_CONFIG`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use advisor_llm::RetryPolicy;
use advisor_llm::pricing::{ModelPrice, PriceTable};
use serde::Deserialize;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse .env file: {0}")]
    DotEnv(#[from] dotenvy::Error),

    #[error("missing required environment variable: {0}")]
    Missing(&'static str),

    #[error("environment variable `{0}` is set but empty")]
    Empty(&'static str),

    #[error("environment variable `{name}` has invalid value: {reason}")]
    Invalid { name: &'static str, reason: String },

    #[error("cannot read settings file {path}: {source}")]
    ReadSettings {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid settings file {path}: {source}")]
    ParseSettings {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error(
        "provider `{provider}` is selected but the settings file has no [llm.{provider}] section"
    )]
    MissingProviderSettings { provider: &'static str },

    #[error("could not create the {service} client: {reason}")]
    Service {
        service: &'static str,
        reason: String,
    },

    #[error("invalid setting `{setting}`: {reason}")]
    InvalidSetting {
        setting: &'static str,
        reason: String,
    },

    #[error("`{needed_by}` needs a [{section}] section in the settings file")]
    MissingSection {
        section: &'static str,
        needed_by: &'static str,
    },
}

/// Non-secret settings, read from the TOML file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub runtime: RuntimeSettings,
    pub logging: LoggingSettings,
    pub llm: LlmSettings,
    pub agent: AgentSettings,
    pub tools: ToolsSettings,
    pub startup: StartupSettings,
    /// Only required when a tool that reads the database is enabled.
    pub postgres: Option<PostgresSettings>,
    /// The next three are only required when `search_news` is enabled.
    pub embeddings: Option<EmbeddingsSettings>,
    pub qdrant: Option<QdrantSettings>,
    pub news_search: Option<NewsSearchSettings>,
    /// Prompt-injection filter for third-party text (news). Required with `search_news`.
    pub warden: Option<WardenSettings>,
    /// Prices per model, for cost tracking. Without it, cost is reported as unknown.
    pub pricing: Option<PricingSettings>,
    /// Settings of the evals runner (`bins/advisor-evals`). Only it needs them.
    pub evals: Option<EvalsSettings>,
}

/// Token prices, one table per model id: `[pricing.models."<model id>"]`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricingSettings {
    pub models: BTreeMap<String, ModelPrice>,
}

/// Where the eval cases are and where reports go. Relative paths are resolved
/// from the directory the runner is started in (the project root with `cargo run`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalsSettings {
    pub cases_file: PathBuf,
    pub report_dir: PathBuf,
    /// Upper bound for one whole case: every LLM and tool call of the question.
    pub case_timeout_secs: u64,
}

impl EvalsSettings {
    pub fn case_timeout(&self) -> Duration {
        Duration::from_secs(self.case_timeout_secs)
    }
}

/// Score thresholds of the prompt-injection filter (0.0 to 1.0).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WardenSettings {
    /// From this score on, suspicious parts are removed and the result is flagged.
    pub sanitize_at: f64,
    /// From this score on, the article is withheld from the model.
    pub block_at: f64,
}

/// The embedding service that embedded the stored news. Must be the same one,
/// or searches compare vectors from different models.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddingsSettings {
    pub url: Url,
    pub timeout_secs: u64,
    /// Size of the vectors; must equal the Qdrant collection's vector size.
    pub vector_size: usize,
}

impl EmbeddingsSettings {
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }
}

/// The Qdrant news collection. `QDRANT_API_KEY` (optional) comes from the environment.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QdrantSettings {
    /// gRPC endpoint, usually port 6334.
    pub url: Url,
    pub collection: String,
    pub timeout_secs: u64,
}

impl QdrantSettings {
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }
}

/// Limits of the `search_news` tool.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewsSearchSettings {
    pub default_results: u32,
    pub max_results: u32,
    pub max_excerpt_chars: usize,
}

/// Checks run before the agent starts.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartupSettings {
    /// How long each service (e.g. Postgres) may take to answer its health check.
    pub health_check_timeout_secs: u64,
}

impl StartupSettings {
    pub fn health_check_timeout(&self) -> Duration {
        Duration::from_secs(self.health_check_timeout_secs)
    }
}

/// Connection to the trading database. The password comes from `POSTGRES_PASSWORD`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostgresSettings {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub max_connections: u32,
    pub acquire_timeout_secs: u64,
}

impl PostgresSettings {
    pub fn acquire_timeout(&self) -> Duration {
        Duration::from_secs(self.acquire_timeout_secs)
    }
}

/// Tokio runtime settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSettings {
    pub worker_threads: usize,
}

/// Logging settings. `RUST_LOG`, when set, overrides `filter`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoggingSettings {
    pub filter: String,
}

/// Which LLM provider to use, plus the settings of every configured provider.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmSettings {
    pub provider: ProviderKind,
    /// Upper bound for one whole LLM call, measured by `TimedProvider`.
    pub call_timeout_secs: u64,
    /// Retries of failed calls; applies to whichever provider is selected.
    pub retry: RetrySettings,
    pub anthropic: Option<AnthropicSettings>,
    pub openai: Option<OpenaiSettings>,
}

impl LlmSettings {
    pub fn call_timeout(&self) -> Duration {
        Duration::from_secs(self.call_timeout_secs)
    }
}

/// `[llm.retry]`: exponential backoff for temporary failures.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetrySettings {
    /// Tries in total, the first one included. 1 switches retries off.
    pub max_attempts: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub multiplier: f64,
    /// Share of each wait that is random, 0.0 to 1.0.
    pub jitter: f64,
}

impl RetrySettings {
    /// The validated policy; invalid numbers are reported with the setting's name.
    pub fn policy(&self) -> Result<RetryPolicy, ConfigError> {
        RetryPolicy::new(
            self.max_attempts,
            Duration::from_millis(self.initial_backoff_ms),
            Duration::from_millis(self.max_backoff_ms),
            self.multiplier,
            self.jitter,
        )
        .map_err(|e| ConfigError::InvalidSetting {
            setting: "llm.retry",
            reason: e.to_string(),
        })
    }
}

/// The providers this build knows about. Add a variant when a new provider is implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Anthropic,
    Openai,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnthropicSettings {
    pub url: Url,
    pub models_url: Url,
    pub version: String,
    pub model: String,
    pub timeout_secs: u64,
}

impl AnthropicSettings {
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenaiSettings {
    /// The Responses API endpoint. Any OpenAI-compatible Responses API works here.
    pub responses_url: Url,
    /// Model list endpoint; the health check asks for `{models_url}/{model}`.
    pub models_url: Url,
    pub model: String,
    pub timeout_secs: u64,
    /// Send reasoning items back in the next request (see `[llm.openai]` in the settings file).
    pub replay_reasoning: bool,
}

impl OpenaiSettings {
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSettings {
    pub max_tokens: u32,
    pub max_tool_rounds: u32,
    pub system_prompt: String,
}

/// Which tools the agent may use. Unknown names are rejected when the file is read.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolsSettings {
    pub enabled: Vec<ToolKind>,
}

/// The tools this build knows about. Add a variant when a new tool is implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    CurrentDate,
    PriceHistory,
    SearchNews,
}

impl Settings {
    /// The configured prices; empty when there is no `[pricing]` section.
    pub fn price_table(&self) -> PriceTable {
        self.pricing
            .as_ref()
            .map(|pricing| PriceTable::new(pricing.models.clone()))
            .unwrap_or_default()
    }

    /// Loads `.env` (if present) into the process environment, then reads the
    /// settings file named by `ADVISOR_CONFIG`. Real environment variables take
    /// precedence over `.env` values.
    pub fn load() -> Result<Self, ConfigError> {
        // A missing .env is fine (e.g. in K3d); a malformed one is an error.
        if let Err(e) = dotenvy::dotenv()
            && !e.not_found()
        {
            return Err(e.into());
        }
        let get_key = |key: &str| std::env::var(key).ok();
        let path = PathBuf::from(required(&get_key, "ADVISOR_CONFIG")?);
        Self::from_file(&path)
    }

    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::ReadSettings {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_toml_str(&text, path)
    }

    pub fn from_toml_str(text: &str, path: &Path) -> Result<Self, ConfigError> {
        toml::from_str(text).map_err(|source| ConfigError::ParseSettings {
            path: path.to_path_buf(),
            source,
        })
    }
}

/// Reads a required variable through `get_key`, so tests can pass values
/// without touching the real environment.
pub fn required<F>(get_key: &F, name: &'static str) -> Result<String, ConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    match get_key(name) {
        None => Err(ConfigError::Missing(name)),
        Some(value) if value.trim().is_empty() => Err(ConfigError::Empty(name)),
        Some(value) => Ok(value.trim().to_string()),
    }
}

/// Reads an optional variable: `None` when it is missing or empty.
pub fn optional<F>(get_key: &F, name: &str) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    get_key(name)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
        [runtime]
        worker_threads = 4

        [logging]
        filter = "info"

        [llm]
        provider = "anthropic"
        call_timeout_secs = 180

        [llm.retry]
        max_attempts = 3
        initial_backoff_ms = 500
        max_backoff_ms = 8000
        multiplier = 2.0
        jitter = 0.5

        [llm.anthropic]
        url = "https://api.anthropic.com/v1/messages"
        models_url = "https://api.anthropic.com/v1/models"
        version = "2023-06-01"
        model = "m"
        timeout_secs = 120

        [agent]
        max_tokens = 1024
        max_tool_rounds = 5
        system_prompt = "p"

        [tools]
        enabled = ["current_date"]

        [startup]
        health_check_timeout_secs = 10
    "#;

    fn parse(text: &str) -> Result<Settings, ConfigError> {
        Settings::from_toml_str(text, Path::new("test.toml"))
    }

    #[test]
    fn project_settings_file_is_valid() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/advisor.toml");
        let settings = Settings::from_file(&path).unwrap();
        assert!(settings.agent.max_tokens > 0);
    }

    #[test]
    fn valid_settings_parse() {
        let settings = parse(VALID).unwrap();
        assert_eq!(settings.llm.provider, ProviderKind::Anthropic);
        assert_eq!(
            settings.llm.anthropic.unwrap().timeout(),
            Duration::from_secs(120)
        );
    }

    #[test]
    fn openai_section_parses() {
        let text = format!(
            "{VALID}\n[llm.openai]\nresponses_url = \"https://api.openai.com/v1/responses\"\n\
             models_url = \"https://api.openai.com/v1/models\"\nmodel = \"m\"\n\
             timeout_secs = 60\nreplay_reasoning = true\n"
        )
        .replace(r#"provider = "anthropic""#, r#"provider = "openai""#);
        let settings = parse(&text).unwrap();
        assert_eq!(settings.llm.provider, ProviderKind::Openai);
        let openai = settings.llm.openai.unwrap();
        assert!(openai.replay_reasoning);
        assert_eq!(openai.timeout(), Duration::from_secs(60));
    }

    #[test]
    fn retry_settings_become_a_policy() {
        let settings = parse(VALID).unwrap();
        let policy = settings.llm.retry.policy().unwrap();
        assert_eq!(policy.max_attempts(), 3);
        assert_eq!(policy.backoff(2), Duration::from_millis(1000));

        let text = VALID.replace("max_attempts = 3", "max_attempts = 0");
        let err = parse(&text).unwrap().llm.retry.policy().unwrap_err();
        assert!(err.to_string().contains("llm.retry"), "got: {err}");
    }

    #[test]
    fn unknown_provider_is_rejected() {
        let text = VALID.replace(r#"provider = "anthropic""#, r#"provider = "gemini""#);
        let err = parse(&text).unwrap_err();
        assert!(err.to_string().contains("gemini"), "got: {err}");
    }

    #[test]
    fn enabled_tools_parse() {
        let settings = parse(VALID).unwrap();
        assert_eq!(settings.tools.enabled, vec![ToolKind::CurrentDate]);
    }

    #[test]
    fn postgres_section_is_optional() {
        let settings = parse(VALID).unwrap();
        assert!(settings.postgres.is_none());

        let text = format!(
            "{VALID}\n[postgres]\nhost = \"localhost\"\nport = 5432\ndatabase = \"trading\"\n\
             username = \"advisor_ro\"\nmax_connections = 5\nacquire_timeout_secs = 10\n"
        );
        let postgres = parse(&text).unwrap().postgres.unwrap();
        assert_eq!(postgres.port, 5432);
        assert_eq!(postgres.acquire_timeout(), Duration::from_secs(10));
    }

    #[test]
    fn news_sections_parse() {
        let text = format!(
            "{VALID}\n[embeddings]\nurl = \"http://localhost:8080/embed\"\ntimeout_secs = 15\nvector_size = 384\n\
             [qdrant]\nurl = \"http://localhost:6334\"\ncollection = \"news\"\ntimeout_secs = 10\n\
             [news_search]\ndefault_results = 5\nmax_results = 10\nmax_excerpt_chars = 1500\n"
        );
        let settings = parse(&text).unwrap();
        assert_eq!(settings.embeddings.unwrap().vector_size, 384);
        assert_eq!(settings.qdrant.unwrap().collection, "news");
        assert_eq!(settings.news_search.unwrap().max_results, 10);
        assert!(settings.warden.is_none());
    }

    #[test]
    fn pricing_and_evals_sections_parse() {
        let text = format!(
            "{VALID}\n[pricing.models.\"claude-x\"]\ninput_per_mtok = 2.0\noutput_per_mtok = 10.0\n\
             cache_write_per_mtok = 2.5\ncache_read_per_mtok = 0.1\n\
             [evals]\ncases_file = \"evals/cases.toml\"\nreport_dir = \"evals/reports\"\n\
             case_timeout_secs = 300\n"
        );
        let settings = parse(&text).unwrap();
        let prices = settings.price_table();
        assert_eq!(prices.price("claude-x").unwrap().output_per_mtok, 10.0);
        assert_eq!(
            settings.evals.unwrap().case_timeout(),
            Duration::from_secs(300)
        );
    }

    #[test]
    fn missing_pricing_means_empty_table() {
        let settings = parse(VALID).unwrap();
        assert!(settings.price_table().price("m").is_none());
    }

    #[test]
    fn unknown_tool_is_rejected() {
        let text = VALID.replace(
            r#"enabled = ["current_date"]"#,
            r#"enabled = ["crystal_ball"]"#,
        );
        let err = parse(&text).unwrap_err();
        assert!(err.to_string().contains("crystal_ball"), "got: {err}");
    }

    #[test]
    fn misspelled_setting_is_rejected() {
        let text = VALID.replace("timeout_secs", "timeout_sec");
        let err = parse(&text).unwrap_err();
        assert!(err.to_string().contains("timeout_sec"), "got: {err}");
    }

    #[test]
    fn invalid_url_is_rejected() {
        let text = VALID.replace("https://api.anthropic.com/v1/messages", "not a url");
        assert!(parse(&text).is_err());
    }

    #[test]
    fn required_reports_missing_and_empty() {
        let get_key = |key: &str| (key == "EMPTY").then(|| "  ".to_string());
        assert!(matches!(
            required(&get_key, "NOPE"),
            Err(ConfigError::Missing("NOPE"))
        ));
        assert!(matches!(
            required(&get_key, "EMPTY"),
            Err(ConfigError::Empty("EMPTY"))
        ));
    }
}
