mod config;
mod providers;

use config::Settings;
use std::process::ExitCode;

use advisor_agent::Agent;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // `{e}` uses Display: a readable message instead of the Debug dump.
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Configuration: settings from the TOML file, secrets from the environment.
    let settings = Settings::load()?;

    // 2. Composition root: build the provider selected in the settings.
    let provider = providers::build_provider(&settings.llm, |key| std::env::var(key).ok())?;

    // 3. Inject it into the agent. The agent never names a concrete provider.
    let mut agent = Agent::new(
        provider,
        settings.agent.system_prompt.clone(),
        settings.agent.max_tokens,
    );

    let answer = agent.ask("Hi").await?;
    println!("{answer}");

    Ok(())
}
