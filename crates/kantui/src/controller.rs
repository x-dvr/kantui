//! Action dispatcher. Turns [`Action`] values into state changes on [`App`]
//! and, when needed, into async calls against the core services. Errors are
//! surfaced to the caller; the event loop logs + displays them.

use kantui_core::{
    Color, CoreError, CoreResult, NewState, NewTask, Priority, ProjectRepository, ProjectService,
    StateId, TagRepository, Task, TaskId, TaskRepository, TaskUpdate,
};

use crate::action::Action;
use crate::app::{App, AppServices, JumpState, Mode, PendingEdit};
use crate::jump;

/// Apply `action` to `app`, calling into `services` for any write that
/// needs to hit storage.
pub async fn process(action: Action, app: &mut App, services: &AppServices) -> CoreResult<()> {
    // Most actions clear any stale status; Noop keeps the previous message
    // so it stays visible until the next meaningful key.
    if !matches!(action, Action::Noop) {
        app.clear_status();
    }

    match action {
        Action::Noop => Ok(()),
        Action::Quit => {
            app.should_quit = true;
            Ok(())
        }

        Action::FocusPrevColumn => {
            if app.focused_column > 0 {
                app.focused_column -= 1;
            }
            Ok(())
        }
        Action::FocusNextColumn => {
            let max = app.board.project.states.len().saturating_sub(1);
            if app.focused_column < max {
                app.focused_column += 1;
            }
            Ok(())
        }
        Action::SelectPrevTask => {
            app.move_selection(-1);
            Ok(())
        }
        Action::SelectNextTask => {
            app.move_selection(1);
            Ok(())
        }
        Action::SelectFirstTask => {
            if !app.current_tasks().is_empty() {
                app.selected_per_column[app.focused_column] = Some(0);
            }
            Ok(())
        }
        Action::SelectLastTask => {
            let len = app.current_tasks().len();
            if len > 0 {
                app.selected_per_column[app.focused_column] = Some(len - 1);
            }
            Ok(())
        }

        Action::BeginNewTaskBelow => {
            let anchor = app.selected_task().map(|t| t.id);
            app.enter_insert(
                PendingEdit::NewTask {
                    column: app.focused_column,
                    anchor,
                },
                "",
            );
            Ok(())
        }
        Action::BeginNewTaskAbove => {
            let anchor = match app.selected_index() {
                Some(i) if i > 0 => app.current_tasks().get(i - 1).map(|t| t.id),
                _ => None,
            };
            app.enter_insert(
                PendingEdit::NewTask {
                    column: app.focused_column,
                    anchor,
                },
                "",
            );
            Ok(())
        }
        Action::BeginRenameTask => {
            let Some(task) = app.selected_task() else {
                app.set_status("no task selected");
                return Ok(());
            };
            let edit = PendingEdit::RenameTask {
                column: app.focused_column,
                task_id: task.id,
            };
            let initial = task.title.clone();
            app.enter_insert(edit, &initial);
            Ok(())
        }

        Action::DeleteTask => delete_selected(app, services).await,
        Action::MoveTaskPrevColumn => move_across_column(app, services, -1).await,
        Action::MoveTaskNextColumn => move_across_column(app, services, 1).await,
        Action::ShiftTaskUp => shift_within_column(app, services, -1).await,
        Action::ShiftTaskDown => shift_within_column(app, services, 1).await,

        Action::InsertChar(ch) => {
            app.input.insert_char(ch);
            if app.mode == Mode::Search {
                update_search(app);
            }
            Ok(())
        }
        Action::InsertBackspace => {
            app.input.backspace();
            if app.mode == Mode::Search {
                update_search(app);
            }
            Ok(())
        }
        Action::InsertDelete => {
            app.input.delete();
            if app.mode == Mode::Search {
                update_search(app);
            }
            Ok(())
        }
        Action::InsertMoveLeft => {
            app.input.move_left();
            Ok(())
        }
        Action::InsertMoveRight => {
            app.input.move_right();
            Ok(())
        }
        Action::InsertMoveHome => {
            app.input.move_to_start();
            Ok(())
        }
        Action::InsertMoveEnd => {
            app.input.move_to_end();
            Ok(())
        }
        Action::InsertCancel => cancel_prompt(app),
        Action::InsertSubmit => submit_prompt(app, services).await,

        Action::BeginCommand => {
            app.enter_command();
            Ok(())
        }
        Action::BeginSearch => {
            app.enter_search();
            Ok(())
        }
        Action::BeginJump => begin_jump(app),
        Action::ToggleHelp => {
            app.show_help = !app.show_help;
            Ok(())
        }
        Action::Escape => {
            if app.show_help {
                app.show_help = false;
            }
            Ok(())
        }

        Action::JumpChar(ch) => jump_char(app, ch),
        Action::JumpCancel => {
            app.jump = None;
            app.mode = Mode::Normal;
            Ok(())
        }

        Action::BeginTagPicker => {
            app.enter_tag_picker();
            Ok(())
        }
        Action::TagPickerChar(ch) => tag_picker_char(app, services, ch).await,
        Action::TagPickerCancel => {
            app.leave_tag_picker();
            Ok(())
        }

        Action::OpenDashboard => open_dashboard(app, services).await,
        Action::CloseDashboard => {
            app.dashboard = None;
            app.mode = Mode::Normal;
            Ok(())
        }
    }
}

