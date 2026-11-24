//! Composition root for tools: the only place that knows the concrete tool types
//! and the services they need. `[tools] enabled` in the settings file decides
//! which tools the agent gets; a service (e.g. Postgres) is only created when an
//! enabled tool needs it, and it is then also health-checked at startup.

use std::sync::Arc;

use advisor_core::{HealthCheck, Secret};
use advisor_market_data::{PgPriceRepository, PostgresConfig};
use advisor_tools::{CurrentDateTool, PriceHistoryTool, ToolRegistry};

use crate::config::{ConfigError, PostgresSettings, Settings, ToolKind, required};

/// The tools for the agent, plus the services they depend on, to check at startup.
pub struct BuiltTools {
    pub registry: ToolRegistry,
    pub health_checks: Vec<Arc<dyn HealthCheck>>,
}

pub fn build_tools<F>(settings: &Settings, get_key: F) -> Result<BuiltTools, ConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    let mut registry = ToolRegistry::new();
    let mut health_checks: Vec<Arc<dyn HealthCheck>> = Vec::new();
    let mut postgres: Option<Arc<PgPriceRepository>> = None;

    for tool in &settings.tools.enabled {
        match tool {
            ToolKind::CurrentDate => registry.register(CurrentDateTool),
            ToolKind::PriceHistory => {
                // Created once, even if several tools use it later.
                let repository = match &postgres {
                    Some(repository) => repository.clone(),
                    None => {
                        let repository =
                            Arc::new(build_postgres(settings, &get_key, "get_price_history")?);
                        health_checks.push(repository.clone());
                        postgres = Some(repository.clone());
                        repository
                    }
                };
                registry.register(PriceHistoryTool::new(repository));
            }
        }
    }

    Ok(BuiltTools {
        registry,
        health_checks,
    })
}

fn build_postgres<F>(
    settings: &Settings,
    get_key: &F,
    needed_by: &'static str,
) -> Result<PgPriceRepository, ConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    let postgres: &PostgresSettings =
        settings
            .postgres
            .as_ref()
            .ok_or(ConfigError::MissingSection {
                section: "postgres",
                needed_by,
            })?;
    let password = required(get_key, "POSTGRES_PASSWORD")?;

    Ok(PgPriceRepository::new(&PostgresConfig {
        host: postgres.host.clone(),
        port: postgres.port,
        database: postgres.database.clone(),
        username: postgres.username.clone(),
        password: Secret::new(password),
        max_connections: postgres.max_connections,
        acquire_timeout: postgres.acquire_timeout(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn settings(enabled: &str, with_postgres: bool) -> Settings {
        let mut text = format!(
            r#"
            [runtime]
            worker_threads = 1
            [logging]
            filter = "info"
            [llm]
            provider = "anthropic"
            call_timeout_secs = 1
            [agent]
            max_tokens = 1
            max_tool_rounds = 1
            system_prompt = "p"
            [tools]
            enabled = [{enabled}]
            [startup]
            health_check_timeout_secs = 1
            "#
        );
        if with_postgres {
            text.push_str(
                "[postgres]\nhost = \"localhost\"\nport = 5432\ndatabase = \"trading\"\n\
                 username = \"advisor_ro\"\nmax_connections = 1\nacquire_timeout_secs = 1\n",
            );
        }
        Settings::from_toml_str(&text, Path::new("test.toml")).unwrap()
    }

    fn names(built: &BuiltTools) -> Vec<String> {
        built
            .registry
            .definitions()
            .into_iter()
            .map(|d| d.name)
            .collect()
    }

    #[test]
    fn date_tool_needs_no_database() {
        let built = build_tools(&settings(r#""current_date""#, false), |_| None).unwrap();
        assert_eq!(names(&built), vec!["current_date"]);
        assert!(built.health_checks.is_empty());
    }

    #[tokio::test]
    async fn price_tool_adds_postgres_and_its_health_check() {
        // Needs a runtime: the lazy pool is created inside one, without connecting.
        let built = build_tools(&settings(r#""price_history""#, true), |key| {
            (key == "POSTGRES_PASSWORD").then(|| "secret".to_string())
        })
        .unwrap();
        assert_eq!(names(&built), vec!["get_price_history"]);
        assert_eq!(built.health_checks.len(), 1);
        assert_eq!(built.health_checks[0].service_name(), "postgres");
    }

    #[test]
    fn price_tool_without_postgres_section_is_reported() {
        let err = build_tools(&settings(r#""price_history""#, false), |_| None)
            .err()
            .unwrap();
        assert!(matches!(
            err,
            ConfigError::MissingSection {
                section: "postgres",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn price_tool_without_password_is_reported() {
        let err = build_tools(&settings(r#""price_history""#, true), |_| None)
            .err()
            .unwrap();
        assert!(matches!(err, ConfigError::Missing("POSTGRES_PASSWORD")));
    }
}
