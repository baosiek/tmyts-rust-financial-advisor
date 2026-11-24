//! Composition root for tools: the only place that knows the concrete tool types
//! and the services they need. `[tools] enabled` in the settings file decides
//! which tools the agent gets; a service (e.g. Postgres) is only created when an
//! enabled tool needs it, and it is then also health-checked at startup.

use std::sync::Arc;

use advisor_core::{HealthCheck, Secret};
use advisor_embed::{HttpEmbedder, HttpEmbedderConfig};
use advisor_market_data::{PgPriceRepository, PostgresConfig};
use advisor_news_rag::{QdrantConfig, QdrantNewsRepository};
use advisor_tools::{
    CurrentDateTool, PriceHistoryTool, SearchNewsConfig, SearchNewsTool, ToolRegistry,
};

use crate::config::{ConfigError, PostgresSettings, Settings, ToolKind, optional, required};

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
            ToolKind::SearchNews => {
                let embedder = Arc::new(build_embedder(settings)?);
                let news = Arc::new(build_qdrant(settings, &get_key)?);
                let limits = section(&settings.news_search, "news_search")?;
                health_checks.push(embedder.clone());
                health_checks.push(news.clone());
                registry.register(SearchNewsTool::new(
                    embedder,
                    news,
                    SearchNewsConfig {
                        default_results: limits.default_results,
                        max_results: limits.max_results,
                        max_excerpt_chars: limits.max_excerpt_chars,
                    },
                ));
            }
        }
    }

    Ok(BuiltTools {
        registry,
        health_checks,
    })
}

/// The settings section `name`, or an error saying `search_news` needs it.
fn section<'a, T>(value: &'a Option<T>, name: &'static str) -> Result<&'a T, ConfigError> {
    value.as_ref().ok_or(ConfigError::MissingSection {
        section: name,
        needed_by: "search_news",
    })
}

fn build_embedder(settings: &Settings) -> Result<HttpEmbedder, ConfigError> {
    let embeddings = section(&settings.embeddings, "embeddings")?;
    HttpEmbedder::new(HttpEmbedderConfig {
        url: embeddings.url.clone(),
        timeout: embeddings.timeout(),
        dimensions: embeddings.vector_size,
    })
    .map_err(|e| ConfigError::Service {
        service: "embeddings",
        reason: e.to_string(),
    })
}

fn build_qdrant<F>(settings: &Settings, get_key: &F) -> Result<QdrantNewsRepository, ConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    let qdrant = section(&settings.qdrant, "qdrant")?;
    // The vector size is set once, in [embeddings], and checked against the collection.
    let embeddings = section(&settings.embeddings, "embeddings")?;
    QdrantNewsRepository::new(QdrantConfig {
        url: qdrant.url.clone(),
        api_key: optional(get_key, "QDRANT_API_KEY").map(Secret::new),
        collection: qdrant.collection.clone(),
        timeout: qdrant.timeout(),
        vector_size: embeddings.vector_size as u64,
    })
    .map_err(|e| ConfigError::Service {
        service: "qdrant",
        reason: e.to_string(),
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
        Settings::from_toml_str(
            &settings_text(enabled, with_postgres),
            Path::new("test.toml"),
        )
        .unwrap()
    }

    fn settings_text(enabled: &str, with_postgres: bool) -> String {
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
        text
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

    fn news_sections() -> &'static str {
        "[embeddings]\nurl = \"http://localhost:8080/embed\"\ntimeout_secs = 1\nvector_size = 384\n\
         [qdrant]\nurl = \"http://localhost:6334\"\ncollection = \"news\"\ntimeout_secs = 1\n\
         [news_search]\ndefault_results = 5\nmax_results = 10\nmax_excerpt_chars = 1500\n"
    }

    #[test]
    fn news_tool_adds_embeddings_and_qdrant_checks() {
        let mut text = settings_text(r#""search_news""#, false);
        text.push_str(news_sections());
        let settings = Settings::from_toml_str(&text, Path::new("test.toml")).unwrap();

        let built = build_tools(&settings, |_| None).unwrap();
        assert_eq!(names(&built), vec!["search_news"]);
        let services: Vec<&str> = built
            .health_checks
            .iter()
            .map(|c| c.service_name())
            .collect();
        assert_eq!(services, vec!["embeddings", "qdrant"]);
    }

    #[test]
    fn news_tool_without_qdrant_section_is_reported() {
        let settings = settings(r#""search_news""#, false);
        let err = build_tools(&settings, |_| None).err().unwrap();
        assert!(
            matches!(err, ConfigError::MissingSection { .. }),
            "got {err:?}"
        );
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
