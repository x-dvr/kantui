//! Application state + top-level event loop.
//!
//! The app holds the on-screen board snapshot, the current mode, and the
//! pending-edit buffer used while the user is typing in Insert mode. All
//! persistence calls go through [`AppServices`] into `kantui_core` services.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kantui_core::{
    CoreError, CoreResult, IdGenerator, NewProject, NewTask, Priority, Project, ProjectId,
    ProjectRepository, ProjectService, StateId, StateSojourn, StatsService, Tag, TagId,
    TagRepository, TagService, Task, TaskId, TaskRepository, TaskService, Throughput,
};
use kantui_store::sqlite::{SqliteProjectRepo, SqliteTagRepo, SqliteTaskRepo};
use kantui_store::{SqlitePool, SystemClock, UuidV4};
use kantui_widgets::{InputState, ProjectEditorFocus, Theme};
use ratatui::Terminal;
use ratatui::backend::Backend;

use crate::config::Config;
use crate::controller;
use crate::event::{AppEvent, Events};
use crate::keymap::Keymap;
use crate::view;

/// Duration between background ticks driving the clock/refresh.
pub const TICK: Duration = Duration::from_millis(500);

/// UI mode. Normal drives navigation; prompt modes (Insert, Command, Search)
/// route keys into [`InputState`]; Jump shows two-letter labels and reads
/// exactly two characters to teleport the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
    Search,
    Jump,
    TagPicker,
    Dashboard,
    TaskDetail,
    ProjectPicker,
    ProjectEditor,
}

/// What the user is currently typing into in Insert mode.
#[derive(Debug, Clone)]
pub enum PendingEdit {
    /// Create a new task in `column`, positioned after `anchor` (or at the
    /// column head if `anchor` is `None`).
    NewTask {
        column: usize,
        anchor: Option<TaskId>,
    },
    /// Rename an existing task.
    RenameTask { column: usize, task_id: TaskId },
    /// Replace a task's description; empty input clears it.
    EditDescription { task_id: TaskId },
    /// Set a task's due date from a `YYYY-MM-DD` string; empty input clears it.
    EditDueDate { task_id: TaskId },
    /// Rename the project shown in the editor.
    EditProjectName { project_id: ProjectId },
    /// Replace the project description; empty input clears it.
    EditProjectDescription { project_id: ProjectId },
    /// Rename a state inside the project editor.
    RenameState { state_id: StateId },
    /// Replace a state's WIP limit. Empty input clears it; a positive integer
    /// sets it.
    SetStateWipLimit { state_id: StateId },
    /// Add a new state to the project shown in the editor.
    AddState { project_id: ProjectId },
    /// Create a new project from the picker. After submit, the new project
    /// becomes active.
    NewProject,
}

/// In-memory snapshot of what's on screen. The repository is queried once
/// per refresh, not per render.
pub struct BoardSnapshot {
    pub project: Project,
    /// Tasks indexed by the *position* of their state in `project.states`.
    /// Each [`Task`]'s `tags` field is populated at load time.
    pub tasks_by_state: Vec<Vec<Task>>,
    /// All tags defined in the DB, sorted by name. Used by the tag picker and
    /// to resolve [`TagId`] → [`Tag`] during rendering.
    pub all_tags: Vec<Tag>,
}

impl BoardSnapshot {
    #[must_use]
    pub fn tag_by_id(&self, id: TagId) -> Option<&Tag> {
        self.all_tags.iter().find(|t| t.id == id)
    }
}

pub struct App {
    pub mode: Mode,
    pub should_quit: bool,
    pub focused_column: usize,
    /// One selected-task index per column. Indices are into the currently
    /// *visible* task list (i.e. already filtered by [`Self::search_query`]).
    pub selected_per_column: Vec<Option<usize>>,
    pub board: BoardSnapshot,
    pub keymap: Keymap,
    pub status_message: Option<String>,
    pub input: InputState,
    pub pending_edit: Option<PendingEdit>,
    /// Active substring filter. `None` when no filter is applied; `Some("")`
    /// while Search mode is being entered but before any keystrokes.
    pub search_query: Option<String>,
    pub show_help: bool,
    pub jump: Option<JumpState>,
    /// Active tag-picker overlay. Populated when the user presses `t` on a
    /// selected task; consumed as they press a single-char label to toggle.
    pub tag_picker: Option<TagPickerState>,
    /// Most-recent dashboard snapshot, rebuilt each time the overlay is
    /// opened.
    pub dashboard: Option<DashboardSnapshot>,
    /// Snapshot for the TaskDetail overlay. Holds the id of the task being
    /// inspected and a cached per-state sojourn list (refreshed each time the
    /// overlay opens).
    pub task_detail: Option<TaskDetailSnapshot>,
    /// Active project picker overlay (list of projects + selection cursor).
    pub project_picker: Option<ProjectPickerSnapshot>,
    /// Active project editor overlay (working copy of one project).
    pub project_editor: Option<ProjectEditorSnapshot>,
    /// Rendering palette — resolved from config at startup.
    pub theme: Theme,
}

