//! Rendering — pure: read from [`App`], write to the frame. No mutation.

use crossterm::event::{KeyCode, KeyEvent};
use kantui_core::Task;
use kantui_widgets::{
    BoardView, BoardViewModel, ChordHelp, Dashboard, DashboardStateRow, DashboardThroughput,
    DashboardView, HelpOverlay, HelpRow, Input, JumpLabelView, JumpLabels, Mode as WidgetMode,
    ProjectEditor, ProjectEditorStateRow, ProjectEditorView, ProjectPicker, ProjectPickerRow,
    ProjectPickerView, StateColumnView, StatusBar, StatusBarView, StatusCounts, TagChip,
    TaskCardView, TaskDetail, TaskDetailView, Theme,
};

use crate::keybinds::Key;

use crate::app::BoardSnapshot;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::app::{App, Mode, PendingEdit};

/// Height of a task card in rows (must match widgets::state_column).
const TASK_CARD_ROWS: u16 = 6;
/// Minimum width (in cells) for a single state column (must match widgets::board).
const MIN_COLUMN_WIDTH: u16 = 20;

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let theme = app.theme;
    let area = frame.area();

    let bg = Paragraph::new(Line::from("")).style(Style::default().bg(theme.background));
    frame.render_widget(bg, area);

    let show_prompt = matches!(app.mode, Mode::Insert | Mode::Command | Mode::Search);
    let constraints: Vec<Constraint> = if show_prompt {
        vec![
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ]
    } else {
        vec![Constraint::Min(1), Constraint::Length(1)]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let board_area = chunks[0];
    render_board(frame, board_area, app, &theme);

    if show_prompt {
        render_prompt(frame, chunks[1], app, &theme);
        render_status(frame, chunks[2], app, &theme);
    } else {
        render_status(frame, chunks[1], app, &theme);
    }

    if app.mode == Mode::Jump
        && let Some(jump_state) = &app.jump
    {
        let labels = build_jump_label_views(app, board_area, &jump_state.labels);
        frame.render_widget(
            JumpLabels::new(&labels, &theme).pending_prefix(jump_state.pending_prefix),
            board_area,
        );
    }

    if app.mode == Mode::TagPicker
        && let Some(picker) = &app.tag_picker
    {
        render_tag_picker(frame, area, picker, &theme);
    }

    if app.mode == Mode::Dashboard
        && let Some(snapshot) = &app.dashboard
    {
        render_dashboard(frame, area, app, snapshot, &theme);
    }

    if app.mode == Mode::TaskDetail
        && let Some(snapshot) = &app.task_detail
    {
        render_task_detail(frame, area, app, snapshot, &theme);
    }

    if app.mode == Mode::ProjectPicker
        && let Some(snapshot) = &app.project_picker
    {
        render_project_picker(frame, area, app, snapshot, &theme);
    }

    if app.mode == Mode::ProjectEditor
        && let Some(snapshot) = &app.project_editor
    {
        render_project_editor(frame, area, snapshot, &theme);
    }

    if app.show_help {
        frame.render_widget(HelpOverlay::new("Keybindings", HELP_ROWS, &theme), area);
    }

    if app.mode == Mode::Normal
        && let Some(pending) = app.keymap.pending()
    {
        render_chord_help(frame, board_area, app, pending, &theme);
    }
}

