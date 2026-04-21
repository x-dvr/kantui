//! End-to-end tests: in-memory DB → seeded project → drive the app through
//! actions and assert on rendered buffers and persisted state.
//!
//! Assertions target buffer content rather than full snapshots so harmless
//! cosmetic changes to widgets don't flap these tests.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use kantui::action::Action;
use kantui::app::{self, App, AppServices, Mode};
use kantui::controller;
use kantui::view;
use kantui_core::{ProjectRepository as _, TaskRepository as _};
use kantui_store::sqlite::{SqliteProjectRepo, SqliteTaskRepo};
use kantui_store::{SqlitePool, SystemClock, UuidV4, sqlite};
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

async fn seeded_pool() -> SqlitePool {
    let pool = sqlite::connect_memory().await.expect("connect");
    app::seed_demo_if_empty(pool.clone(), SystemClock::new(), UuidV4::new())
        .await
        .expect("seed");
    pool
}

async fn build_app(pool: SqlitePool) -> App {
    let projects = SqliteProjectRepo::new(pool.clone());
    let tasks = SqliteTaskRepo::new(pool.clone());
    let project = projects
        .list()
        .await
        .expect("list projects")
        .into_iter()
        .next()
        .expect("seeded project");
    let board = app::load_board(&projects, &tasks, project)
        .await
        .expect("load board");
    App::new(board)
}

fn services(pool: &SqlitePool) -> AppServices {
    AppServices::new(pool.clone(), SystemClock::new(), UuidV4::new())
}

fn render_once(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(120, 20)).expect("terminal");
    terminal
        .draw(|frame| view::render(frame, app))
        .expect("draw");
    buffer_to_string(terminal.backend().buffer())
}

async fn run_actions(
    app: &mut App,
    services: &AppServices,
    actions: impl IntoIterator<Item = Action>,
) {
    for action in actions {
        controller::process(action, app, services)
            .await
            .expect("process");
    }
}

async fn type_text(app: &mut App, services: &AppServices, text: &str) {
    for ch in text.chars() {
        controller::process(Action::InsertChar(ch), app, services)
            .await
            .expect("process");
    }
}

async fn dispatch_key(app: &mut App, services: &AppServices, code: KeyCode, mods: KeyModifiers) {
    let key = KeyEvent::new(code, mods);
    let action = app.keymap.dispatch(app.mode, key);
    controller::process(action, app, services)
        .await
        .expect("process");
}

#[tokio::test]
async fn seeded_app_renders_with_expected_content() {
    let pool = seeded_pool().await;
    let app = build_app(pool).await;
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
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    assert_eq!(app.focused_column, 0);
    assert_eq!(app.selected_per_column[0], Some(0));

    run_actions(&mut app, &services, [Action::FocusNextColumn]).await;
    assert_eq!(app.focused_column, 1);

    run_actions(&mut app, &services, [Action::SelectNextTask]).await;
    assert_eq!(app.selected_per_column[1], Some(0));

    run_actions(
        &mut app,
        &services,
        [Action::FocusPrevColumn, Action::SelectNextTask],
    )
    .await;
    assert_eq!(app.selected_per_column[0], Some(1));

    // `gg` chord returns the cursor to the top of the focused column.
    dispatch_key(&mut app, &services, KeyCode::Char('g'), KeyModifiers::NONE).await;
    dispatch_key(&mut app, &services, KeyCode::Char('g'), KeyModifiers::NONE).await;
    assert_eq!(app.selected_per_column[0], Some(0));
}

#[tokio::test]
async fn quit_key_sets_should_quit() {
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    dispatch_key(&mut app, &services, KeyCode::Char('q'), KeyModifiers::NONE).await;
    assert!(app.should_quit);
}

#[tokio::test]
async fn new_task_creates_and_persists() {
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    // Press `n` to start a new-task flow in column 0 (Todo).
    dispatch_key(&mut app, &services, KeyCode::Char('n'), KeyModifiers::NONE).await;
    assert_eq!(app.mode, Mode::Insert);

    type_text(&mut app, &services, "Buy milk").await;
    dispatch_key(&mut app, &services, KeyCode::Enter, KeyModifiers::NONE).await;

    assert_eq!(app.mode, Mode::Normal);

    // Rendered buffer shows the new task.
    let out = render_once(&app);
    assert!(out.contains("Buy milk"), "new task not in buffer: {out}");

    // Task persisted to the DB under the first state.
    let state_id = app.board.project.states[0].id;
    let tasks = SqliteTaskRepo::new(pool.clone())
        .list_by_state(state_id)
        .await
        .expect("list");
    assert!(tasks.iter().any(|t| t.title == "Buy milk"));
}

