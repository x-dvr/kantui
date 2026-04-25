//! Action dispatcher. Turns [`Action`] values into state changes on [`App`]
//! and, when needed, into async calls against the core services. Errors are
//! surfaced to the caller; the event loop logs + displays them.

use kantui_core::{
    Color, CoreError, CoreResult, NewProject, NewState, NewTask, Priority, Project, ProjectId,
    ProjectRepository, ProjectService, StateId, TagRepository, Task, TaskId, TaskRepository,
    TaskUpdate,
};
use kantui_widgets::ProjectEditorFocus;

use crate::action::Action;
use crate::app::{
    App, AppServices, JumpState, Mode, PendingEdit, ProjectEditorSnapshot, ProjectPickerSnapshot,
};
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

        Action::OpenTaskDetail => open_task_detail(app, services).await,
        Action::CloseTaskDetail => {
            app.task_detail = None;
            app.mode = Mode::Normal;
            Ok(())
        }
        Action::CycleTaskPriority => cycle_priority(app, services).await,
        Action::CycleTaskComplexity => cycle_complexity(app, services).await,
        Action::BeginEditDescription => begin_edit_description(app),
        Action::BeginEditDueDate => begin_edit_due_date(app),

        Action::OpenProjectPicker => open_project_picker(app, services).await,
        Action::CloseProjectPicker => {
            app.project_picker = None;
            app.mode = Mode::Normal;
            Ok(())
        }
        Action::PickerSelectPrev => {
            picker_move(app, -1);
            Ok(())
        }
        Action::PickerSelectNext => {
            picker_move(app, 1);
            Ok(())
        }
        Action::PickerActivate => picker_activate(app, services).await,
        Action::PickerEditSelected => picker_edit_selected(app, services).await,
        Action::PickerNewProject => picker_new_project(app),
        Action::PickerDeleteSelected => picker_delete_selected(app, services).await,

        Action::CloseProjectEditor => close_project_editor(app, services).await,
        Action::EditorFocusPrev => {
            editor_focus_move(app, -1);
            Ok(())
        }
        Action::EditorFocusNext => {
            editor_focus_move(app, 1);
            Ok(())
        }
        Action::EditorBeginEdit => editor_begin_edit(app),
        Action::EditorBeginEditWip => editor_begin_edit_wip(app),
        Action::EditorAddState => editor_begin_add_state(app),
        Action::EditorDeleteState => editor_delete_state(app, services).await,
        Action::EditorShiftStateUp => editor_shift_state(app, services, -1).await,
        Action::EditorShiftStateDown => editor_shift_state(app, services, 1).await,
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
            // If the cancelled prompt belongs to the project editor / picker,
            // restore that overlay rather than returning to the board.
            let return_mode = prompt_return_mode(app);
            app.leave_prompt();
            app.mode = return_mode;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Decide which mode the prompt should return to once it's cancelled or
