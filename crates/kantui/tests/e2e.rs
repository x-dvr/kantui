//! End-to-end smoke test: in-memory DB → seeded project → rendered buffer.
//!
//! Asserts on the rendered buffer content rather than a full snapshot so the
//! test survives harmless cosmetic changes to widgets.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use kantui::action::Action;
use kantui::app::{self, App};
use kantui::view;
use kantui_core::ProjectRepository as _;
use kantui_store::sqlite::{SqliteProjectRepo, SqliteTaskRepo};
use kantui_store::{SystemClock, UuidV4, sqlite};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

fn buffer_to_string(buf: &Buffer) -> String {
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

async fn build_app() -> App {
    let pool = sqlite::connect_memory().await.expect("connect");
    let project = app::seed_demo_if_empty(pool.clone(), SystemClock::new(), UuidV4::new())
        .await
        .expect("seed");

    let projects = SqliteProjectRepo::new(pool.clone());
    let tasks = SqliteTaskRepo::new(pool.clone());
    let reloaded = projects
        .get(project.id)
        .await
        .expect("reload")
        .expect("project exists");
    let board = app::load_board(&projects, &tasks, reloaded)
        .await
        .expect("load board");

    App::new(board)
}

fn render_once(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(120, 20)).expect("terminal");
    terminal
        .draw(|frame| view::render(frame, app))
        .expect("draw");
    buffer_to_string(terminal.backend().buffer())
}

#[tokio::test]
async fn seeded_app_renders_with_expected_content() {
    let app = build_app().await;
    let out = render_once(&app);

    assert!(out.contains("Todo"), "missing Todo column: {out}");
    assert!(out.contains("Doing"), "missing Doing column: {out}");
    assert!(out.contains("Done"), "missing Done column: {out}");

    assert!(
        out.contains("Wire up event loop"),
        "missing seeded Todo task: {out}"
    );
    assert!(
        out.contains("Render board with widgets"),
        "missing seeded Doing task: {out}"
    );
    assert!(
        out.contains("Scaffold workspace"),
        "missing seeded Done task: {out}"
    );

    assert!(out.contains("NOR"), "missing mode chip: {out}");
    assert!(out.contains("kantui"), "missing project name: {out}");
}

#[tokio::test]
async fn navigation_changes_focus_and_selection() {
    let mut app = build_app().await;
    assert_eq!(app.focused_column, 0);
    assert_eq!(app.selected_per_column[0], Some(0));

    app.apply(Action::FocusNextColumn);
    assert_eq!(app.focused_column, 1);

    app.apply(Action::SelectNextTask);
    assert_eq!(app.selected_per_column[1], Some(0));

    app.apply(Action::FocusPrevColumn);
    app.apply(Action::SelectNextTask);
    assert_eq!(app.selected_per_column[0], Some(1));

    let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
    let _ = app.keymap.dispatch(key);
    let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
    let action = app.keymap.dispatch(key);
    app.apply(action);
    assert_eq!(app.selected_per_column[0], Some(0));
}

#[tokio::test]
async fn quit_key_sets_should_quit() {
    let mut app = build_app().await;
    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    let action = app.keymap.dispatch(key);
    app.apply(action);
    assert!(app.should_quit);
}