#[tokio::test]
async fn insert_cancel_leaves_no_task() {
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    dispatch_key(&mut app, &services, KeyCode::Char('n'), KeyModifiers::NONE).await;
    type_text(&mut app, &services, "discard me").await;
    dispatch_key(&mut app, &services, KeyCode::Esc, KeyModifiers::NONE).await;

    assert_eq!(app.mode, Mode::Normal);
    assert!(app.pending_edit.is_none());

    let state_id = app.board.project.states[0].id;
    let tasks = SqliteTaskRepo::new(pool.clone())
        .list_by_state(state_id)
        .await
        .expect("list");
    assert!(!tasks.iter().any(|t| t.title == "discard me"));
}

#[tokio::test]
async fn delete_removes_task() {
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    // Select first task in Todo (seeded "Wire up event loop") and delete.
    assert_eq!(app.board.tasks_by_state[0].len(), 2);
    let victim_id = app.board.tasks_by_state[0][0].id;
    run_actions(&mut app, &services, [Action::DeleteTask]).await;

    assert_eq!(app.board.tasks_by_state[0].len(), 1);
    let repo = SqliteTaskRepo::new(pool.clone());
    let tasks = repo
        .list_by_state(app.board.project.states[0].id)
        .await
        .expect("list");
    assert!(tasks.iter().all(|t| t.id != victim_id));
}

#[tokio::test]
async fn move_task_across_columns() {
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    // Move "Wire up event loop" from Todo (col 0) → Doing (col 1).
    let task_id = app.board.tasks_by_state[0][0].id;
    run_actions(&mut app, &services, [Action::MoveTaskNextColumn]).await;

    assert_eq!(app.focused_column, 1);
    let doing = &app.board.tasks_by_state[1];
    assert!(
        doing.iter().any(|t| t.id == task_id),
        "task did not land in Doing"
    );
    assert!(
        !app.board.tasks_by_state[0].iter().any(|t| t.id == task_id),
        "task still in Todo"
    );

    // Persisted.
    let repo = SqliteTaskRepo::new(pool.clone());
    let reloaded = repo.get(task_id).await.expect("get").expect("exists");
    assert_eq!(reloaded.state_id, app.board.project.states[1].id);
}

#[tokio::test]
async fn shift_task_within_column() {
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    // Todo has two seeded tasks; move the first one down past the second.
    let first_id = app.board.tasks_by_state[0][0].id;
    let second_id = app.board.tasks_by_state[0][1].id;
    run_actions(&mut app, &services, [Action::ShiftTaskDown]).await;

    let todo = &app.board.tasks_by_state[0];
    assert_eq!(todo[0].id, second_id, "second task should now lead");
    assert_eq!(todo[1].id, first_id, "first task should now trail");
    assert_eq!(app.selected_per_column[0], Some(1));
}

#[tokio::test]
async fn search_filters_visible_tasks_and_clears_on_escape() {
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    // Enter search mode and live-filter to tasks containing "wire".
    dispatch_key(&mut app, &services, KeyCode::Char('/'), KeyModifiers::NONE).await;
    assert_eq!(app.mode, Mode::Search);
    type_text(&mut app, &services, "wire").await;

    assert_eq!(app.active_filter(), Some("wire"));
    // Only "Wire up event loop" remains visible in Todo.
    assert_eq!(app.visible_tasks(0).len(), 1);
    assert!(app.visible_tasks(1).is_empty());

    // Commit with Enter — filter stays.
    dispatch_key(&mut app, &services, KeyCode::Enter, KeyModifiers::NONE).await;
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.active_filter(), Some("wire"));

    // Re-enter search and cancel with Esc — filter clears, all tasks visible.
    dispatch_key(&mut app, &services, KeyCode::Char('/'), KeyModifiers::NONE).await;
    dispatch_key(&mut app, &services, KeyCode::Esc, KeyModifiers::NONE).await;
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.active_filter(), None);
    assert_eq!(app.visible_tasks(0).len(), 2);
}

#[tokio::test]
async fn help_toggle_shows_overlay() {
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    assert!(!app.show_help);
    dispatch_key(&mut app, &services, KeyCode::Char('?'), KeyModifiers::NONE).await;
    assert!(app.show_help);

    let out = render_once(&app);
    assert!(out.contains("Keybindings"), "help title missing: {out}");
    assert!(out.contains("Quit"), "help body missing: {out}");

    // Pressing `?` again closes it.
    dispatch_key(&mut app, &services, KeyCode::Char('?'), KeyModifiers::NONE).await;
    assert!(!app.show_help);
}

