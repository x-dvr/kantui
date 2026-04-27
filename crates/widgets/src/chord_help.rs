//! Bottom-right hint listing the second keys available for an in-flight chord.
//!
//! Shown after the user presses the first key of a Vim-style chord (e.g. `g`)
//! so they can see the possible completions (`gg`, `gw`, …) without leaving
//! Normal mode or opening the full help overlay.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::help::HelpRow;
use crate::theme::Theme;

/// Compact chord-completion hint anchored at the bottom-right of `area`.
pub struct ChordHelp<'a> {
    pub rows: &'a [HelpRow<'a>],
    pub theme: &'a Theme,
}

impl<'a> ChordHelp<'a> {
    #[must_use]
    pub const fn new(rows: &'a [HelpRow<'a>], theme: &'a Theme) -> Self {
        Self { rows, theme }
    }
}

impl Widget for ChordHelp<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.rows.is_empty() || area.width < 3 || area.height < 3 {
            return;
        }

        Clear.render(area, buf);

        let title = " Go to ... ";
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(Span::styled(
                title,
                Style::default()
                    .fg(self.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(self.theme.border_focused))
            .style(Style::default().bg(self.theme.background));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let keys_width = self
            .rows
            .iter()
            .map(|r| r.keys.chars().count())
            .max()
            .unwrap_or(0) as u16;

        let lines: Vec<Line<'_>> = self
            .rows
            .iter()
            .map(|row| build_row(row, keys_width, self.theme))
            .collect();

        Paragraph::new(lines)
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
