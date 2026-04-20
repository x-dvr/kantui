//! Rendering — pure: read from [`App`], write to the frame. No mutation.

use kantui_core::Task;
use kantui_widgets::{
    BoardView, BoardViewModel, Mode as WidgetMode, StateColumnView, StatusBar, StatusBarView,
    StatusCounts, TagChip, TaskCardView, Theme,
};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::app::{App, Mode};

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let theme = Theme::default();
    let area = frame.area();

    let bg = Paragraph::new(Line::from("")).style(Style::default().bg(theme.background));
    frame.render_widget(bg, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    render_board(frame, chunks[0], app, &theme);
    render_status(frame, chunks[1], app, &theme);
}

fn render_board(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    // Own the TaskCardView vectors (one per column) so the slices we hand to
    // StateColumnView outlive the render call.
    let cards_by_column: Vec<Vec<TaskCardView<'_>>> = app
        .board
        .tasks_by_state
        .iter()
        .map(|tasks| tasks.iter().map(task_to_view).collect())
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

fn task_to_view(task: &Task) -> TaskCardView<'_> {
    TaskCardView {
        title: task.title.as_str(),
        priority: task.priority,
        complexity: task.complexity,
        due_date: task.due_date,
        // Tags resolve once a TagRepository wire-up lands (M7). For now,
        // render without chips.
        tags: &[] as &[TagChip<'_>],
    }
}

fn render_status(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let mode = match app.mode {
        Mode::Normal => WidgetMode::Normal,
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

    let view = StatusBarView {
        mode,
        project: app.board.project.name.as_str(),
        state: state_name,
        task_title: message.or(selected_title),
        counts: StatusCounts {
            tasks_total: total,
            tasks_done: done,
        },
        clock: clock.as_str(),
    };
    frame.render_widget(StatusBar::new(view, theme), area);
}