// -----------------------------------------------------------------------
// Prompt handling
// -----------------------------------------------------------------------

fn cancel_prompt(app: &mut App) -> CoreResult<()> {
    match app.mode {
        Mode::Search => {
            // Clearing the filter restores the pre-search selection.
            let snapshot = app.selection_snapshot();
            app.search_query = None;
            app.refresh_selection(&snapshot);
            app.leave_prompt();
            Ok(())
        }
        Mode::Command | Mode::Insert => {
            app.leave_prompt();
            Ok(())
        }
        _ => Ok(()),
    }
}

async fn submit_prompt(app: &mut App, services: &AppServices) -> CoreResult<()> {
    match app.mode {
        Mode::Insert => submit_insert(app, services).await,
        Mode::Command => {
            let line = app.input.value().to_owned();
            app.leave_prompt();
            execute_command(app, services, &line).await
        }
        Mode::Search => {
            // Enter commits the filter; it stays applied until `/` is pressed
            // again or a reset command clears it.
            app.leave_prompt();
            Ok(())
        }
        _ => Ok(()),
    }
}

async fn submit_insert(app: &mut App, services: &AppServices) -> CoreResult<()> {
    let Some(edit) = app.pending_edit.clone() else {
        app.leave_insert();
        return Ok(());
    };
    let title = app.input.value().trim().to_owned();
    if title.is_empty() {
        app.set_status("title must not be empty");
        return Ok(());
    }

    match edit {
        PendingEdit::NewTask { column, anchor } => {
            create_task(app, services, column, anchor, title).await
        }
        PendingEdit::RenameTask { column, task_id } => {
            rename_task(app, services, column, task_id, title).await
        }
    }
}

fn update_search(app: &mut App) {
    let snapshot = app.selection_snapshot();
    app.search_query = Some(app.input.value().to_owned());
    app.refresh_selection(&snapshot);
}

// -----------------------------------------------------------------------
// Jump-mode handling
// -----------------------------------------------------------------------

fn begin_jump(app: &mut App) -> CoreResult<()> {
    let labels = jump::generate(app);
    if labels.is_empty() {
        app.set_status("no visible tasks to jump to");
        return Ok(());
    }
    app.jump = Some(JumpState {
        labels,
        pending_prefix: None,
    });
    app.mode = Mode::Jump;
    Ok(())
}

fn jump_char(app: &mut App, ch: char) -> CoreResult<()> {
    let Some(jump_state) = app.jump.as_mut() else {
        app.mode = Mode::Normal;
        return Ok(());
    };

    match jump_state.pending_prefix {
        None => {
            if jump_state.labels.iter().any(|l| l.label[0] == ch) {
                jump_state.pending_prefix = Some(ch);
                Ok(())
            } else {
                app.jump = None;
                app.mode = Mode::Normal;
                app.set_status(format!("no jump target for '{ch}'"));
                Ok(())
            }
        }
        Some(first) => {
            let target = jump_state
                .labels
                .iter()
                .find(|l| l.label == [first, ch])
                .copied();
            app.jump = None;
            app.mode = Mode::Normal;
            match target {
                Some(label) => {
                    app.focused_column = label.column;
                    app.selected_per_column[label.column] = Some(label.visible_index);
                    Ok(())
                }
                None => {
                    app.set_status(format!("no jump target for '{first}{ch}'"));
                    Ok(())
                }
            }
        }
    }
}

// -----------------------------------------------------------------------
// Task mutations
// -----------------------------------------------------------------------

