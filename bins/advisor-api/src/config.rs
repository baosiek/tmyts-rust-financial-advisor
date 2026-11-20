//! Reads configuration: secrets from environment variables (and `.env`),
//! every other setting from the TOML file named by `ADVISOR_CONFIG`.

use std::path::{Path, PathBuf};
use std::time::Duration;

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
}

/// Non-secret settings, read from the TOML file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub llm: LlmSettings,
    pub agent: AgentSettings,
}

/// Which LLM provider to use, plus the settings of every configured provider.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmSettings {
    pub provider: ProviderKind,
    pub anthropic: Option<AnthropicSettings>,
}

/// The providers this build knows about. Add a variant when a new provider is implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Anthropic,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnthropicSettings {
    pub url: Url,
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
pub struct AgentSettings {
    pub max_tokens: u32,
    pub system_prompt: String,
}

impl Settings {
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

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
        [llm]
        provider = "anthropic"

        [llm.anthropic]
        url = "https://api.anthropic.com/v1/messages"
        version = "2023-06-01"
        model = "m"
        timeout_secs = 120

        [agent]
        max_tokens = 1024
        system_prompt = "p"
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
    fn unknown_provider_is_rejected() {
        let text = VALID.replace(r#"provider = "anthropic""#, r#"provider = "gemini""#);
        let err = parse(&text).unwrap_err();
        assert!(err.to_string().contains("gemini"), "got: {err}");
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
