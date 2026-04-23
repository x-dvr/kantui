//! Rendering — pure: read from [`App`], write to the frame. No mutation.

use kantui_core::Task;
use kantui_widgets::{
    BoardView, BoardViewModel, Dashboard, DashboardStateRow, DashboardThroughput, DashboardView,
    HelpOverlay, HelpRow, Input, JumpLabelView, JumpLabels, Mode as WidgetMode, StateColumnView,
    StatusBar, StatusBarView, StatusCounts, TagChip, TaskCardView, Theme,
};

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
    let theme = Theme::default();
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

    if app.show_help {
        frame.render_widget(HelpOverlay::new("Keybindings", HELP_ROWS, &theme), area);
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
            selected: app.selected_per_column.get(i).copied().flatten(),
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
        (Mode::Command, _) => ":",
        (Mode::Search, _) => "/",
        _ => "› ",
    };
    frame.render_widget(Input::new(&app.input, theme).prefix(prefix), area);
}

fn render_status(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let mode = match app.mode {
        Mode::Normal | Mode::Jump | Mode::TagPicker | Mode::Dashboard => WidgetMode::Normal,
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