async fn create_task(
    app: &mut App,
    services: &AppServices,
    column: usize,
    anchor: Option<TaskId>,
    title: String,
) -> CoreResult<()> {
    let state = column_state_id(app, column)?;
    let project_id = app.board.project.id;

    let svc = services.task_service();
    let mut new_task = NewTask::new(project_id, state, title);
    new_task.priority = Priority::Normal;
    let created = svc.create(new_task).await?;

    // If no anchor, append-after-last is exactly what `create` did. Otherwise
    // move the newly created task to just after the anchor.
    if let Some(anchor_id) = anchor {
        svc.move_task(created.id, state, Some(anchor_id)).await?;
    }

    reload_column(app, services, column).await?;
    update_selection(app, column, Some(created.id));
    app.leave_insert();
    Ok(())
}

async fn rename_task(
    app: &mut App,
    services: &AppServices,
    column: usize,
    task_id: TaskId,
    title: String,
) -> CoreResult<()> {
    let svc = services.task_service();
    let update = TaskUpdate {
        title: Some(title),
        ..Default::default()
    };
    let updated = svc.update(task_id, update).await?;

    if let Some(col_tasks) = app.board.tasks_by_state.get_mut(column)
        && let Some(slot) = col_tasks.iter_mut().find(|t| t.id == updated.id)
    {
        *slot = updated;
    }
    app.leave_insert();
    Ok(())
}

async fn delete_selected(app: &mut App, services: &AppServices) -> CoreResult<()> {
    let Some(task) = app.selected_task() else {
        app.set_status("no task selected");
        return Ok(());
    };
    let task_id = task.id;
    let column = app.focused_column;

    let svc = services.task_service();
    svc.delete(task_id).await?;

    if let Some(col_tasks) = app.board.tasks_by_state.get_mut(column) {
        col_tasks.retain(|t| t.id != task_id);
    }
    update_selection(app, column, None);
    Ok(())
}

async fn move_across_column(app: &mut App, services: &AppServices, delta: i32) -> CoreResult<()> {
    let Some(task) = app.selected_task() else {
        app.set_status("no task selected");
        return Ok(());
    };
    let task_id = task.id;
    let from = app.focused_column;
    let to = (from as i32 + delta).clamp(0, app.board.project.states.len() as i32 - 1) as usize;
    if to == from {
        return Ok(());
    }
    let target_state = column_state_id(app, to)?;

    let svc = services.task_service();
    svc.move_task(task_id, target_state, None).await?;

    reload_column(app, services, from).await?;
    reload_column(app, services, to).await?;

    update_selection(app, from, None);
    app.focused_column = to;
    update_selection(app, to, Some(task_id));
    Ok(())
}

async fn shift_within_column(app: &mut App, services: &AppServices, delta: i32) -> CoreResult<()> {
    let column = app.focused_column;
    let Some(idx) = app.selected_index() else {
        app.set_status("no task selected");
        return Ok(());
    };

    let (task_id, anchor) = {
        let tasks = app.current_tasks();
        let len = tasks.len();
        if len < 2 {
            return Ok(());
        }
        let target = (idx as i32 + delta).clamp(0, len as i32 - 1) as usize;
        if target == idx {
            return Ok(());
        }
        // The anchor is the task we want to sit *after*. Moving up means the
        // anchor is at `target - 1` (or None if moving to the top); moving
        // down means the anchor is the task currently at `target`.
        let task_id = tasks[idx].id;
        let anchor = if delta < 0 {
            if target == 0 {
                None
            } else {
                Some(tasks[target - 1].id)
            }
        } else {
            Some(tasks[target].id)
        };
        (task_id, anchor)
    };

    let state_id = column_state_id(app, column)?;
    let svc = services.task_service();
    svc.move_task(task_id, state_id, anchor).await?;

    reload_column(app, services, column).await?;
    update_selection(app, column, Some(task_id));
    Ok(())
}

// -----------------------------------------------------------------------
// Command execution
// -----------------------------------------------------------------------

async fn execute_command(app: &mut App, services: &AppServices, line: &str) -> CoreResult<()> {
    let mut parts = line.trim().splitn(2, char::is_whitespace);
    let Some(cmd) = parts.next() else {
        return Ok(());
    };
    let rest = parts.next().unwrap_or("").trim();

    match cmd {
        "" => Ok(()),
        "q" | "quit" | "wq" => {
            app.should_quit = true;
            Ok(())
        }
        "help" | "h" => {
            app.show_help = true;
            Ok(())
        }
        "close-help" => {
            app.show_help = false;
            Ok(())
        }
        "new-state" => cmd_new_state(app, services, rest).await,
        "rename-state" => cmd_rename_state(app, services, rest).await,
        "delete-state" => cmd_delete_state(app, services).await,
        "new-task" => cmd_new_task(app, services, rest).await,
        "tag-new" => cmd_tag_new(app, services, rest).await,
        "tag-delete" => cmd_tag_delete(app, services, rest).await,
        "stats" | "dashboard" => open_dashboard(app, services).await,
        other => {
            app.set_status(format!("unknown command: {other}"));
            Ok(())
        }
    }
}

