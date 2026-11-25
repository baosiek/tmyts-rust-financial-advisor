use std::process::ExitCode;

use advisor_app::App;
use advisor_app::config::Settings;

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

    // Composition root (advisor-app): provider, tools, services and their health checks.
    let get_key = |key: &str| std::env::var(key).ok();
    let app = App::build(&settings, get_key).await?;
    let mut agent = app.new_agent();

    // The answer is program output, so it goes to stdout; diagnostics go to the log.
    let answer = agent.ask(question).await?;
    println!("{}", answer.text);

    // One line per question with what it cost: the number to watch.
    let usage = answer.trace.usage();
    let cost = answer.trace.cost_usd(app.prices());
    tracing::info!(
        llm_calls = answer.trace.llm_calls.len(),
        tools = ?answer.trace.tool_names(),
        input_tokens = usage.input_tokens,
        output_tokens = usage.output_tokens,
        cache_read_tokens = usage.cache_read_input_tokens,
        cost_usd = %format_cost(cost),
        elapsed_ms = answer.trace.elapsed.as_millis() as u64,
        "question answered"
    );

    Ok(())
}

/// `$0.012345`, or `unknown` when a model has no configured price.
fn format_cost(cost: Option<f64>) -> String {
    match cost {
        Some(usd) => format!("${usd:.6}"),
        None => "unknown".to_string(),
    }
}