/// committed. Defaults to Normal; project-editor edits return to the editor,
/// new-project edits return to the picker, and task-detail edits stay in
/// detail.
fn prompt_return_mode(app: &App) -> Mode {
    match app.pending_edit {
        Some(
            PendingEdit::EditProjectName { .. }
            | PendingEdit::EditProjectDescription { .. }
            | PendingEdit::RenameState { .. }
            | PendingEdit::SetStateWipLimit { .. }
            | PendingEdit::AddState { .. },
        ) => Mode::ProjectEditor,
        Some(PendingEdit::NewProject) => {
            if app.project_picker.is_some() {
                Mode::ProjectPicker
            } else {
                Mode::Normal
            }
        }
        Some(
            PendingEdit::EditDescription { .. }
            | PendingEdit::EditDueDate { .. },
        ) if app.task_detail.is_some() => Mode::TaskDetail,
        _ => Mode::Normal,
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
    let raw = app.input.value().to_owned();

    // NewTask / RenameTask require a non-empty trimmed title; the
    // description / due-date edits accept empty input as "clear the field".
    match edit {
        PendingEdit::NewTask { column, anchor } => {
            let title = raw.trim().to_owned();
            if title.is_empty() {
                app.set_status("title must not be empty");
                return Ok(());
            }
            create_task(app, services, column, anchor, title).await
        }
        PendingEdit::RenameTask { column, task_id } => {
            let title = raw.trim().to_owned();
            if title.is_empty() {
                app.set_status("title must not be empty");
                return Ok(());
            }
            rename_task(app, services, column, task_id, title).await
        }
        PendingEdit::EditDescription { task_id } => {
            set_description(app, services, task_id, raw).await
        }
        PendingEdit::EditDueDate { task_id } => set_due_date(app, services, task_id, raw).await,
        PendingEdit::EditProjectName { project_id } => {
            let title = raw.trim().to_owned();
            if title.is_empty() {
                app.set_status("project name must not be empty");
                return Ok(());
            }
            project_rename(app, services, project_id, title).await
        }
        PendingEdit::EditProjectDescription { project_id } => {
            project_set_description(app, services, project_id, raw).await
        }
        PendingEdit::RenameState { state_id } => {
            let name = raw.trim().to_owned();
            if name.is_empty() {
                app.set_status("state name must not be empty");
                return Ok(());
            }
            state_rename(app, services, state_id, name).await
        }
        PendingEdit::SetStateWipLimit { state_id } => {
            state_set_wip(app, services, state_id, raw).await
        }
        PendingEdit::AddState { project_id } => {
            let name = raw.trim().to_owned();
            if name.is_empty() {
                app.set_status("state name must not be empty");
                return Ok(());
            }
            state_add(app, services, project_id, name).await
        }
        PendingEdit::NewProject => {
            let name = raw.trim().to_owned();
            if name.is_empty() {
                app.set_status("project name must not be empty");
                return Ok(());
            }
            project_create(app, services, name).await
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
        "projects" => open_project_picker(app, services).await,
        "edit-project" => {
            let project = app.board.project.clone();
            open_editor_for(app, services, project, false).await
        }
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
// Task detail
// -----------------------------------------------------------------------

async fn open_task_detail(app: &mut App, services: &AppServices) -> CoreResult<()> {
    let Some(task) = app.selected_task() else {
        app.set_status("no task selected");
        return Ok(());
    };
    let task_id = task.id;
    let sojourn = load_sojourn(app, services, task_id).await?;
    app.task_detail = Some(crate::app::TaskDetailSnapshot { task_id, sojourn });
    app.mode = Mode::TaskDetail;
    Ok(())
}

async fn load_sojourn(
    app: &App,
    services: &AppServices,
    task_id: TaskId,
) -> CoreResult<Vec<(StateId, std::time::Duration)>> {
    let raw = services.stats_service().task_history(task_id).await?;
    // Re-order so per-state rows match the project's column order.
    let mut by_id: std::collections::HashMap<[u8; 16], std::time::Duration> = raw
        .into_iter()
        .map(|(id, d)| (*id.inner().as_bytes(), d))
        .collect();
    let mut out = Vec::with_capacity(app.board.project.states.len());
    for st in &app.board.project.states {
        if let Some(d) = by_id.remove(st.id.inner().as_bytes()) {
            out.push((st.id, d));
        }
    }
    Ok(out)
}

async fn cycle_priority(app: &mut App, services: &AppServices) -> CoreResult<()> {
    let Some(snapshot) = app.task_detail.as_ref() else {
        return Ok(());
    };
    let task_id = snapshot.task_id;
    let task = task_by_id(app, task_id)
        .ok_or_else(|| CoreError::validation("task vanished from snapshot"))?;
    let next = match task.priority {
        Priority::Low => Priority::Normal,
        Priority::Normal => Priority::High,
        Priority::High => Priority::Critical,
        Priority::Critical => Priority::Low,
    };
    let update = TaskUpdate {
        priority: Some(next),
        ..Default::default()
    };
    services.task_service().update(task_id, update).await?;
    let column = column_of(app, task_id).unwrap_or(app.focused_column);
    reload_column(app, services, column).await?;
    Ok(())
}

async fn cycle_complexity(app: &mut App, services: &AppServices) -> CoreResult<()> {
    let Some(snapshot) = app.task_detail.as_ref() else {
        return Ok(());
    };
    let task_id = snapshot.task_id;
    let task = task_by_id(app, task_id)
        .ok_or_else(|| CoreError::validation("task vanished from snapshot"))?;
    let next = match task.complexity {
        kantui_core::Complexity::Light => kantui_core::Complexity::Deep,
        kantui_core::Complexity::Deep => kantui_core::Complexity::Light,
    };
    let update = TaskUpdate {
        complexity: Some(next),
        ..Default::default()
    };
    services.task_service().update(task_id, update).await?;
    let column = column_of(app, task_id).unwrap_or(app.focused_column);
    reload_column(app, services, column).await?;
    Ok(())
}

fn begin_edit_description(app: &mut App) -> CoreResult<()> {
    let Some(snapshot) = app.task_detail.as_ref() else {
        return Ok(());
    };
    let task_id = snapshot.task_id;
    let initial = task_by_id(app, task_id)
        .and_then(|t| t.description.clone())
        .unwrap_or_default();
    app.enter_insert(PendingEdit::EditDescription { task_id }, &initial);
    Ok(())
}

fn begin_edit_due_date(app: &mut App) -> CoreResult<()> {
    let Some(snapshot) = app.task_detail.as_ref() else {
        return Ok(());
    };
    let task_id = snapshot.task_id;
    let initial = task_by_id(app, task_id)
        .and_then(|t| t.due_date)
        .map(format_date_yyyy_mm_dd)
        .unwrap_or_default();
    app.enter_insert(PendingEdit::EditDueDate { task_id }, &initial);
    Ok(())
}

async fn set_description(
    app: &mut App,
    services: &AppServices,
    task_id: TaskId,
    raw: String,
) -> CoreResult<()> {
    let trimmed = raw.trim();
    let value = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    };
    let update = TaskUpdate {
        description: Some(value),
        ..Default::default()
    };
    services.task_service().update(task_id, update).await?;
    finish_detail_edit(app, services, task_id).await
}

async fn set_due_date(
    app: &mut App,
    services: &AppServices,
    task_id: TaskId,
    raw: String,
) -> CoreResult<()> {
    let trimmed = raw.trim();
    let parsed = if trimmed.is_empty() {
        None
    } else {
        match parse_yyyy_mm_dd(trimmed) {
            Some(ts) => Some(ts),
            None => {
                app.set_status(format!("bad date `{trimmed}` — use YYYY-MM-DD or empty"));
                app.leave_insert();
                return Ok(());
            }
        }
    };
    let update = TaskUpdate {
        due_date: Some(parsed),
        ..Default::default()
    };
    services.task_service().update(task_id, update).await?;
    finish_detail_edit(app, services, task_id).await
}

/// Common tail for description/due-date submits: refresh the owning column,
/// dismiss the prompt, and return to the detail overlay.
async fn finish_detail_edit(
    app: &mut App,
    services: &AppServices,
    task_id: TaskId,
) -> CoreResult<()> {
    let column = column_of(app, task_id).unwrap_or(app.focused_column);
    reload_column(app, services, column).await?;
    app.leave_insert();
    if app.task_detail.is_some() {
        app.mode = Mode::TaskDetail;
    }
    Ok(())
}

fn task_by_id(app: &App, id: TaskId) -> Option<&Task> {
    app.board
        .tasks_by_state
        .iter()
        .flatten()
        .find(|t| t.id == id)
}

fn column_of(app: &App, id: TaskId) -> Option<usize> {
    app.board
        .tasks_by_state
        .iter()
        .position(|tasks| tasks.iter().any(|t| t.id == id))
}

/// `YYYY-MM-DD` → midnight-UTC `Timestamp`. Reject anything else.
fn parse_yyyy_mm_dd(s: &str) -> Option<kantui_core::Timestamp> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let y: i32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    let d: u32 = parts[2].parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let secs = days_from_civil(y, m, d).checked_mul(86_400)?;
    let ts = std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(secs as u64))?;
    Some(kantui_core::Timestamp::from_system_time(ts))
}

/// Inverse of `civil_date` in widgets — Howard Hinnant's days-from-civil.
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era as i64 * 146_097 + doe as i64 - 719_468
}

