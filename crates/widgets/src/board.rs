//! Top-level board: horizontal strip of state columns.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::Widget;

use crate::state_column::StateColumn;
use crate::theme::Theme;
use crate::view::BoardViewModel;

/// Minimum width (in cells) for a single state column so its borders and
/// contents don't collapse. If the area can't fit this for every column,
/// columns are still split equally — the binary is free to add scrolling on
/// top of this widget when it comes time.
const MIN_COLUMN_WIDTH: u16 = 20;

pub struct BoardView<'a> {
    pub view: BoardViewModel<'a>,
    pub theme: &'a Theme,
}

impl<'a> BoardView<'a> {
    #[must_use]
    pub const fn new(view: BoardViewModel<'a>, theme: &'a Theme) -> Self {
        Self { view, theme }
    }
}

impl Widget for BoardView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.view.states.is_empty() || area.width == 0 || area.height == 0 {
            return;
        }

        let n = self.view.states.len() as u16;
        let column_width = (area.width / n).max(MIN_COLUMN_WIDTH.min(area.width));
        let constraints: Vec<Constraint> =
            (0..n).map(|_| Constraint::Length(column_width)).collect();

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        for (i, state) in self.view.states.iter().enumerate() {
            let focused = i == self.view.focused_column;
            StateColumn::new(*state, self.theme)
                .focused(focused)
                .render(columns[i], buf);
        }
    }
}