/// Cached state behind the project-picker overlay.
#[derive(Debug, Clone)]
pub struct ProjectPickerSnapshot {
    pub projects: Vec<Project>,
    /// Total tasks per project (parallel to `projects`).
    pub task_counts: Vec<u32>,
    pub selected: usize,
}

/// Cached state behind the project-editor overlay.
#[derive(Debug, Clone)]
pub struct ProjectEditorSnapshot {
    pub project: Project,
    /// Task count per state (parallel to `project.states`).
    pub state_task_counts: Vec<u32>,
    pub focus: ProjectEditorFocus,
    /// When `true`, closing the editor returns to the picker; otherwise it
    /// returns to Normal mode.
    pub return_to_picker: bool,
}

/// State cached while the TaskDetail overlay is open.
#[derive(Debug, Clone)]
pub struct TaskDetailSnapshot {
    pub task_id: TaskId,
    /// `(state_id, duration)` pairs, ordered to match the project's state
    /// order (so the detail panel renders them top-to-bottom naturally).
    pub sojourn: Vec<(StateId, std::time::Duration)>,
}

/// Stats rendered by the Dashboard overlay. Assembled from
/// [`StatsService::project_sojourns`] and [`StatsService::throughput`].
#[derive(Debug, Clone)]
pub struct DashboardSnapshot {
    pub sojourns: Vec<StateSojourn>,
    pub throughput: Throughput,
    /// The state used as "done" — stored so the view can skip rebuild math.
    pub done_state: StateId,
}

/// State for the tag-picker overlay. Each row maps a single-character label
/// to a tag and records whether the currently-selected task has that tag
/// attached at the moment the picker was opened.
#[derive(Debug, Clone)]
pub struct TagPickerState {
    pub target_task: TaskId,
    pub rows: Vec<TagPickerRow>,
}

#[derive(Debug, Clone)]
pub struct TagPickerRow {
    pub label: char,
    pub tag: Tag,
    pub attached: bool,
}

/// Active jump-label overlay. Populated when the user presses `gw` and
/// consumed as they type the two-character label.
#[derive(Debug, Clone)]
pub struct JumpState {
    /// `(column, visible_index) -> [first, second]` label characters.
    pub labels: Vec<JumpLabel>,
    /// First character already typed, if any.
    pub pending_prefix: Option<char>,
}

#[derive(Debug, Clone, Copy)]
pub struct JumpLabel {
    pub column: usize,
    /// Index into the column's *visible* (filtered) task list.
    pub visible_index: usize,
    pub label: [char; 2],
}

impl App {
    pub fn new(board: BoardSnapshot) -> Self {
        Self::with_config(board, &Config::default())
    }