fn format_date_yyyy_mm_dd(ts: kantui_core::Timestamp) -> String {
    let secs = ts
        .to_system_time()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
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

// -----------------------------------------------------------------------
// Project picker
// -----------------------------------------------------------------------

async fn open_project_picker(app: &mut App, services: &AppServices) -> CoreResult<()> {
    let snapshot = build_picker_snapshot(app, services, Some(app.board.project.id)).await?;
    app.project_picker = Some(snapshot);
    app.mode = Mode::ProjectPicker;
    Ok(())
}

async fn build_picker_snapshot(
    app: &App,
    services: &AppServices,
    prefer: Option<ProjectId>,
) -> CoreResult<ProjectPickerSnapshot> {
    let project_repo = services.project_repo();
    let task_repo = services.task_repo();
    let mut projects = project_repo.list().await?;
    projects.sort_by_key(|p| p.name.to_lowercase());

    let mut task_counts = Vec::with_capacity(projects.len());
    for project in &projects {
        let mut count = 0u32;
        for state in &project.states {
            count = count.saturating_add(task_repo.list_by_state(state.id).await?.len() as u32);
        }
        task_counts.push(count);
    }

    let preferred = prefer.unwrap_or(app.board.project.id);
    let selected = projects
        .iter()
        .position(|p| p.id == preferred)
        .unwrap_or(0);

    Ok(ProjectPickerSnapshot {
        projects,
        task_counts,
        selected,
    })
}

fn picker_move(app: &mut App, delta: i32) {
    let Some(picker) = app.project_picker.as_mut() else {
        return;
    };
    if picker.projects.is_empty() {
        picker.selected = 0;
        return;
    }
    let len = picker.projects.len() as i32;
    let next = (picker.selected as i32 + delta).rem_euclid(len);
    picker.selected = next as usize;
}

async fn picker_activate(app: &mut App, services: &AppServices) -> CoreResult<()> {
    let Some(picker) = app.project_picker.as_ref() else {
        return Ok(());
    };
    let Some(project) = picker.projects.get(picker.selected).cloned() else {
        app.set_status("no project selected");
        return Ok(());
    };
    if project.id == app.board.project.id {
        app.project_picker = None;
        app.mode = Mode::Normal;
        return Ok(());
    }
    switch_active_project(app, services, project).await?;
    app.project_picker = None;
    app.mode = Mode::Normal;
    Ok(())
}

async fn switch_active_project(
    app: &mut App,
    services: &AppServices,
    project: Project,
) -> CoreResult<()> {
    let board = crate::app::load_board(
        &services.project_repo(),
        &services.task_repo(),
        &services.tag_repo(),
        project,
    )
    .await?;
    app.board = board;
    app.focused_column = 0;
    app.selected_per_column = app
        .board
        .tasks_by_state
        .iter()
        .map(|tasks| if tasks.is_empty() { None } else { Some(0) })
        .collect();
    app.search_query = None;

    // Persist the active project so the next launch re-opens it.
    let mut state = crate::state::UiState::load(services.state_path());
    state.set_last_project(app.board.project.id);
    if let Err(err) = state.save(services.state_path()) {
        tracing::warn!(
            path = %services.state_path().display(),
            %err,
            "failed to persist UI state"
        );
    }
    Ok(())
}

async fn picker_edit_selected(app: &mut App, services: &AppServices) -> CoreResult<()> {
    let Some(picker) = app.project_picker.as_ref() else {
        return Ok(());
    };
    let Some(project) = picker.projects.get(picker.selected).cloned() else {
        app.set_status("no project selected");
        return Ok(());
    };
    open_editor_for(app, services, project, true).await
}

fn picker_new_project(app: &mut App) -> CoreResult<()> {
    app.mode = Mode::Insert;
    app.pending_edit = Some(PendingEdit::NewProject);
    app.input = kantui_widgets::InputState::new();
    Ok(())
}

async fn picker_delete_selected(app: &mut App, services: &AppServices) -> CoreResult<()> {
    let Some(picker) = app.project_picker.as_ref() else {
        return Ok(());
    };
    let Some(project) = picker.projects.get(picker.selected).cloned() else {
        app.set_status("no project selected");
        return Ok(());
    };
    if picker.projects.len() <= 1 {
        app.set_status("cannot delete the only project");
        return Ok(());
    }

    let active = project.id == app.board.project.id;
    project_service(services).delete(project.id).await?;

    // Refresh the picker. If we deleted the active project, switch to whatever
    // project sorts first now.
    let mut snapshot = build_picker_snapshot(app, services, None).await?;
    if active {
        if let Some(first) = snapshot.projects.first().cloned() {
            switch_active_project(app, services, first).await?;
            snapshot = build_picker_snapshot(app, services, Some(app.board.project.id)).await?;
        }
    }
    snapshot.selected = snapshot.selected.min(snapshot.projects.len().saturating_sub(1));
    app.project_picker = Some(snapshot);
    Ok(())
}

async fn project_create(
    app: &mut App,
    services: &AppServices,
    name: String,
) -> CoreResult<()> {
    let svc = project_service(services);
    let created = svc
        .create(NewProject {
            name,
            description: None,
            initial_states: vec!["Todo".into(), "Doing".into(), "Done".into()],
        })
        .await?;
    let created_id = created.id;

    // Always rebuild the picker to include the new project.
    let snapshot = build_picker_snapshot(app, services, Some(created_id)).await?;
    app.project_picker = Some(snapshot);
    app.leave_insert();
    app.mode = Mode::ProjectPicker;
    Ok(())
}

// -----------------------------------------------------------------------
// Project editor
// -----------------------------------------------------------------------

async fn open_editor_for(
    app: &mut App,
    services: &AppServices,
    project: Project,
    return_to_picker: bool,
) -> CoreResult<()> {
    let snapshot = build_editor_snapshot(services, project, return_to_picker).await?;
    app.project_editor = Some(snapshot);
    app.mode = Mode::ProjectEditor;
    Ok(())
}

async fn build_editor_snapshot(
    services: &AppServices,
    project: Project,
    return_to_picker: bool,
) -> CoreResult<ProjectEditorSnapshot> {
    let task_repo = services.task_repo();
    let mut state_task_counts = Vec::with_capacity(project.states.len());
    for state in &project.states {
        state_task_counts.push(task_repo.list_by_state(state.id).await?.len() as u32);
    }
    Ok(ProjectEditorSnapshot {
        project,
        state_task_counts,
        focus: ProjectEditorFocus::Name,
        return_to_picker,
    })
}

async fn refresh_editor_snapshot(
    app: &mut App,
    services: &AppServices,
    preserve_focus: ProjectEditorFocus,
) -> CoreResult<()> {
    let Some(current) = app.project_editor.as_ref() else {
        return Ok(());
    };
    let project_id = current.project.id;
    let return_to_picker = current.return_to_picker;
    let project = services
        .project_repo()
        .get(project_id)
        .await?
        .ok_or_else(|| CoreError::validation("project under edit vanished"))?;
    let mut snapshot = build_editor_snapshot(services, project, return_to_picker).await?;
    snapshot.focus = clamp_focus(preserve_focus, snapshot.project.states.len());
    app.project_editor = Some(snapshot);
    if app.mode == Mode::Insert {
        // Caller will pop Insert; we leave the mode flip to them.
    } else {
        app.mode = Mode::ProjectEditor;
    }
    Ok(())
}

fn clamp_focus(focus: ProjectEditorFocus, state_count: usize) -> ProjectEditorFocus {
    match focus {
        ProjectEditorFocus::State(i) if i >= state_count => {
            if state_count == 0 {
                ProjectEditorFocus::AddState
            } else {
                ProjectEditorFocus::State(state_count - 1)
            }
        }
        other => other,
    }
}

async fn close_project_editor(app: &mut App, services: &AppServices) -> CoreResult<()> {
    let Some(editor) = app.project_editor.take() else {
        app.mode = Mode::Normal;
        return Ok(());
    };

    // If the editor was open on the active project, refresh the board so any
    // state edits/reorders show up immediately.
    if editor.project.id == app.board.project.id {
        reload_board(app, services).await?;
    }

    if editor.return_to_picker {
        // Rebuild the picker so its rows reflect any rename/state changes.
        let snapshot = build_picker_snapshot(app, services, Some(editor.project.id)).await?;
        app.project_picker = Some(snapshot);
        app.mode = Mode::ProjectPicker;
    } else {
        app.mode = Mode::Normal;
    }
    Ok(())
}

fn editor_focus_move(app: &mut App, delta: i32) {
    let Some(editor) = app.project_editor.as_mut() else {
        return;
    };
    let total = editor_focus_count(editor.project.states.len());
    let current = focus_to_index(editor.focus, editor.project.states.len());
    let next = ((current as i32 + delta).rem_euclid(total as i32)) as usize;
    editor.focus = index_to_focus(next, editor.project.states.len());
}

fn editor_focus_count(state_count: usize) -> usize {
    // Name + Description + N state rows + AddState
    3 + state_count
}

fn focus_to_index(focus: ProjectEditorFocus, state_count: usize) -> usize {
    match focus {
        ProjectEditorFocus::Name => 0,
        ProjectEditorFocus::Description => 1,
        ProjectEditorFocus::State(i) => 2 + i.min(state_count.saturating_sub(1)),
        ProjectEditorFocus::AddState => 2 + state_count,
    }
}

fn index_to_focus(idx: usize, state_count: usize) -> ProjectEditorFocus {
    match idx {
        0 => ProjectEditorFocus::Name,
        1 => ProjectEditorFocus::Description,
        i if i == 2 + state_count => ProjectEditorFocus::AddState,
        i => ProjectEditorFocus::State(i - 2),
    }
}

fn editor_begin_edit(app: &mut App) -> CoreResult<()> {
    let Some(editor) = app.project_editor.as_ref() else {
        return Ok(());
    };
    let project_id = editor.project.id;
    match editor.focus {
        ProjectEditorFocus::Name => {
            let initial = editor.project.name.clone();
            app.enter_insert(PendingEdit::EditProjectName { project_id }, &initial);
            Ok(())
        }
        ProjectEditorFocus::Description => {
            let initial = editor.project.description.clone().unwrap_or_default();
            app.enter_insert(
                PendingEdit::EditProjectDescription { project_id },
                &initial,
            );
            Ok(())
        }
        ProjectEditorFocus::State(i) => {
            let Some(state) = editor.project.states.get(i) else {
                return Ok(());
            };
            let state_id = state.id;
            let initial = state.name.clone();
            app.enter_insert(PendingEdit::RenameState { state_id }, &initial);
            Ok(())
        }
        ProjectEditorFocus::AddState => editor_begin_add_state(app),
    }
}

fn editor_begin_edit_wip(app: &mut App) -> CoreResult<()> {
    let Some(editor) = app.project_editor.as_ref() else {
        return Ok(());
    };
    let ProjectEditorFocus::State(i) = editor.focus else {
        app.set_status("focus a state to edit its WIP limit");
        return Ok(());
    };
    let Some(state) = editor.project.states.get(i) else {
        return Ok(());
    };
    let state_id = state.id;
    let initial = state
        .wip_limit
        .map(|n| n.to_string())
        .unwrap_or_default();
    app.enter_insert(PendingEdit::SetStateWipLimit { state_id }, &initial);
    Ok(())
}

fn editor_begin_add_state(app: &mut App) -> CoreResult<()> {
    let Some(editor) = app.project_editor.as_ref() else {
        return Ok(());
    };
    let project_id = editor.project.id;
    app.enter_insert(PendingEdit::AddState { project_id }, "");
    Ok(())
}

async fn editor_delete_state(app: &mut App, services: &AppServices) -> CoreResult<()> {
    let Some(editor) = app.project_editor.as_ref() else {
        return Ok(());
    };
    let ProjectEditorFocus::State(i) = editor.focus else {
        app.set_status("focus a state to delete it");
        return Ok(());
    };
    let Some(state) = editor.project.states.get(i).cloned() else {
        return Ok(());
    };
    if editor.project.states.len() <= 1 {
        app.set_status("cannot delete the only state");
        return Ok(());
    }
    if editor.state_task_counts.get(i).copied().unwrap_or(0) > 0 {
        app.set_status("state is not empty; delete its tasks first");
        return Ok(());
    }
    project_service(services).remove_state(state.id).await?;
    refresh_editor_snapshot(app, services, ProjectEditorFocus::State(i)).await
}

async fn editor_shift_state(
    app: &mut App,
    services: &AppServices,
    delta: i32,
) -> CoreResult<()> {
    let Some(editor) = app.project_editor.as_ref() else {
        return Ok(());
    };
    let ProjectEditorFocus::State(i) = editor.focus else {
        app.set_status("focus a state to reorder it");
        return Ok(());
    };
    let len = editor.project.states.len();
    if len < 2 {
        return Ok(());
    }
    let target = (i as i32 + delta).clamp(0, len as i32 - 1) as usize;
    if target == i {
        return Ok(());
    }
    let project_id = editor.project.id;
    let mut ordered: Vec<StateId> = editor.project.states.iter().map(|s| s.id).collect();
    let moved = ordered.remove(i);
    ordered.insert(target, moved);
    project_service(services)
        .reorder_states(project_id, &ordered)
        .await?;
    refresh_editor_snapshot(app, services, ProjectEditorFocus::State(target)).await
}

async fn project_rename(
    app: &mut App,
    services: &AppServices,
    project_id: ProjectId,
    name: String,
) -> CoreResult<()> {
    project_service(services).rename(project_id, &name).await?;
    finish_editor_edit(app, services, ProjectEditorFocus::Name).await
}

async fn project_set_description(
    app: &mut App,
    services: &AppServices,
    project_id: ProjectId,
    raw: String,
) -> CoreResult<()> {
    let trimmed = raw.trim();
    let new_desc = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    };
    let mut project = services
        .project_repo()
        .get(project_id)
        .await?
        .ok_or_else(|| CoreError::validation("project vanished"))?;
    project.description = new_desc;
    services.project_repo().update(&project).await?;
    finish_editor_edit(app, services, ProjectEditorFocus::Description).await
}

