//! Two-letter jump labels overlaid on the board.
//!
//! The binary decides *where* labels go (see `kantui::jump`) — this widget
//! only renders a supplied list of label strings at their target rectangles.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::theme::Theme;

/// A single label + the cell where its top-left corner should land.
#[derive(Debug, Clone, Copy)]
pub struct JumpLabelView<'a> {
    pub label: &'a str,
    pub x: u16,
    pub y: u16,
}

pub struct JumpLabels<'a> {
    pub labels: &'a [JumpLabelView<'a>],
    pub theme: &'a Theme,
    /// If set, only labels whose first character matches are highlighted.
    pub pending_prefix: Option<char>,
}

impl<'a> JumpLabels<'a> {
    #[must_use]
    pub const fn new(labels: &'a [JumpLabelView<'a>], theme: &'a Theme) -> Self {
        Self {
            labels,
            theme,
            pending_prefix: None,
        }
    }

    #[must_use]
    pub const fn pending_prefix(mut self, prefix: Option<char>) -> Self {
        self.pending_prefix = prefix;
        self
    }
}

impl Widget for JumpLabels<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for label in self.labels {
            if label.x >= area.x + area.width || label.y >= area.y + area.height {
                continue;
            }
            let width = (label.label.chars().count() as u16).min(area.width);
            if width == 0 {
                continue;
            }
            let rect = Rect {
                x: label.x,
                y: label.y,
                width,
                height: 1,
            };

            let (bg, fg) = match self.pending_prefix {
                Some(prefix) if label.label.starts_with(prefix) => {
                    (self.theme.accent, self.theme.background)
                }
                _ => (self.theme.mode_command, self.theme.background),
            };

            Paragraph::new(Line::from(Span::styled(
                label.label.to_owned(),
                Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
            )))
            .style(Style::default().bg(bg))
            .render(rect, buf);
        }
    }
}
