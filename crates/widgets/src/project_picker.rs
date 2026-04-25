//! Project picker overlay: a centered modal listing every project with a
//! cursor for selection. Read-only — the binary mutates state and rebuilds
//! the view on each frame.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::theme::Theme;

/// One project entry shown in the picker.
#[derive(Debug, Clone, Copy)]
pub struct ProjectPickerRow<'a> {
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub state_count: u32,
    pub task_count: u32,
    /// Marks the project currently loaded on the board.
    pub is_active: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectPickerView<'a> {
    pub projects: &'a [ProjectPickerRow<'a>],
    pub selected: Option<usize>,
}

pub struct ProjectPicker<'a> {
    pub view: ProjectPickerView<'a>,
    pub theme: &'a Theme,
}

impl<'a> ProjectPicker<'a> {
    #[must_use]
    pub const fn new(view: ProjectPickerView<'a>, theme: &'a Theme) -> Self {
        Self { view, theme }
    }
}

impl Widget for ProjectPicker<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = centered_rect(area, 70, 80);
        Clear.render(popup, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(Span::styled(
                " Projects ",
                Style::default()
                    .fg(self.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(self.theme.border_focused))
            .style(Style::default().bg(self.theme.background));

        let inner = block.inner(popup);
        block.render(popup, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let mut lines: Vec<Line<'_>> = Vec::with_capacity(self.view.projects.len() + 2);

        if self.view.projects.is_empty() {
            lines.push(Line::from(Span::styled(
                "(no projects — press `n` to create one)",
                Style::default().fg(self.theme.muted),
            )));
        } else {
            for (i, row) in self.view.projects.iter().enumerate() {
                let is_selected = self.view.selected == Some(i);
                lines.push(build_row(row, is_selected, self.theme));
            }
        }

        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "Enter open · e edit · n new · d delete · Esc close",
            Style::default().fg(self.theme.muted),
        )));

        Paragraph::new(lines)
            .style(Style::default().bg(self.theme.background))
            .render(inner, buf);
    }
}

fn build_row<'a>(row: &ProjectPickerRow<'a>, selected: bool, theme: &Theme) -> Line<'a> {
    let cursor = if selected { "▶ " } else { "  " };
    let active_marker = if row.is_active { " ●" } else { "" };
    let counts = format!("  ({} states · {} tasks)", row.state_count, row.task_count);

    let mut spans: Vec<Span<'a>> = Vec::new();
    let cursor_style = if selected {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };
    spans.push(Span::styled(cursor.to_owned(), cursor_style));

    let name_style = if selected {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.foreground)
    };
    spans.push(Span::styled(row.name.to_owned(), name_style));
    if !active_marker.is_empty() {
        spans.push(Span::styled(
            active_marker.to_owned(),
            Style::default().fg(theme.accent),
        ));
    }
    spans.push(Span::styled(counts, Style::default().fg(theme.muted)));

    if let Some(desc) = row.description {
        if !desc.trim().is_empty() {
            spans.push(Span::styled(
                format!("  — {desc}"),
                Style::default().fg(theme.muted),
            ));
        }
    }

    let mut line = Line::from(spans);
    if selected {
        line = line.style(Style::default().bg(theme.selection));
    }
    line
}

fn centered_rect(parent: Rect, pct_x: u16, pct_y: u16) -> Rect {
    let w = parent.width.saturating_mul(pct_x) / 100;
    let h = parent.height.saturating_mul(pct_y) / 100;
    let x = parent.x + (parent.width.saturating_sub(w)) / 2;
    let y = parent.y + (parent.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}
