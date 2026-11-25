//! Runs the eval cases against the real agent, with the real provider and
//! services from `config/advisor.toml`. This calls the paid API, so it is run
//! on demand, not on every build:
//!
//!   cargo run -p advisor-evals                     # all cases
//!   cargo run -p advisor-evals -- --only price_aapl --only news_cites_source
//!
//! Exit code: 0 when every case passed, 1 when any failed, 2 when the run could not start.

use std::process::ExitCode;

use advisor_app::App;
use advisor_app::config::Settings;
use advisor_evals::{Report, load_cases, run_cases};

fn main() -> ExitCode {
    let only = match parse_only(std::env::args().skip(1)) {
        Ok(only) => only,
        Err(e) => {
            eprintln!("error: {e}\nusage: advisor-evals [--only <case id>]...");
            return ExitCode::from(2);
        }
    };

    let settings = match Settings::load() {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = advisor_observability::init_tracing(&settings.logging.filter) {
        eprintln!("error: {e}");
        return ExitCode::from(2);
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(settings.runtime.worker_threads)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            tracing::error!("cannot start the async runtime: {e}");
            return ExitCode::from(2);
        }
    };

    match runtime.block_on(run(settings, only)) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            tracing::error!("{e}");
            ExitCode::from(2)
        }
    }
}

/// Returns whether every case passed.
async fn run(settings: Settings, only: Vec<String>) -> Result<bool, Box<dyn std::error::Error>> {
    let evals = settings
        .evals
        .clone()
        .ok_or("the settings file has no [evals] section")?;

    let mut cases = load_cases(&evals.cases_file)?;
    if !only.is_empty() {
        if let Some(unknown) = only.iter().find(|id| !cases.iter().any(|c| &c.id == *id)) {
            return Err(format!(
                "no case with id `{unknown}` in {}",
                evals.cases_file.display()
            )
            .into());
        }
        cases.retain(|case| only.contains(&case.id));
    }

    let get_key = |key: &str| std::env::var(key).ok();
    let app = App::build(&settings, get_key).await?;
    tracing::info!(
        cases = cases.len(),
        model = app.model(),
        "running evals (each case calls the paid API)"
    );

    let started_at = chrono::Utc::now();
    let results = run_cases(
        &cases,
        || app.new_agent(),
        app.prices(),
        evals.case_timeout(),
    )
    .await;
    let report = Report::new(started_at, app.provider_name(), app.model(), results);

    let (json_path, md_path) = report.write(&evals.report_dir)?;
    println!("{}", report.to_markdown());
    tracing::info!(
        passed = report.totals.passed,
        failed = report.totals.failed,
        cost_usd = ?report.totals.cost_usd,
        json = %json_path.display(),
        markdown = %md_path.display(),
        "evals finished"
    );
    Ok(report.all_passed())
}

/// `--only <id>` may be given several times.
fn parse_only(mut args: impl Iterator<Item = String>) -> Result<Vec<String>, String> {
    let mut only = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--only" => only.push(args.next().ok_or("--only needs a case id")?),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(only)
}