    pub fn with_config(board: BoardSnapshot, config: &Config) -> Self {
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
            keymap: Keymap::with_binds(config.keybinds.clone()),
            status_message: None,
            input: InputState::new(),
            pending_edit: None,
            search_query: None,
            show_help: false,
            jump: None,
            tag_picker: None,
            dashboard: None,
            task_detail: None,
            project_picker: None,
            project_editor: None,
            theme: config.theme,
        }
    }

    /// Tasks visible in `column` after applying the active filter.
    ///
    /// Filter syntax: a leading `#` filters by tag name (case-insensitive
    /// substring of any attached tag); otherwise the query is matched against
    /// the task title.
    #[must_use]
    pub fn visible_tasks(&self, column: usize) -> Vec<&Task> {
        let full = self
            .board
            .tasks_by_state
            .get(column)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        match self.active_filter() {
            Some(q) => match q.strip_prefix('#') {
                Some(tag_query) => full
                    .iter()
                    .filter(|t| task_matches_tag(t, &self.board, tag_query))
                    .collect(),
                None => full.iter().filter(|t| title_matches(&t.title, q)).collect(),
            },
            None => full.iter().collect(),
        }
    }

    /// Visible tasks in the currently focused column.
    #[must_use]
    pub fn current_tasks(&self) -> Vec<&Task> {
        self.visible_tasks(self.focused_column)
    }

    #[must_use]
    pub fn selected_task(&self) -> Option<&Task> {
        let idx = self
            .selected_per_column
            .get(self.focused_column)
            .copied()??;
        self.current_tasks().get(idx).copied()
    }

    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_per_column
            .get(self.focused_column)
            .copied()
            .flatten()
    }

    /// The filter that should be applied to the board. Returns `None` when
    /// no filter is active (or the query is empty).
    #[must_use]
    pub fn active_filter(&self) -> Option<&str> {
        self.search_query.as_deref().filter(|q| !q.is_empty())
    }

    /// Re-resolve the selected index per column against the currently-visible
    /// task lists, preserving the selected [`TaskId`] when possible. Falls
    /// back to the first visible task if the previous selection is filtered
    /// out, or `None` when the column is empty.
    pub fn refresh_selection(&mut self, previous: &[Option<TaskId>]) {
        for column in 0..self.board.tasks_by_state.len() {
            let (new_idx, empty) = {
                let visible = self.visible_tasks(column);
                let prev_id = previous.get(column).copied().flatten();
                let idx = prev_id.and_then(|id| visible.iter().position(|t| t.id == id));
                (idx, visible.is_empty())
            };
            let slot = &mut self.selected_per_column[column];
            *slot = match (new_idx, empty) {
                (Some(i), _) => Some(i),
                (None, true) => None,
                (None, false) => Some(0),
            };
        }
    }

    /// Snapshot of the currently-selected [`TaskId`] per column, useful as an
    /// argument to [`Self::refresh_selection`] after mutating the filter.
    #[must_use]
    pub fn selection_snapshot(&self) -> Vec<Option<TaskId>> {
        (0..self.board.tasks_by_state.len())
            .map(|column| {
                let idx = self.selected_per_column.get(column).copied().flatten()?;
                self.visible_tasks(column).get(idx).map(|t| t.id)
            })
            .collect()
    }

    /// Move the selection within the focused column by `delta`, clamped to
    /// the column bounds. A no-op if the column is empty.
    pub fn move_selection(&mut self, delta: i32) {
        let len = self.current_tasks().len();
        if len == 0 {
            self.selected_per_column[self.focused_column] = None;
            return;
        }
        let current = self.selected_per_column[self.focused_column].unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len as i32 - 1) as usize;
        self.selected_per_column[self.focused_column] = Some(next);
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    /// Enter Insert mode with the given pending edit. `initial` seeds the
    /// input buffer (empty for new tasks, existing title for renames).
    pub fn enter_insert(&mut self, edit: PendingEdit, initial: &str) {
        self.mode = Mode::Insert;
        self.pending_edit = Some(edit);
        self.input = InputState::with_value(initial);
    }

    /// Cancel any in-flight insert-mode edit and return to Normal. Does not
    /// touch Command/Search state — those have their own reset paths.
    pub fn leave_insert(&mut self) {
        self.mode = Mode::Normal;
        self.pending_edit = None;
        self.input.clear();
    }

    /// Enter Command mode with an empty input buffer.
    pub fn enter_command(&mut self) {
        self.mode = Mode::Command;
        self.pending_edit = None;
        self.input = InputState::new();
    }

    /// Enter Search mode. The filter starts empty; every subsequent key
    /// refreshes `search_query` live.
    pub fn enter_search(&mut self) {
        self.mode = Mode::Search;
        self.pending_edit = None;
        self.input = InputState::new();
        self.search_query = Some(String::new());
    }

    /// Clear any prompt state and return to Normal mode.
    pub fn leave_prompt(&mut self) {
        self.mode = Mode::Normal;
        self.pending_edit = None;
        self.input.clear();
    }

    /// After a task is inserted into the snapshot, clamp the selection to
    /// the newly inserted index.
    pub fn select_in_column(&mut self, column: usize, index: usize) {
        if let Some(slot) = self.selected_per_column.get_mut(column) {
            *slot = Some(index);
        }
    }

    /// Open the tag picker for the currently-selected task. Returns `false`
    /// if no task is selected or no tags exist yet.
    pub fn enter_tag_picker(&mut self) -> bool {
        let Some(task) = self.selected_task() else {
            self.set_status("no task selected");
            return false;
        };
        if self.board.all_tags.is_empty() {
            self.set_status("no tags yet — use `:tag-new <name>`");
            return false;
        }
        let target_task = task.id;
        let attached: std::collections::HashSet<TagId> = task.tags.iter().copied().collect();
        let rows: Vec<TagPickerRow> = self
            .board
            .all_tags
            .iter()
            .take(TAG_PICKER_LABELS.len())
            .zip(TAG_PICKER_LABELS.chars())
            .map(|(tag, label)| TagPickerRow {
                label,
                tag: tag.clone(),
                attached: attached.contains(&tag.id),
            })
            .collect();
        self.tag_picker = Some(TagPickerState { target_task, rows });
        self.mode = Mode::TagPicker;
        true
    }

    pub fn leave_tag_picker(&mut self) {
        self.tag_picker = None;
        self.mode = Mode::Normal;
    }
}

/// Single-character labels used for the tag picker rows (up to 26 tags).
const TAG_PICKER_LABELS: &str = "abcdefghijklmnopqrstuvwxyz";

fn title_matches(title: &str, query: &str) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return true;
    }
    title.to_lowercase().contains(&q.to_lowercase())
}

