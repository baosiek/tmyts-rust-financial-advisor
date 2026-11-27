use std::sync::Arc;
use thiserror::Error;

use crate::config::{ConfigError, LlmSettings, ProviderKind, required};
use advisor_core::Secret;
use advisor_llm::{
    AnthropicConfig, AnthropicProvider, LlmError, LlmProvider, OpenaiConfig, OpenaiProvider,
    TimedProvider,
};

#[derive(Debug, Error)]
pub enum BuildProviderError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error("could not create the LLM provider: {0}")]
    Llm(#[from] LlmError),
}

/// Builds the provider selected in the settings and wraps it in the decorators
/// every call goes through (today: timing and logging). Secrets are read through
/// `get_key`, and only the selected provider's secret is required.
pub fn build_provider<F>(
    llm: &LlmSettings,
    get_key: F,
) -> Result<Arc<dyn LlmProvider>, BuildProviderError>
where
    F: Fn(&str) -> Option<String>,
{
    let provider = build_concrete_provider(llm, get_key)?;
    Ok(Arc::new(TimedProvider::new(provider, llm.call_timeout())))
}

/// Builds the bare provider selected in the settings, without decorators.
fn build_concrete_provider<F>(
    llm: &LlmSettings,
    get_key: F,
) -> Result<Arc<dyn LlmProvider>, BuildProviderError>
where
    F: Fn(&str) -> Option<String>,
{
    match llm.provider {
        ProviderKind::Anthropic => {
            let settings = llm
                .anthropic
                .as_ref()
                .ok_or(ConfigError::MissingProviderSettings {
                    provider: "anthropic",
                })?;

            let api_key = required(&get_key, "ANTHROPIC_API_KEY")?;
            validate_anthropic_api_key(&api_key)?;

            let provider = AnthropicProvider::new(AnthropicConfig {
                api_key: Secret::new(api_key),
                model: settings.model.clone(),
                version: settings.version.clone(),
                url: settings.url.clone(),
                models_url: settings.models_url.clone(),
                timeout: settings.timeout(),
            })?;

            Ok(Arc::new(provider))
        }
        ProviderKind::Openai => {
            let settings = llm
                .openai
                .as_ref()
                .ok_or(ConfigError::MissingProviderSettings { provider: "openai" })?;

            let api_key = required(&get_key, "OPENAI_API_KEY")?;
            validate_openai_api_key(&api_key)?;

            let provider = OpenaiProvider::new(OpenaiConfig {
                api_key: Secret::new(api_key),
                model: settings.model.clone(),
                responses_url: settings.responses_url.clone(),
                models_url: settings.models_url.clone(),
                timeout: settings.timeout(),
                replay_reasoning: settings.replay_reasoning,
            })?;

            Ok(Arc::new(provider))
        }
    }
}

fn validate_anthropic_api_key(api_key: &str) -> Result<(), ConfigError> {
    validate_api_key("ANTHROPIC_API_KEY", api_key, "sk-ant-")
}

fn validate_openai_api_key(api_key: &str) -> Result<(), ConfigError> {
    validate_api_key("OPENAI_API_KEY", api_key, "sk-")
}

/// Catches the usual copy-paste mistakes before the first request does.
fn validate_api_key(name: &'static str, api_key: &str, prefix: &str) -> Result<(), ConfigError> {
    let invalid = |reason: String| ConfigError::Invalid { name, reason };

    if api_key.starts_with('"') || api_key.starts_with('\'') {
        return Err(invalid("the key is wrapped in quotes".to_string()));
    }
    if !api_key.starts_with(prefix) {
        return Err(invalid(format!("should start with `{prefix}`")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AnthropicSettings, OpenaiSettings};
    use std::collections::HashMap;

    fn anthropic_settings() -> LlmSettings {
        LlmSettings {
            provider: ProviderKind::Anthropic,
            call_timeout_secs: 180,
            anthropic: Some(AnthropicSettings {
                url: "https://api.anthropic.com/v1/messages".parse().unwrap(),
                models_url: "https://api.anthropic.com/v1/models".parse().unwrap(),
                version: "2023-06-01".to_string(),
                model: "test-model".to_string(),
                timeout_secs: 5,
            }),
            openai: None,
        }
    }

    fn openai_settings() -> LlmSettings {
        LlmSettings {
            provider: ProviderKind::Openai,
            call_timeout_secs: 180,
            anthropic: None,
            openai: Some(OpenaiSettings {
                responses_url: "https://api.openai.com/v1/responses".parse().unwrap(),
                models_url: "https://api.openai.com/v1/models".parse().unwrap(),
                model: "openai-test-model".to_string(),
                timeout_secs: 5,
                replay_reasoning: false,
            }),
        }
    }

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key| map.get(key).cloned()
    }

    #[test]
    fn builds_the_selected_provider() {
        let provider = build_provider(
            &anthropic_settings(),
            env(&[("ANTHROPIC_API_KEY", "sk-ant-test")]),
        )
        .unwrap();
        assert_eq!(provider.name(), "Anthropic");
        assert_eq!(provider.model(), "test-model");
    }

    #[test]
    fn missing_api_key_is_reported() {
        let err = build_provider(&anthropic_settings(), env(&[]))
            .err()
            .unwrap();
        assert!(matches!(
            err,
            BuildProviderError::Config(ConfigError::Missing("ANTHROPIC_API_KEY"))
        ));
    }

    #[test]
    fn quoted_api_key_is_rejected() {
        let err = build_provider(
            &anthropic_settings(),
            env(&[("ANTHROPIC_API_KEY", "\"sk-ant-test\"")]),
        )
        .err()
        .unwrap();
        assert!(matches!(
            err,
            BuildProviderError::Config(ConfigError::Invalid { .. })
        ));
    }

    #[test]
    fn missing_provider_section_is_reported() {
        let mut settings = anthropic_settings();
        settings.anthropic = None;
        let err = build_provider(&settings, env(&[("ANTHROPIC_API_KEY", "sk-ant-test")]))
            .err()
            .unwrap();
        assert!(matches!(
            err,
            BuildProviderError::Config(ConfigError::MissingProviderSettings {
                provider: "anthropic"
            })
        ));
    }

    #[test]
    fn builds_openai_when_selected() {
        let provider = build_provider(
            &openai_settings(),
            env(&[("OPENAI_API_KEY", "sk-proj-test")]),
        )
        .unwrap();
        assert_eq!(provider.name(), "OpenAI");
        assert_eq!(provider.model(), "openai-test-model");
        assert_eq!(provider.service_name(), "openai_provider");
    }

    #[test]
    fn only_the_selected_providers_key_is_required() {
        // OpenAI selected: a missing ANTHROPIC_API_KEY does not matter.
        let err = build_provider(
            &openai_settings(),
            env(&[("ANTHROPIC_API_KEY", "sk-ant-x")]),
        )
        .err()
        .unwrap();
        assert!(matches!(
            err,
            BuildProviderError::Config(ConfigError::Missing("OPENAI_API_KEY"))
        ));
    }

    #[test]
    fn anthropic_key_for_openai_is_rejected() {
        let err = build_provider(&openai_settings(), env(&[("OPENAI_API_KEY", "ant-key")]))
            .err()
            .unwrap();
        assert!(matches!(
            err,
            BuildProviderError::Config(ConfigError::Invalid {
                name: "OPENAI_API_KEY",
                ..
            })
        ));
    }
}
