//! kantui — terminal kanban board binary entry point.
//!
//! All composition logic lives in `kantui::*`; this file just parses args,
//! initialises logging + terminal, and hands control off.

use std::process::ExitCode;

use clap::Parser;
use kantui::{app, cli, logging, tui};
use kantui_core::{CoreError, CoreResult, ProjectRepository as _};
use kantui_store::sqlite::{SqliteProjectRepo, SqliteTaskRepo};
use kantui_store::{SystemClock, UuidV4, sqlite};

#[tokio::main]
async fn main() -> ExitCode {
    let args = cli::Args::parse();
    let resolved = args.resolve();

    if let Err(err) = logging::init(&resolved.log_path, &resolved.log_level) {
        eprintln!(
            "failed to initialise logging at {:?}: {err}",
            resolved.log_path
        );
        return ExitCode::FAILURE;
    }

    match run(resolved).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            // Core errors carry their full cause chain; log it before we show
            // a single-line summary so operators have enough to diagnose.
            tracing::error!("{}", err.log_chain());
            eprintln!("kantui: {err}");
            ExitCode::FAILURE
        }
    }
}

async fn run(resolved: cli::Resolved) -> CoreResult<()> {
    tracing::info!(db_url = %resolved.db_url, "starting kantui");

    let pool = sqlite::connect(&resolved.db_url).await?;

    let project = if resolved.seed_demo {
        app::seed_demo_if_empty(pool.clone(), SystemClock::new(), UuidV4::new()).await?
    } else {
        let repo = SqliteProjectRepo::new(pool.clone());
        repo.list()
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| CoreError::validation("no projects in the database"))?
    };

    let tasks_repo = SqliteTaskRepo::new(pool.clone());
    let projects_repo = SqliteProjectRepo::new(pool.clone());
    let board = app::load_board(&projects_repo, &tasks_repo, project).await?;
    let mut app_state = app::App::new(board);

    let mut terminal = tui::init().map_err(io_to_core)?;
    let result = app::run(&mut terminal, &mut app_state).await;
    tui::restore().map_err(io_to_core)?;

    result
}

fn io_to_core(err: std::io::Error) -> CoreError {
    CoreError::storage("terminal setup failed", err)
}
