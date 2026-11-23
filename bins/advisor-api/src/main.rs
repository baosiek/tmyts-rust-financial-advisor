mod config;
mod providers;

use std::process::ExitCode;

use advisor_agent::Agent;
use config::Settings;

/// `main` is synchronous on purpose: the runtime's thread count comes from the
/// settings file, so settings must be read before the runtime exists.
fn main() -> ExitCode {
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

    match runtime.block_on(run(settings)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(settings: Settings) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!(
        provider = ?settings.llm.provider,
        worker_threads = settings.runtime.worker_threads,
        "starting advisor"
    );

    // Composition root: build the selected provider, wrapped in TimedProvider.
    let provider = providers::build_provider(&settings.llm, |key| std::env::var(key).ok())?;

    // Inject it into the agent. The agent never names a concrete provider.
    let mut agent = Agent::new(
        provider,
        settings.agent.system_prompt.clone(),
        settings.agent.max_tokens,
    );

    // The answer is program output, so it goes to stdout; diagnostics go to the log.
    let answer = agent.ask("Hi").await?;
    println!("{answer}");

    Ok(())
}
