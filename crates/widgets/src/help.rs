//! Centered modal widget showing a keybind cheatsheet.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::theme::Theme;

/// One row in the cheatsheet: a keys label and a description.
#[derive(Debug, Clone, Copy)]
pub struct HelpRow<'a> {
    pub keys: &'a str,
    pub description: &'a str,
}

pub struct HelpOverlay<'a> {
    pub title: &'a str,
    pub rows: &'a [HelpRow<'a>],
    pub theme: &'a Theme,
}

impl<'a> HelpOverlay<'a> {
    #[must_use]
    pub const fn new(title: &'a str, rows: &'a [HelpRow<'a>], theme: &'a Theme) -> Self {
        Self { title, rows, theme }
    }
}

impl Widget for HelpOverlay<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = centered_rect(area, 70, 95);
        // Clear the area first so the overlay doesn't blend with whatever
        // was beneath it.
        Clear.render(popup, buf);

        let keys_width = self
            .rows
            .iter()
            .map(|r| r.keys.chars().count())
            .max()
            .unwrap_or(0) as u16;

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(Span::styled(
                format!(" {} ", self.title),
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

        let rows: Vec<Line<'_>> = self
            .rows
            .iter()
            .map(|row| build_row(row, keys_width, self.theme))
            .collect();

        Paragraph::new(rows)
            .style(Style::default().bg(self.theme.background))
            .render(inner, buf);
    }
}

fn build_row<'a>(row: &HelpRow<'a>, keys_width: u16, theme: &Theme) -> Line<'a> {
    let pad = (keys_width as usize)
        .saturating_sub(row.keys.chars().count())
        .saturating_add(2);
    let padding: String = " ".repeat(pad);
    Line::from(vec![
        Span::styled(
            row.keys.to_owned(),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(padding),
        Span::styled(
            row.description.to_owned(),
            Style::default().fg(theme.foreground),
        ),
    ])
}

/// Centered rectangle: `percent_x` × `percent_y` of `area`.
fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let width = area.width.saturating_mul(percent_x) / 100;
    let height = area.height.saturating_mul(percent_y) / 100;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}
