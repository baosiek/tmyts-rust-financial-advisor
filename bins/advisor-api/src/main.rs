mod config;
mod providers;
mod startup;
mod tools;

use std::process::ExitCode;
use std::sync::Arc;

use advisor_agent::{Agent, AgentConfig};
use config::Settings;

/// `main` is synchronous on purpose: the runtime's thread count comes from the
/// settings file, so settings must be read before the runtime exists.
fn main() -> ExitCode {
    // 0. The question comes from the command line, e.g.
    //    cargo run -p advisor-api -- "What is today's date?"
    let question = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    if question.trim().is_empty() {
        eprintln!("usage: advisor-api <question>");
        return ExitCode::FAILURE;
    }

    // 1. Configuration. Logging is not running yet, so errors go to stderr.
    let settings = match Settings::load() {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    // 2. Logging, with the filter from the settings file (RUST_LOG overrides it).
    if let Err(e) = advisor_observability::init_tracing(&settings.logging.filter) {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }

    // 3. The async runtime, sized from the settings file.
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(settings.runtime.worker_threads)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            tracing::error!("cannot start the async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run(settings, question)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(settings: Settings, question: String) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!(
        provider = ?settings.llm.provider,
        tools = ?settings.tools.enabled,
        worker_threads = settings.runtime.worker_threads,
        "starting advisor"
    );

    // Composition root: build the selected provider (wrapped in TimedProvider),
    // the enabled tools and the services they need.
    let get_key = |key: &str| std::env::var(key).ok();
    let provider = providers::build_provider(&settings.llm, get_key)?;
    let built = tools::build_tools(&settings, get_key)?;

    // Before the agent starts: check every service the tools depend on.
    startup::check_services(
        &built.health_checks,
        settings.startup.health_check_timeout(),
    )
    .await?;

    // Inject provider and tools into the agent. It never names a concrete type.
    let mut agent = Agent::new(
        provider,
        Arc::new(built.registry),
        AgentConfig {
            system_prompt: settings.agent.system_prompt.clone(),
            max_tokens: settings.agent.max_tokens,
            max_tool_rounds: settings.agent.max_tool_rounds,
        },
    );

    // The answer is program output, so it goes to stdout; diagnostics go to the log.
    let answer = agent.ask(question).await?;
    println!("{answer}");

    Ok(())
}
