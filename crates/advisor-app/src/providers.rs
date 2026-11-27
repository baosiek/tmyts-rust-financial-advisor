use std::sync::Arc;
use thiserror::Error;

use crate::config::{ConfigError, LlmSettings, ProviderKind, required};
use advisor_core::Secret;
use advisor_llm::{AnthropicConfig, AnthropicProvider, LlmError, LlmProvider, TimedProvider};

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
    }
}

fn validate_anthropic_api_key(api_key: &str) -> Result<(), ConfigError> {
    let invalid = |reason: &str| ConfigError::Invalid {
        name: "ANTHROPIC_API_KEY",
        reason: reason.to_string(),
    };

    if api_key.starts_with('"') || api_key.starts_with('\'') {
        return Err(invalid("the key is wrapped in quotes"));
    }
    if !api_key.starts_with("sk-ant-") {
        return Err(invalid("should start with `sk-ant-`"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AnthropicSettings;
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
}
