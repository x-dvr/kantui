//! A single kanban state rendered as a vertical column of task cards.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::task_card::TaskCard;
use crate::theme::Theme;
use crate::view::StateColumnView;

/// Height of a task card in rows (3 borders + 3 content).
const TASK_CARD_ROWS: u16 = 6;

/// Renders a state column: header + task cards stacked vertically.
pub struct StateColumn<'a> {
    pub view: StateColumnView<'a>,
    pub theme: &'a Theme,
    pub focused: bool,
}

impl<'a> StateColumn<'a> {
    #[must_use]
    pub const fn new(view: StateColumnView<'a>, theme: &'a Theme) -> Self {
        Self {
            view,
            theme,
            focused: false,
        }
    }

    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

impl Widget for StateColumn<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_color = if self.focused {
            self.theme.border_focused
        } else {
            self.theme.border
        };

        let title = build_column_title(&self.view, self.theme);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        // Build constraints for each visible task + a final Min(0) filler so
        // short columns don't stretch their cards vertically.
        let visible = visible_tasks(inner.height, self.view.tasks.len());
        if visible == 0 {
            return;
        }

        let mut constraints: Vec<Constraint> = (0..visible)
            .map(|_| Constraint::Length(TASK_CARD_ROWS))
            .collect();
        constraints.push(Constraint::Min(0));

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(inner);

        for (i, task) in self.view.tasks.iter().take(visible).enumerate() {
            let selected = self.view.selected == Some(i);
            TaskCard::new(*task, self.theme)
                .selected(selected)
                .render(rows[i], buf);
        }
    }
}

fn build_column_title<'a>(view: &StateColumnView<'a>, theme: &Theme) -> Line<'a> {
    let mut spans: Vec<Span<'a>> = vec![
        Span::raw(" "),
        Span::styled(
            view.name.to_owned(),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{}", view.tasks.len()),
            Style::default().fg(theme.muted),
        ),
    ];
    if let Some(limit) = view.wip_limit {
        spans.push(Span::styled(
            format!("/{limit}"),
            Style::default().fg(theme.muted),
        ));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

fn visible_tasks(inner_height: u16, task_count: usize) -> usize {
    ((inner_height / TASK_CARD_ROWS) as usize).min(task_count)
}

/// Empty-state placeholder used when a column has no tasks and we still want
/// to fill the inner area with a subtle hint. Not wired by default — columns
/// simply render their header when empty.
pub fn empty_hint<'a>(message: &'a str, theme: &Theme) -> Paragraph<'a> {
    Paragraph::new(Line::from(Span::styled(
        message,
        Style::default().fg(theme.muted),
    )))
}