fn render_chord_help(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    pending: KeyEvent,
    theme: &Theme,
) {
    let completions = app.keymap.chord_completions(&pending);
    if completions.is_empty() {
        return;
    }
    // Own the strings referenced by HelpRow for the duration of this call.
    let owned: Vec<(String, &'static str)> = completions
        .into_iter()
        .map(|(k, action)| (key_label(&k), action.description()))
        .collect();
    let rows: Vec<HelpRow<'_>> = owned
        .iter()
        .map(|(k, d)| HelpRow {
            keys: k.as_str(),
            description: d,
        })
        .collect();

    if area.width == 0 || area.height == 0 {
        return;
    }
    // Snap the popup area to the rightmost column so its left/right borders
    // line up with the column borders (mirrors `widgets::board` layout math).
    let n = app.board.project.states.len().max(1) as u16;
    let column_width = (area.width / n).max(MIN_COLUMN_WIDTH.min(area.width));
    let height = (rows.len() as u16).saturating_add(2).min(area.height);
    let x = area.x + (n - 1) * column_width;
    let max_width = (area.x + area.width).saturating_sub(x);
    let width = column_width.min(max_width);
    if width < 3 || height < 3 {
        return;
    }
    let y = area.y + area.height - height;
    let popup_area = Rect {
        x,
        y,
        width,
        height,
    };
    frame.render_widget(ChordHelp::new(&rows, theme), popup_area);
}

/// Format a [`Key`] as a short label suitable for the chord-help overlay.
fn key_label(k: &Key) -> String {
    match k.code {
        KeyCode::Char(' ') => "space".to_owned(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Esc => "esc".to_owned(),
        KeyCode::Enter => "enter".to_owned(),
        KeyCode::Tab => "tab".to_owned(),
        KeyCode::Backspace => "bs".to_owned(),
        KeyCode::Delete => "del".to_owned(),
        KeyCode::Left => "←".to_owned(),
        KeyCode::Right => "→".to_owned(),
        KeyCode::Up => "↑".to_owned(),
        KeyCode::Down => "↓".to_owned(),
        other => format!("{other:?}"),
    }
}

fn render_dashboard(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    snapshot: &crate::app::DashboardSnapshot,
    theme: &Theme,
) {
    // Own strings referenced by the DashboardStateRow slice for the duration
    // of this call.
    let mut rows: Vec<DashboardStateRow<'_>> = Vec::with_capacity(app.board.project.states.len());
    let sojourns_by_state: std::collections::HashMap<[u8; 16], (u64, u32)> = snapshot
        .sojourns
        .iter()
        .map(|s| (*s.state_id.inner().as_bytes(), (s.total.as_secs(), s.count)))
        .collect();
    for (i, state) in app.board.project.states.iter().enumerate() {
        let tasks = app.board.tasks_by_state.get(i).map(Vec::len).unwrap_or(0) as u32;
        let (total_seconds, visits) = sojourns_by_state
            .get(state.id.inner().as_bytes())
            .copied()
            .unwrap_or((0, 0));
        rows.push(DashboardStateRow {
            name: state.name.as_str(),
            tasks,
            wip_limit: state.wip_limit,
            total_seconds,
            visits,
        });
    }

    let view = DashboardView {
        project_name: app.board.project.name.as_str(),
        states: &rows,
        throughput: DashboardThroughput {
            total: snapshot.throughput.total,
            per_day: &snapshot.throughput.per_day,
        },
    };
    frame.render_widget(Dashboard::new(view, theme), area);
}

fn render_task_detail(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    snapshot: &crate::app::TaskDetailSnapshot,
    theme: &Theme,
) {
    // Look up the live task from the board (the snapshot only carries the
    // id + sojourn).
    let Some(task) = app
        .board
        .tasks_by_state
        .iter()
        .flatten()
        .find(|t| t.id == snapshot.task_id)
    else {
        return;
    };

    // Build owned buffers for borrowed slices in TaskDetailView.
    let chips: Vec<TagChip<'_>> = task
        .tags
        .iter()
        .filter_map(|id| app.board.tag_by_id(*id))
        .map(|tag| TagChip {
            name: tag.name.as_str(),
            color: tag.color,
        })
        .collect();

    let sojourn_owned: Vec<(&str, u64)> = snapshot
        .sojourn
        .iter()
        .filter_map(|(state_id, dur)| {
            app.board
                .project
                .states
                .iter()
                .find(|s| s.id == *state_id)
                .map(|s| (s.name.as_str(), dur.as_secs()))
        })
        .collect();

    let view = TaskDetailView {
        title: task.title.as_str(),
        description: task.description.as_deref(),
        priority: task.priority,
        complexity: task.complexity,
        due_date: task.due_date,
        tags: &chips,
        sojourn: &sojourn_owned,
        created_at: task.created_at,
        updated_at: task.updated_at,
    };

    frame.render_widget(TaskDetail::new(view, theme), area);
}

fn render_project_picker(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    snapshot: &crate::app::ProjectPickerSnapshot,
    theme: &Theme,
) {
    let active_id = app.board.project.id;
    let rows: Vec<ProjectPickerRow<'_>> = snapshot
        .projects
        .iter()
        .zip(snapshot.task_counts.iter())
        .map(|(project, count)| ProjectPickerRow {
            name: project.name.as_str(),
            description: project.description.as_deref(),
            state_count: project.states.len() as u32,
            task_count: *count,
            is_active: project.id == active_id,
        })
        .collect();

    let view = ProjectPickerView {
        projects: &rows,
        selected: if snapshot.projects.is_empty() {
            None
        } else {
            Some(snapshot.selected)
        },
    };
    frame.render_widget(ProjectPicker::new(view, theme), area);
}

fn render_project_editor(
    frame: &mut Frame<'_>,
    area: Rect,
    snapshot: &crate::app::ProjectEditorSnapshot,
    theme: &Theme,
) {
    let states: Vec<ProjectEditorStateRow<'_>> = snapshot
        .project
        .states
        .iter()
        .zip(snapshot.state_task_counts.iter())
        .map(|(state, count)| ProjectEditorStateRow {
            name: state.name.as_str(),
            wip_limit: state.wip_limit,
            task_count: *count,
        })
        .collect();
    let view = ProjectEditorView {
        name: snapshot.project.name.as_str(),
        description: snapshot.project.description.as_deref(),
        states: &states,
        focus: snapshot.focus,
    };
    frame.render_widget(ProjectEditor::new(view, theme), area);
}

fn render_tag_picker(
    frame: &mut Frame<'_>,
    area: Rect,
    picker: &crate::app::TagPickerState,
    theme: &Theme,
) {
    // Own the row strings so the HelpOverlay borrows outlive this call.
    let owned: Vec<(String, String)> = picker
        .rows
        .iter()
        .map(|r| {
            let keys = format!("[{}]", r.label);
            let mark = if r.attached { "●" } else { "○" };
            let desc = format!("{mark} #{}", r.tag.name);
            (keys, desc)
        })
        .collect();
    let rows: Vec<HelpRow<'_>> = owned
        .iter()
        .map(|(k, d)| HelpRow {
            keys: k.as_str(),
            description: d.as_str(),
        })
        .collect();
    frame.render_widget(
        HelpOverlay::new("Toggle tag (Esc to close)", &rows, theme),
        area,
    );
}

fn render_board(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    // Own the per-column TaskCardView vectors so the slices we hand to
    // StateColumnView outlive the render call. The visible lists already
    // reflect the active search filter.
    let visible_by_column: Vec<Vec<&Task>> = (0..app.board.project.states.len())
        .map(|i| app.visible_tasks(i))
        .collect();

    // Resolve each task's `Vec<TagId>` into owned `TagChip` vectors. These
    // live for the whole render call so the slices handed to TaskCardView
    // remain valid.
    let chips_by_task: Vec<Vec<Vec<TagChip<'_>>>> = visible_by_column
        .iter()
        .map(|tasks| {
            tasks
                .iter()
                .map(|t| task_chips(t, &app.board))
                .collect::<Vec<_>>()
        })
        .collect();

    let cards_by_column: Vec<Vec<TaskCardView<'_>>> = visible_by_column
        .iter()
        .zip(chips_by_task.iter())
        .map(|(tasks, chips)| {
            tasks
                .iter()
                .zip(chips.iter())
                .map(|(t, c)| task_to_view(t, c))
                .collect()
        })
        .collect();

    let columns: Vec<StateColumnView<'_>> = app
        .board
        .project
        .states
        .iter()
        .enumerate()
        .map(|(i, state)| StateColumnView {
            name: state.name.as_str(),
            wip_limit: state.wip_limit,
            tasks: cards_by_column.get(i).map(Vec::as_slice).unwrap_or(&[]),
            // Only the focused column shows a highlighted task — selection is
            // a single cursor, not one-per-column state.
            selected: if i == app.focused_column {
                app.selected_per_column.get(i).copied().flatten()
            } else {
                None
            },
        })
        .collect();

    let board = BoardViewModel {
        project_name: app.board.project.name.as_str(),
        states: &columns,
        focused_column: app.focused_column,
    };
    frame.render_widget(BoardView::new(board, theme), area);
}

fn task_to_view<'a>(task: &'a Task, chips: &'a [TagChip<'a>]) -> TaskCardView<'a> {
    TaskCardView {
        title: task.title.as_str(),
        priority: task.priority,
        complexity: task.complexity,
        due_date: task.due_date,
        tags: chips,
    }
}

