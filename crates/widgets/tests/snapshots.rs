//! Snapshot tests for the minimal widget set defined in M3.
//!
//! Buffers are rendered with `TestBackend` and serialised to plain text (one
//! row per line, `symbol()` per cell) so diffs stay readable in the snapshot
//! files.

use std::time::{Duration, UNIX_EPOCH};

use kantui_core::{Color, Complexity, Priority, Timestamp};
use kantui_widgets::{
    BoardView, BoardViewModel, Input, InputState, Mode, StateColumn, StateColumnView, StatusBar,
    StatusBarView, StatusCounts, TagChip, TaskCard, TaskCardView, Theme,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::widgets::Widget;

fn buffer_to_string(buf: &Buffer) -> String {
    let mut out = String::new();
    let area = buf.area;
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn render<F: FnOnce(&mut Buffer)>(width: u16, height: u16, f: F) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal
        .draw(|frame| {
            let area = frame.area();
            f(frame.buffer_mut());
            let _ = area; // unused when closure needs Rect via buffer
        })
        .expect("draw");
    buffer_to_string(terminal.backend().buffer())
}

fn ts_from_unix_secs(secs: i64) -> Timestamp {
    let t = if secs >= 0 {
        UNIX_EPOCH + Duration::from_secs(secs as u64)
    } else {
        UNIX_EPOCH - Duration::from_secs((-secs) as u64)
    };
    Timestamp::from_system_time(t)
}

fn sample_tags() -> [TagChip<'static>; 2] {
    [
        TagChip {
            name: "bug",
            color: Color::Red,
        },
        TagChip {
            name: "ui",
            color: Color::Cyan,
        },
    ]
}

#[test]
fn task_card_selected_with_tags() {
    let theme = Theme::default();
    let tags = sample_tags();
    let view = TaskCardView {
        title: "Wire up status bar",
        priority: Priority::High,
        complexity: Complexity::Deep,
        due_date: Some(ts_from_unix_secs(1_776_643_200)), // 2026-04-20
        tags: &tags,
    };

    let out = render(40, 8, |buf| {
        let area = buf.area;
        TaskCard::new(view, &theme).selected(true).render(area, buf);
    });
    insta::assert_snapshot!("task_card_selected_with_tags", out);
}

#[test]
fn task_card_unselected_no_due_date() {
    let theme = Theme::default();
    let view = TaskCardView {
        title: "Draft plan",
        priority: Priority::Normal,
        complexity: Complexity::Light,
        due_date: None,
        tags: &[],
    };

    let out = render(40, 5, |buf| {
        let area = buf.area;
        TaskCard::new(view, &theme).render(area, buf);
    });
    insta::assert_snapshot!("task_card_unselected_no_due_date", out);
}

fn sample_state_column<'a>(
    tasks: &'a [TaskCardView<'a>],
    selected: Option<usize>,
) -> StateColumnView<'a> {
    StateColumnView {
        name: "In progress",
        wip_limit: Some(3),
        tasks,
        selected,
    }
}

#[test]
fn state_column_with_selection() {
    let theme = Theme::default();
    let t1 = TaskCardView {
        title: "First task",
        priority: Priority::Critical,
        complexity: Complexity::Deep,
        due_date: None,
        tags: &[],
    };
    let t2 = TaskCardView {
        title: "Second task",
        priority: Priority::Low,
        complexity: Complexity::Light,
        due_date: None,
        tags: &[],
    };
    let tasks = [t1, t2];
    let view = sample_state_column(&tasks, Some(1));
    let out = render(30, 20, |buf| {
        let area = buf.area;
        StateColumn::new(view, &theme)
            .focused(true)
            .render(area, buf);
    });
    insta::assert_snapshot!("state_column_with_selection", out);
}

#[test]
fn board_view_two_columns() {
    let theme = Theme::default();
    let t1 = TaskCardView {
        title: "Write widgets",
        priority: Priority::High,
        complexity: Complexity::Deep,
        due_date: None,
        tags: &[],
    };
    let t2 = TaskCardView {
        title: "Hook up event loop",
        priority: Priority::Normal,
        complexity: Complexity::Light,
        due_date: None,
        tags: &[],
    };
    let todo_tasks = [t1];
    let doing_tasks = [t2];
    let todo = StateColumnView {
        name: "Todo",
        wip_limit: None,
        tasks: &todo_tasks,
        selected: Some(0),
    };
    let doing = StateColumnView {
        name: "Doing",
        wip_limit: Some(2),
        tasks: &doing_tasks,
        selected: None,
    };
    let columns = [todo, doing];
    let board = BoardViewModel {
        project_name: "kantui",
        states: &columns,
        focused_column: 0,
    };

    let out = render(60, 14, |buf| {
        let area = buf.area;
        BoardView::new(board, &theme).render(area, buf);
    });
    insta::assert_snapshot!("board_view_two_columns", out);
}

#[test]
fn status_bar_normal_mode() {
    let theme = Theme::default();
    let view = StatusBarView {
        mode: Mode::Normal,
        project: "kantui",
        state: "In progress",
        task_title: Some("Wire up status bar"),
        counts: StatusCounts {
            tasks_total: 12,
            tasks_done: 4,
        },
        clock: "10:42",
    };
    let out = render(80, 1, |buf| {
        let area = buf.area;
        StatusBar::new(view, &theme).render(area, buf);
    });
    insta::assert_snapshot!("status_bar_normal", out);
}

#[test]
fn status_bar_insert_mode_no_task() {
    let theme = Theme::default();
    let view = StatusBarView {
        mode: Mode::Insert,
        project: "kantui",
        state: "Todo",
        task_title: None,
        counts: StatusCounts {
            tasks_total: 0,
            tasks_done: 0,
        },
        clock: "10:42",
    };
    let out = render(60, 1, |buf| {
        let area = buf.area;
        StatusBar::new(view, &theme).render(area, buf);
    });
    insta::assert_snapshot!("status_bar_insert", out);
}

#[test]
fn input_rendered_with_prompt() {
    let theme = Theme::default();
    let mut state = InputState::with_value("new-board");
    state.move_left();
    state.move_left();
    let out = render(30, 1, |buf| {
        let area = buf.area;
        Input::new(&state, &theme).prefix(":").render(area, buf);
    });
    insta::assert_snapshot!("input_with_prompt", out);
}
