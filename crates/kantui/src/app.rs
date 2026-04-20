//! Application state + top-level event loop.
//!
//! M4 is read-only: we load one project's states + tasks at startup, render
//! it, and let the user navigate. Mutations, modes, and refresh-after-write
//! arrive in later milestones.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kantui_core::{
    CoreError, CoreResult, IdGenerator, NewProject, NewTask, Priority, Project, ProjectRepository,
    ProjectService, Task, TaskRepository, TaskService,
};
use kantui_store::sqlite::{SqliteProjectRepo, SqliteTaskRepo};
use kantui_store::{SqlitePool, SystemClock};
use ratatui::Terminal;
use ratatui::backend::Backend;

use crate::action::Action;
use crate::event::{AppEvent, Events};
use crate::keymap::Keymap;
use crate::view;

/// Duration between background ticks driving the clock/refresh.
pub const TICK: Duration = Duration::from_millis(500);

/// UI mode — M4 only needs Normal. Kept as a field so later milestones can
/// grow Insert/Command/Search without reshaping the state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
}

/// In-memory snapshot of what's on screen. The repository is queried once per
/// refresh, not per render.
pub struct BoardSnapshot {
    pub project: Project,
    /// Tasks indexed by the *position* of their state in `project.states`.
    pub tasks_by_state: Vec<Vec<Task>>,
}

pub struct App {
    pub mode: Mode,
    pub should_quit: bool,
    pub focused_column: usize,
    /// One selected-task index per column; kept even across column switches.
    pub selected_per_column: Vec<Option<usize>>,
    pub board: BoardSnapshot,
    pub keymap: Keymap,
    pub status_message: Option<String>,
}

impl App {
    pub fn new(board: BoardSnapshot) -> Self {
        let selected_per_column = board
            .tasks_by_state
            .iter()
            .map(|tasks| if tasks.is_empty() { None } else { Some(0) })
            .collect();
        Self {
            mode: Mode::Normal,
            should_quit: false,
            focused_column: 0,
            selected_per_column,
            board,
            keymap: Keymap::new(),
            status_message: None,
        }
    }

    pub fn apply(&mut self, action: Action) {
        match action {
            Action::Noop => {}
            Action::Quit => self.should_quit = true,
            Action::FocusPrevColumn => {
                if self.focused_column > 0 {
                    self.focused_column -= 1;
                }
            }
            Action::FocusNextColumn => {
                let max = self.board.project.states.len().saturating_sub(1);
                if self.focused_column < max {
                    self.focused_column += 1;
                }
            }
            Action::SelectPrevTask => self.move_selection(-1),
            Action::SelectNextTask => self.move_selection(1),
            Action::SelectFirstTask => {
                if !self.current_tasks().is_empty() {
                    self.selected_per_column[self.focused_column] = Some(0);
                }
            }
            Action::SelectLastTask => {
                let len = self.current_tasks().len();
                if len > 0 {
                    self.selected_per_column[self.focused_column] = Some(len - 1);
                }
            }
            Action::ToggleHelp => {
                self.status_message = Some("help overlay not implemented yet".to_owned());
            }
        }
    }

    fn move_selection(&mut self, delta: i32) {
        let len = self.current_tasks().len();
        if len == 0 {
            self.selected_per_column[self.focused_column] = None;
            return;
        }
        let current = self.selected_per_column[self.focused_column].unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len as i32 - 1) as usize;
        self.selected_per_column[self.focused_column] = Some(next);
    }

    pub fn current_tasks(&self) -> &[Task] {
        self.board
            .tasks_by_state
            .get(self.focused_column)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn selected_task(&self) -> Option<&Task> {
        let idx = self
            .selected_per_column
            .get(self.focused_column)
            .copied()??;
        self.current_tasks().get(idx)
    }
}

/// Main entry point — owns the terminal and the event loop.
pub async fn run<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> CoreResult<()> {
    let mut events = Events::start(TICK);

    loop {
        terminal
            .draw(|frame| view::render(frame, app))
            .map_err(io_to_core)?;

        if app.should_quit {
            break;
        }

        let Some(event) = events.next().await else {
            break;
        };

        match event {
            AppEvent::Key(key) => {
                let action = app.keymap.dispatch(key);
                app.apply(action);
            }
            AppEvent::Resize => {}
            AppEvent::Tick => {}
        }
    }

    Ok(())
}

fn io_to_core(err: std::io::Error) -> CoreError {
    CoreError::storage("terminal draw failed", err)
}

/// Load the board snapshot for `project`. Separated so tests can call it
/// without any terminal setup.
pub async fn load_board(
    projects: &impl ProjectRepository,
    tasks: &impl TaskRepository,
    project: Project,
) -> CoreResult<BoardSnapshot> {
    let mut tasks_by_state = Vec::with_capacity(project.states.len());
    for state in &project.states {
        let mut in_state = tasks.list_by_state(state.id).await?;
        in_state.sort_by_key(|t| t.position);
        tasks_by_state.push(in_state);
    }
    // Silence unused-variable warning in read-only builds where projects is
    // carried for parity with future refresh paths.
    let _ = projects;
    Ok(BoardSnapshot {
        project,
        tasks_by_state,
    })
}

/// Seed a demo project when the DB has none, so first-run users see a board.
pub async fn seed_demo_if_empty(
    pool: SqlitePool,
    clock: SystemClock,
    ids: impl IdGenerator + Copy,
) -> CoreResult<Project> {
    let project_repo = SqliteProjectRepo::new(pool.clone());
    let existing = project_repo.list().await?;
    if let Some(first) = existing.into_iter().next() {
        return Ok(first);
    }

    let project_service = ProjectService::new(SqliteProjectRepo::new(pool.clone()), clock, ids);
    let task_service = TaskService::new(
        SqliteProjectRepo::new(pool.clone()),
        SqliteTaskRepo::new(pool.clone()),
        clock,
        ids,
    );

    let project = project_service
        .create(NewProject {
            name: "kantui".to_owned(),
            description: Some("Demo board — delete with `:delete-project`.".to_owned()),
            initial_states: vec!["Todo".into(), "Doing".into(), "Done".into()],
        })
        .await?;

    let states = &project.states;
    let todo = states[0].id;
    let doing = states[1].id;
    let done = states[2].id;

    for (state, title, prio) in [
        (todo, "Wire up event loop", Priority::High),
        (todo, "Read CLAUDE.md", Priority::Normal),
        (doing, "Render board with widgets", Priority::High),
        (done, "Scaffold workspace", Priority::Low),
    ] {
        let mut input = NewTask::new(project.id, state, title);
        input.priority = prio;
        task_service.create(input).await?;
    }

    Ok(project)
}

/// Format the current wall-clock UTC time as `HH:MM` without pulling in a
/// date/time crate.
#[must_use]
pub fn format_clock_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let hh = (secs / 3600) % 24;
    let mm = (secs / 60) % 60;
    format!("{hh:02}:{mm:02}")
}