fn task_chips<'a>(task: &Task, board: &'a BoardSnapshot) -> Vec<TagChip<'a>> {
    task.tags
        .iter()
        .filter_map(|id| board.tag_by_id(*id))
        .map(|tag| TagChip {
            name: tag.name.as_str(),
            color: tag.color,
        })
        .collect()
}

fn render_prompt(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let prefix = match (app.mode, app.pending_edit.as_ref()) {
        (Mode::Insert, Some(PendingEdit::NewTask { .. })) => "new › ",
        (Mode::Insert, Some(PendingEdit::RenameTask { .. })) => "rename › ",
        (Mode::Insert, Some(PendingEdit::EditDescription { .. })) => "desc › ",
        (Mode::Insert, Some(PendingEdit::EditDueDate { .. })) => "due (YYYY-MM-DD) › ",
        (Mode::Insert, Some(PendingEdit::EditProjectName { .. })) => "project name › ",
        (Mode::Insert, Some(PendingEdit::EditProjectDescription { .. })) => "project desc › ",
        (Mode::Insert, Some(PendingEdit::RenameState { .. })) => "state name › ",
        (Mode::Insert, Some(PendingEdit::SetStateWipLimit { .. })) => "wip limit (empty=none) › ",
        (Mode::Insert, Some(PendingEdit::AddState { .. })) => "new state › ",
        (Mode::Insert, Some(PendingEdit::NewProject)) => "new project › ",
        (Mode::Command, _) => ":",
        (Mode::Search, _) => "/",
        _ => "› ",
    };
    frame.render_widget(Input::new(&app.input, theme).prefix(prefix), area);
}