async fn cmd_new_state(app: &mut App, services: &AppServices, name: &str) -> CoreResult<()> {
    if name.is_empty() {
        app.set_status("usage: :new-state <name>");
        return Ok(());
    }
    let project_svc = project_service(services);
    project_svc
        .add_state(
            app.board.project.id,
            NewState {
                name: name.to_owned(),
                wip_limit: None,
            },
        )
        .await?;
    reload_board(app, services).await
}

async fn cmd_rename_state(app: &mut App, services: &AppServices, name: &str) -> CoreResult<()> {
    if name.is_empty() {
        app.set_status("usage: :rename-state <name>");
        return Ok(());
    }
    let state_id = column_state_id(app, app.focused_column)?;
    let project_svc = project_service(services);
    project_svc.rename_state(state_id, name).await?;
    reload_board(app, services).await
}

async fn cmd_delete_state(app: &mut App, services: &AppServices) -> CoreResult<()> {
    let column = app.focused_column;
    let state_id = column_state_id(app, column)?;
    let full = app
        .board
        .tasks_by_state
        .get(column)
        .map(Vec::len)
        .unwrap_or(0);
    if full > 0 {
        app.set_status("state is not empty; delete its tasks first");
        return Ok(());
    }
    if app.board.project.states.len() <= 1 {
        app.set_status("cannot delete the only state");
        return Ok(());
    }
    let project_svc = project_service(services);
    project_svc.remove_state(state_id).await?;
    reload_board(app, services).await?;
    if app.focused_column >= app.board.project.states.len() {
        app.focused_column = app.board.project.states.len().saturating_sub(1);
    }
    Ok(())
}

async fn cmd_new_task(app: &mut App, services: &AppServices, title: &str) -> CoreResult<()> {
    if title.is_empty() {
        app.set_status("usage: :new-task <title>");
        return Ok(());
    }
    create_task(app, services, app.focused_column, None, title.to_owned()).await
}

/// `:tag-new <name> [color]` — create a new tag. `color` is optional; when
/// omitted the tag uses [`Color::White`].
async fn cmd_tag_new(app: &mut App, services: &AppServices, args: &str) -> CoreResult<()> {
    let mut parts = args.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or("").trim();
    if name.is_empty() {
        app.set_status("usage: :tag-new <name> [color]");
        return Ok(());
    }
    let color = match parts.next().map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => match parse_color(raw) {
            Some(c) => c,
            None => {
                app.set_status(format!("unknown color '{raw}'"));
                return Ok(());
            }
        },
        None => Color::White,
    };
    let svc = services.tag_service();
    let tag = svc.create(name, color).await?;
    app.board.all_tags.push(tag);
    app.board.all_tags.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(())
}

/// `:tag-delete <name>` — delete a tag globally. Detaches from all tasks
/// via the adapter's foreign-key cascade.
async fn cmd_tag_delete(app: &mut App, services: &AppServices, args: &str) -> CoreResult<()> {
    let name = args.trim();
    if name.is_empty() {
        app.set_status("usage: :tag-delete <name>");
        return Ok(());
    }
    let svc = services.tag_service();
    let tag = match services.tag_repo().find_by_name(name).await? {
        Some(t) => t,
        None => {
            app.set_status(format!("no tag named '{name}'"));
            return Ok(());
        }
    };
    svc.delete(tag.id).await?;
    reload_board(app, services).await
}

fn parse_color(raw: &str) -> Option<Color> {
    match raw.to_ascii_lowercase().as_str() {
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "white" => Some(Color::White),
        "gray" | "grey" => Some(Color::Gray),
        _ => None,
    }
}

// -----------------------------------------------------------------------
// Dashboard
// -----------------------------------------------------------------------

/// Rolling window (in days) shown by the throughput chart.
const DASHBOARD_THROUGHPUT_DAYS: u32 = 14;

async fn open_dashboard(app: &mut App, services: &AppServices) -> CoreResult<()> {
    let project_id = app.board.project.id;
    // Convention: the last column is "done". Empty projects (no states) have
    // no sensible dashboard.
    let Some(done_state) = app.board.project.states.last().map(|s| s.id) else {
        app.set_status("project has no states");
        return Ok(());
    };

    let stats = services.stats_service();
    let sojourns = stats.project_sojourns(project_id).await?;
    let throughput = stats
        .throughput(project_id, done_state, DASHBOARD_THROUGHPUT_DAYS)
        .await?;

    app.dashboard = Some(crate::app::DashboardSnapshot {
        sojourns,
        throughput,
        done_state,
    });
    app.mode = Mode::Dashboard;
    Ok(())
}