#[tokio::test]
async fn command_quit_sets_should_quit() {
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    dispatch_key(&mut app, &services, KeyCode::Char(':'), KeyModifiers::NONE).await;
    assert_eq!(app.mode, Mode::Command);
    type_text(&mut app, &services, "quit").await;
    dispatch_key(&mut app, &services, KeyCode::Enter, KeyModifiers::NONE).await;
    assert!(app.should_quit);
}

#[tokio::test]
async fn command_new_task_creates_task() {
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    dispatch_key(&mut app, &services, KeyCode::Char(':'), KeyModifiers::NONE).await;
    type_text(&mut app, &services, "new-task From command").await;
    dispatch_key(&mut app, &services, KeyCode::Enter, KeyModifiers::NONE).await;
    assert_eq!(app.mode, Mode::Normal);

    let state_id = app.board.project.states[0].id;
    let tasks = SqliteTaskRepo::new(pool.clone())
        .list_by_state(state_id)
        .await
        .expect("list");
    assert!(tasks.iter().any(|t| t.title == "From command"));
}

#[tokio::test]
async fn command_new_state_adds_column() {
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    let before = app.board.project.states.len();
    dispatch_key(&mut app, &services, KeyCode::Char(':'), KeyModifiers::NONE).await;
    type_text(&mut app, &services, "new-state Review").await;
    dispatch_key(&mut app, &services, KeyCode::Enter, KeyModifiers::NONE).await;

    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.board.project.states.len(), before + 1);
    assert!(
        app.board.project.states.iter().any(|s| s.name == "Review"),
        "new state not present"
    );
}

#[tokio::test]
async fn unknown_command_reports_status_message() {
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    dispatch_key(&mut app, &services, KeyCode::Char(':'), KeyModifiers::NONE).await;
    type_text(&mut app, &services, "bogus").await;
    dispatch_key(&mut app, &services, KeyCode::Enter, KeyModifiers::NONE).await;

    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(
        app.status_message.as_deref(),
        Some("unknown command: bogus")
    );
}

#[tokio::test]
async fn jump_label_teleports_selection() {
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    // gw → generate labels across columns. The Doing column (index 1) has
    // exactly one seeded task ("Render board with widgets"), which is the
    // 3rd visible task overall in column-major order: aa, ab, ac.
    dispatch_key(&mut app, &services, KeyCode::Char('g'), KeyModifiers::NONE).await;
    dispatch_key(&mut app, &services, KeyCode::Char('w'), KeyModifiers::NONE).await;
    assert_eq!(app.mode, Mode::Jump);

    // Type "ac" → jumps to (column=1, index=0).
    dispatch_key(&mut app, &services, KeyCode::Char('a'), KeyModifiers::NONE).await;
    dispatch_key(&mut app, &services, KeyCode::Char('c'), KeyModifiers::NONE).await;

    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.focused_column, 1);
    assert_eq!(app.selected_per_column[1], Some(0));
}

#[tokio::test]
async fn jump_escape_returns_to_normal() {
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    dispatch_key(&mut app, &services, KeyCode::Char('g'), KeyModifiers::NONE).await;
    dispatch_key(&mut app, &services, KeyCode::Char('w'), KeyModifiers::NONE).await;
    assert_eq!(app.mode, Mode::Jump);

    dispatch_key(&mut app, &services, KeyCode::Esc, KeyModifiers::NONE).await;
    assert_eq!(app.mode, Mode::Normal);
    assert!(app.jump.is_none());
}

#[tokio::test]
async fn rename_updates_title() {
    let pool = seeded_pool().await;
    let mut app = build_app(pool.clone()).await;
    let services = services(&pool);

    let victim_id = app.board.tasks_by_state[0][0].id;

    dispatch_key(&mut app, &services, KeyCode::Char('i'), KeyModifiers::NONE).await;
    assert_eq!(app.mode, Mode::Insert);
    // Clear seeded title and type a new one.
    run_actions(&mut app, &services, [Action::InsertMoveEnd]).await;
    for _ in 0..80 {
        controller::process(Action::InsertBackspace, &mut app, &services)
            .await
            .expect("process");
    }
    type_text(&mut app, &services, "Renamed task").await;
    dispatch_key(&mut app, &services, KeyCode::Enter, KeyModifiers::NONE).await;

    assert_eq!(app.mode, Mode::Normal);
    let repo = SqliteTaskRepo::new(pool.clone());
    let reloaded = repo.get(victim_id).await.expect("get").expect("exists");
    assert_eq!(reloaded.title, "Renamed task");
}