fn render_status(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let mode = match app.mode {
        Mode::Normal
        | Mode::Jump
        | Mode::TagPicker
        | Mode::Dashboard
        | Mode::TaskDetail
        | Mode::ProjectPicker
        | Mode::ProjectEditor => WidgetMode::Normal,
        Mode::Insert => WidgetMode::Insert,
        Mode::Command => WidgetMode::Command,
        Mode::Search => WidgetMode::Search,
    };

    let state_name = app
        .board
        .project
        .states
        .get(app.focused_column)
        .map(|s| s.name.as_str())
        .unwrap_or("—");

    let selected_title = app.selected_task().map(|t| t.title.as_str());
    let message = app.status_message.as_deref();
    let filter_note = app
        .active_filter()
        .map(|f| format!("/{f}"))
        .or_else(|| (app.mode == Mode::Jump).then(|| "jump".to_owned()))
        .or_else(|| (app.mode == Mode::TagPicker).then(|| "tag".to_owned()));

    let right_hint: Option<String> = filter_note;

    let clock = crate::app::format_clock_utc();

    let total: u32 = app
        .board
        .tasks_by_state
        .iter()
        .map(|ts| ts.len() as u32)
        .sum();

    let done = app
        .board
        .tasks_by_state
        .last()
        .map(|ts| ts.len() as u32)
        .unwrap_or(0);

    let task_title = message.or(selected_title).or(right_hint.as_deref());

    let view = StatusBarView {
        mode,
        project: app.board.project.name.as_str(),
        state: state_name,
        task_title,
        counts: StatusCounts {
            tasks_total: total,
            tasks_done: done,
        },
        clock: clock.as_str(),
    };
    frame.render_widget(StatusBar::new(view, theme), area);
}