fn task_matches_tag(task: &Task, board: &BoardSnapshot, query: &str) -> bool {
    let q = query.trim();
    // `#` with no text matches any task that carries at least one tag.
    if q.is_empty() {
        return !task.tags.is_empty();
    }
    let q = q.to_lowercase();
    task.tags.iter().any(|id| {
        board
            .tag_by_id(*id)
            .is_some_and(|tag| tag.name.to_lowercase().contains(&q))
    })
}

/// Services wired at the composition root. Cheap to clone because repos are
/// thin wrappers around `Arc<SqlitePool>` and the clock/id generators are
/// zero-sized.
#[derive(Clone)]
pub struct AppServices {
    pool: SqlitePool,
    clock: SystemClock,
    ids: UuidV4,
    /// Path of the UI-state file. Used to persist e.g. last opened project.
    state_path: PathBuf,
}

impl AppServices {
    #[must_use]
    pub fn new(pool: SqlitePool, clock: SystemClock, ids: UuidV4, state_path: PathBuf) -> Self {
        Self {
            pool,
            clock,
            ids,
            state_path,
        }
    }

    #[must_use]
    pub fn state_path(&self) -> &std::path::Path {
        &self.state_path
    }

    #[must_use]
    pub fn project_repo(&self) -> SqliteProjectRepo {
        SqliteProjectRepo::new(self.pool.clone())
    }

    #[must_use]
    pub fn task_repo(&self) -> SqliteTaskRepo {
        SqliteTaskRepo::new(self.pool.clone())
    }

    #[must_use]
    pub fn tag_repo(&self) -> SqliteTagRepo {
        SqliteTagRepo::new(self.pool.clone())
    }

    #[must_use]
    pub fn task_service(
        &self,
    ) -> TaskService<SqliteProjectRepo, SqliteTaskRepo, SystemClock, UuidV4> {
        TaskService::new(self.project_repo(), self.task_repo(), self.clock, self.ids)
    }

    #[must_use]
    pub fn tag_service(&self) -> TagService<SqliteTagRepo, UuidV4> {
        TagService::new(self.tag_repo(), self.ids)
    }

    #[must_use]
    pub fn stats_service(&self) -> StatsService<SqliteTaskRepo, SystemClock> {
        StatsService::new(self.task_repo(), self.clock)
    }
}

/// Main entry point — owns the terminal and the event loop.
pub async fn run<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    services: &AppServices,
) -> CoreResult<()> {
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
                let action = app.keymap.dispatch(app.mode, key);
                if let Err(err) = controller::process(action, app, services).await {
                    tracing::error!("{}", err.log_chain());
                    app.set_status(err.to_string());
                }
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
    tags: &impl TagRepository,
    project: Project,
) -> CoreResult<BoardSnapshot> {
    let mut tasks_by_state = Vec::with_capacity(project.states.len());
    for state in &project.states {
        let mut in_state = tasks.list_by_state(state.id).await?;
        in_state.sort_by_key(|t| t.position);
        for task in &mut in_state {
            task.tags = tags.list_for_task(task.id).await?;
        }
        tasks_by_state.push(in_state);
    }
    let all_tags = tags.list().await?;
    let _ = projects;
    Ok(BoardSnapshot {
        project,
        tasks_by_state,
        all_tags,
    })
}

/// Ensure the DB has at least one project. If the DB is empty, create a blank
/// `kantui` project with `Todo` / `Doing` / `Done` columns and no tasks so
/// the UI has somewhere to render.
pub async fn ensure_default_project(
    pool: SqlitePool,
    clock: SystemClock,
    ids: impl IdGenerator + Copy,
) -> CoreResult<Project> {
    let project_repo = SqliteProjectRepo::new(pool.clone());
    if let Some(first) = project_repo.list().await?.into_iter().next() {
        return Ok(first);
    }

    let project_service = ProjectService::new(SqliteProjectRepo::new(pool.clone()), clock, ids);
    project_service
        .create(NewProject {
            name: "kantui".to_owned(),
            description: None,
            initial_states: vec!["Todo".into(), "Doing".into(), "Done".into()],
        })
        .await
}

/// Seed a demo project when the DB has none, so first-run users see a board.
pub async fn seed_demo_if_empty(
    pool: SqlitePool,
    clock: SystemClock,
    ids: impl IdGenerator + Copy,
) -> CoreResult<Project> {
    let project_repo = SqliteProjectRepo::new(pool.clone());
    if let Some(first) = project_repo.list().await?.into_iter().next() {
        return Ok(first);
    }

    let project = ensure_default_project(pool.clone(), clock, ids).await?;

    let task_service = TaskService::new(
        SqliteProjectRepo::new(pool.clone()),
        SqliteTaskRepo::new(pool.clone()),
        clock,
        ids,
    );

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