// -----------------------------------------------------------------------
// Tag picker
// -----------------------------------------------------------------------

async fn tag_picker_char(app: &mut App, services: &AppServices, ch: char) -> CoreResult<()> {
    let Some(picker) = app.tag_picker.as_ref() else {
        app.mode = Mode::Normal;
        return Ok(());
    };
    let target_task = picker.target_task;
    let Some(row) = picker.rows.iter().find(|r| r.label == ch).cloned() else {
        app.set_status(format!("no tag for '{ch}'"));
        return Ok(());
    };

    let svc = services.tag_service();
    if row.attached {
        svc.detach(target_task, row.tag.id).await?;
    } else {
        svc.attach(target_task, row.tag.id).await?;
    }

    // Refresh only the owning column — cheap, preserves selection.
    let column = app.focused_column;
    reload_column(app, services, column).await?;
    update_selection(app, column, Some(target_task));

    // Flip the attached bit so the picker reflects the new state without
    // being rebuilt.
    if let Some(state) = app.tag_picker.as_mut()
        && let Some(r) = state.rows.iter_mut().find(|r| r.tag.id == row.tag.id)
    {
        r.attached = !row.attached;
    }
    Ok(())
}

fn project_service(
    services: &AppServices,
) -> ProjectService<
    kantui_store::sqlite::SqliteProjectRepo,
    kantui_store::SystemClock,
    kantui_store::UuidV4,
> {
    // Rebuild a ProjectService each call — all internals are cheap (Arc clones
    // + zero-sized clocks/ids), and keeping the service behind a factory lets
    // the rest of the module stay free of concrete adapter types.
    let repo = services.project_repo();
    ProjectService::new(
        repo,
        kantui_store::SystemClock::new(),
        kantui_store::UuidV4::new(),
    )
}

// -----------------------------------------------------------------------
// Shared helpers
// -----------------------------------------------------------------------

fn column_state_id(app: &App, column: usize) -> CoreResult<StateId> {
    app.board
        .project
        .states
        .get(column)
        .map(|s| s.id)
        .ok_or_else(|| CoreError::validation("column index out of range"))
}

async fn reload_column(app: &mut App, services: &AppServices, column: usize) -> CoreResult<()> {
    let state_id = column_state_id(app, column)?;
    let repo = services.task_repo();
    let tag_repo = services.tag_repo();
    let mut tasks: Vec<Task> = repo.list_by_state(state_id).await?;
    tasks.sort_by_key(|t| t.position);
    for task in &mut tasks {
        task.tags = tag_repo.list_for_task(task.id).await?;
    }
    if let Some(slot) = app.board.tasks_by_state.get_mut(column) {
        *slot = tasks;
    }
    Ok(())
}

async fn reload_board(app: &mut App, services: &AppServices) -> CoreResult<()> {
    let project = services
        .project_repo()
        .get(app.board.project.id)
        .await?
        .ok_or_else(|| CoreError::validation("active project vanished"))?;
    let mut tasks_by_state = Vec::with_capacity(project.states.len());
    let repo = services.task_repo();
    let tag_repo = services.tag_repo();
    for state in &project.states {
        let mut in_state = repo.list_by_state(state.id).await?;
        in_state.sort_by_key(|t| t.position);
        for task in &mut in_state {
            task.tags = tag_repo.list_for_task(task.id).await?;
        }
        tasks_by_state.push(in_state);
    }
    app.board.project = project;
    app.board.tasks_by_state = tasks_by_state;
    app.board.all_tags = tag_repo.list().await?;
    app.selected_per_column
        .resize(app.board.tasks_by_state.len(), None);
    let snapshot: Vec<Option<TaskId>> = app.selection_snapshot();
    app.refresh_selection(&snapshot);
    Ok(())
}

/// Resolve the selection in `column` to `preferred` if it's visible, else
/// clamp to the end of the visible list. Also converges `selected_per_column`
/// when the column's task set has shrunk.
fn update_selection(app: &mut App, column: usize, preferred: Option<TaskId>) {
    let visible = app.visible_tasks(column);
    let new_idx = preferred.and_then(|id| visible.iter().position(|t| t.id == id));
    let len = visible.len();
    let slot = &mut app.selected_per_column[column];
    *slot = match (new_idx, len) {
        (Some(i), _) => Some(i),
        (None, 0) => None,
        (None, len) => slot.map(|i| i.min(len - 1)).or(Some(0)),
    };
}