async fn state_rename(
    app: &mut App,
    services: &AppServices,
    state_id: StateId,
    name: String,
) -> CoreResult<()> {
    project_service(services).rename_state(state_id, &name).await?;
    let focus = focus_for_state(app, state_id);
    finish_editor_edit(app, services, focus).await
}

async fn state_set_wip(
    app: &mut App,
    services: &AppServices,
    state_id: StateId,
    raw: String,
) -> CoreResult<()> {
    let trimmed = raw.trim();
    let parsed = if trimmed.is_empty() {
        None
    } else {
        match trimmed.parse::<u32>() {
            Ok(n) if n > 0 => Some(n),
            _ => {
                app.set_status(format!("bad WIP `{trimmed}` — use a positive integer or empty"));
                app.leave_insert();
                app.mode = Mode::ProjectEditor;
                return Ok(());
            }
        }
    };
    project_service(services).set_wip_limit(state_id, parsed).await?;
    let focus = focus_for_state(app, state_id);
    finish_editor_edit(app, services, focus).await
}

async fn state_add(
    app: &mut App,
    services: &AppServices,
    project_id: ProjectId,
    name: String,
) -> CoreResult<()> {
    project_service(services)
        .add_state(
            project_id,
            NewState {
                name,
                wip_limit: None,
            },
        )
        .await?;
    // Focus the freshly-appended row.
    let new_index = app
        .project_editor
        .as_ref()
        .map(|e| e.project.states.len())
        .unwrap_or(0);
    finish_editor_edit(app, services, ProjectEditorFocus::State(new_index)).await
}

fn focus_for_state(app: &App, state_id: StateId) -> ProjectEditorFocus {
    app.project_editor
        .as_ref()
        .and_then(|e| {
            e.project
                .states
                .iter()
                .position(|s| s.id == state_id)
                .map(ProjectEditorFocus::State)
        })
        .unwrap_or(ProjectEditorFocus::Name)
}

/// Common tail for editor field submits: reload the editor snapshot, dismiss
/// the prompt, and return to the editor overlay.
async fn finish_editor_edit(
    app: &mut App,
    services: &AppServices,
    focus: ProjectEditorFocus,
) -> CoreResult<()> {
    refresh_editor_snapshot(app, services, focus).await?;
    app.leave_insert();
    app.mode = Mode::ProjectEditor;
    Ok(())
}
