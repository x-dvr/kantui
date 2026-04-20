//! Single-line text input. State lives in [`InputState`]; the widget only
//! draws. Key handling is the binary's responsibility.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::theme::Theme;

/// Mutable input state: the text and a cursor index in *characters* (not bytes).
#[derive(Debug, Default, Clone)]
pub struct InputState {
    value: String,
    cursor: usize,
}

impl InputState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_value(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        Self { value, cursor }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    /// Insert a character at the current cursor position and advance by one.
    pub fn insert_char(&mut self, ch: char) {
        let byte = self.byte_offset(self.cursor);
        self.value.insert(byte, ch);
        self.cursor += 1;
    }

    /// Delete the character left of the cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let target = self.cursor - 1;
        let start = self.byte_offset(target);
        let end = self.byte_offset(self.cursor);
        self.value.replace_range(start..end, "");
        self.cursor = target;
    }

    /// Delete the character at the cursor (forward delete).
    pub fn delete(&mut self) {
        let len = self.value.chars().count();
        if self.cursor >= len {
            return;
        }
        let start = self.byte_offset(self.cursor);
        let end = self.byte_offset(self.cursor + 1);
        self.value.replace_range(start..end, "");
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        let len = self.value.chars().count();
        if self.cursor < len {
            self.cursor += 1;
        }
    }

    pub fn move_to_start(&mut self) {
        self.cursor = 0;
    }

    pub fn move_to_end(&mut self) {
        self.cursor = self.value.chars().count();
    }

    fn byte_offset(&self, char_idx: usize) -> usize {
        self.value
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(self.value.len())
    }
}

/// Ratatui widget rendering an [`InputState`] with an optional prompt prefix.
pub struct Input<'a> {
    pub state: &'a InputState,
    pub theme: &'a Theme,
    pub prefix: Option<&'a str>,
}

impl<'a> Input<'a> {
    #[must_use]
    pub const fn new(state: &'a InputState, theme: &'a Theme) -> Self {
        Self {
            state,
            theme,
            prefix: None,
        }
    }

    #[must_use]
    pub const fn prefix(mut self, prefix: &'a str) -> Self {
        self.prefix = Some(prefix);
        self
    }
}

impl Widget for Input<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut spans: Vec<Span<'_>> = Vec::with_capacity(4);
        if let Some(prefix) = self.prefix {
            spans.push(Span::styled(
                prefix.to_owned(),
                Style::default().fg(self.theme.accent),
            ));
        }
        let (before, at, after) = split_around_cursor(self.state.value(), self.state.cursor());
        spans.push(Span::styled(
            before,
            Style::default().fg(self.theme.foreground),
        ));
        spans.push(Span::styled(
            at,
            Style::default()
                .fg(self.theme.background)
                .bg(self.theme.accent),
        ));
        spans.push(Span::styled(
            after,
            Style::default().fg(self.theme.foreground),
        ));

        Paragraph::new(Line::from(spans))
            .style(Style::default().bg(self.theme.status_bar))
            .render(area, buf);
    }
}

/// Split text into (before-cursor, at-cursor, after-cursor). The at-cursor
/// slice is always 1 char wide (or a space if the cursor is past the end) so
/// the caller can render it with an inverted style as a faux cursor.
fn split_around_cursor(value: &str, cursor: usize) -> (String, String, String) {
    let mut chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    if cursor >= len {
        let before: String = chars.into_iter().collect();
        return (before, " ".to_string(), String::new());
    }
    let after: String = chars.split_off(cursor + 1).into_iter().collect();
    let at = chars.pop().map(|c| c.to_string()).unwrap_or_default();
    let before: String = chars.into_iter().collect();
    (before, at, after)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_backspace() {
        let mut s = InputState::new();
        s.insert_char('h');
        s.insert_char('i');
        assert_eq!(s.value(), "hi");
        assert_eq!(s.cursor(), 2);
        s.backspace();
        assert_eq!(s.value(), "h");
        assert_eq!(s.cursor(), 1);
    }

    #[test]
    fn unicode_handled_by_char() {
        let mut s = InputState::with_value("héllo");
        assert_eq!(s.cursor(), 5);
        s.backspace();
        assert_eq!(s.value(), "héll");
    }

    #[test]
    fn move_cursor_bounds() {
        let mut s = InputState::with_value("ab");
        s.move_right();
        assert_eq!(s.cursor(), 2);
        s.move_left();
        s.move_left();
        s.move_left();
        assert_eq!(s.cursor(), 0);
    }

    #[test]
    fn split_around_cursor_middle() {
        let (b, a, af) = split_around_cursor("abc", 1);
        assert_eq!((b.as_str(), a.as_str(), af.as_str()), ("a", "b", "c"));
    }

    #[test]
    fn split_around_cursor_end() {
        let (b, a, af) = split_around_cursor("abc", 3);
        assert_eq!((b.as_str(), a.as_str(), af.as_str()), ("abc", " ", ""));
    }
}