/// Place each jump label at the top-left of the corresponding task card.
/// Mirrors the layout math in `widgets::board` / `widgets::state_column`.
fn build_jump_label_views<'a>(
    app: &App,
    board_area: Rect,
    labels: &'a [crate::app::JumpLabel],
) -> Vec<JumpLabelView<'a>> {
    let n = app.board.project.states.len().max(1) as u16;
    if board_area.width == 0 || board_area.height == 0 {
        return Vec::new();
    }
    let column_width = (board_area.width / n).max(MIN_COLUMN_WIDTH.min(board_area.width));

    labels
        .iter()
        .filter_map(|l| {
            let column_x = board_area.x + (l.column as u16) * column_width;
            // Inside the column: one row for the top border, then each card
            // occupies TASK_CARD_ROWS rows.
            let inner_top = board_area.y.saturating_add(1);
            let y = inner_top.saturating_add((l.visible_index as u16) * TASK_CARD_ROWS);
            // Inside the column block: offset one cell to the right of the
            // left border so the label sits on the first content column.
            let x = column_x.saturating_add(1);
            if y >= board_area.y + board_area.height {
                return None;
            }
            Some(JumpLabelView {
                label: label_str(&l.label),
                x,
                y,
            })
        })
        .collect()
}

/// Cache a static string slice per label pair. A tiny lookup table beats
/// allocating a new `String` every render.
fn label_str(label: &[char; 2]) -> &'static str {
    static TABLE: std::sync::OnceLock<[String; 26 * 26]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut arr: [String; 26 * 26] = std::array::from_fn(|_| String::new());
        for i in 0..26 {
            for j in 0..26 {
                let first = (b'a' + i as u8) as char;
                let second = (b'a' + j as u8) as char;
                arr[i * 26 + j] = format!("{first}{second}");
            }
        }
        arr
    });
    let idx = (label[0] as usize - 'a' as usize) * 26 + (label[1] as usize - 'a' as usize);
    table[idx].as_str()
}

/// Keybind cheatsheet shown by [`HelpOverlay`]. Kept in the binary because
/// the exact set of keys lives here (not in the widgets crate).
const HELP_ROWS: &[HelpRow<'static>] = &[
    HelpRow {
        keys: "h / l",
        description: "Focus previous / next column",
    },
    HelpRow {
        keys: "j / k",
        description: "Select next / previous task",
    },
    HelpRow {
        keys: "gg / G",
        description: "Top / bottom of column",
    },
    HelpRow {
        keys: "gw",
        description: "Enter two-char jump mode",
    },
    HelpRow {
        keys: "n / N",
        description: "New task below / above selection",
    },
    HelpRow {
        keys: "i",
        description: "Rename selected task",
    },
    HelpRow {
        keys: "e",
        description: "Open task detail (priority, description, due date, sojourn)",
    },
    HelpRow {
        keys: "d",
        description: "Delete selected task",
    },
    HelpRow {
        keys: "H / L",
        description: "Move task to previous / next column",
    },
    HelpRow {
        keys: "K / J",
        description: "Shift task up / down within column",
    },
    HelpRow {
        keys: "t",
        description: "Tag picker (toggle tags on selected task)",
    },
    HelpRow {
        keys: "gs",
        description: "Open statistics dashboard",
    },
    HelpRow {
        keys: "gp",
        description: "Open project picker (Enter open · e edit · n new · d delete)",
    },
    HelpRow {
        keys: ":",
        description: "Enter command mode",
    },
    HelpRow {
        keys: "/",
        description: "Search (live filter; `#tag` filters by tag)",
    },
    HelpRow {
        keys: "q",
        description: "Quit",
    },
    HelpRow {
        keys: "Esc",
        description: "Cancel prompt / exit jump mode",
    },
];
