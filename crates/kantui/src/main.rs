//! kantui — terminal kanban board binary entry point.
//!
//! All composition logic lives in `kantui::*`; this file just parses args,
//! initialises logging + terminal, and hands control off.

use std::process::ExitCode;

use clap::Parser;
use kantui::{app, cli, config as app_config, logging, state as ui_state, tui};
use kantui_core::{CoreError, CoreResult, ProjectRepository};
use kantui_store::sqlite::{SqliteProjectRepo, SqliteTagRepo, SqliteTaskRepo};
use kantui_store::{SystemClock, UuidV4, sqlite};

#[tokio::main]
async fn main() -> ExitCode {
    let args = cli::Args::parse();
    let resolved = args.resolve();

    if resolved.gen_conf {
        return match app_config::write_default(&resolved.config_path) {
            Ok(()) => {
                println!("wrote default config to {}", resolved.config_path.display());
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!(
                    "failed to write config to {}: {err}",
                    resolved.config_path.display()
                );
                ExitCode::FAILURE
            }
        };
    }

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

    let (config, warnings) = app_config::load(&resolved.config_path);
    for warning in &warnings {
        tracing::warn!(config_path = %resolved.config_path.display(), "{warning}");
    }

    let pool = sqlite::connect(&resolved.db_url).await?;

    let fallback_project = if resolved.seed_demo {
        app::seed_demo_if_empty(pool.clone(), SystemClock::new(), UuidV4::new()).await?
    } else {
        app::ensure_default_project(pool.clone(), SystemClock::new(), UuidV4::new()).await?
    };

    let projects_repo = SqliteProjectRepo::new(pool.clone());
    let tasks_repo = SqliteTaskRepo::new(pool.clone());
    let tags_repo = SqliteTagRepo::new(pool.clone());

    let mut state = ui_state::UiState::load(&resolved.state_path);
    let project = match state.last_project_id() {
        Some(id) => match projects_repo.get(id).await? {
            Some(p) => p,
            None => fallback_project,
        },
        None => fallback_project,
    };

    // Persist whatever project we actually opened so the file always reflects
    // current truth, even on first run.
    state.set_last_project(project.id);
    if let Err(err) = state.save(&resolved.state_path) {
        tracing::warn!(path = %resolved.state_path.display(), %err, "failed to persist UI state");
    }

    let board = app::load_board(&projects_repo, &tasks_repo, &tags_repo, project).await?;
    let mut app_state = app::App::with_config(board, &config);
    let services = app::AppServices::new(
        pool.clone(),
        SystemClock::new(),
        UuidV4::new(),
        resolved.state_path.clone(),
    );

    let mut terminal = tui::init().map_err(io_to_core)?;
    let result = app::run(&mut terminal, &mut app_state, &services).await;
    tui::restore().map_err(io_to_core)?;

    result
}

fn io_to_core(err: std::io::Error) -> CoreError {
    CoreError::storage("terminal setup failed", err)
}
