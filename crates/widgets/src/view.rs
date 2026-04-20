//! View models consumed by widgets.
//!
//! Widgets don't borrow domain entities directly because the binary may have
//! already resolved references (e.g. `TagId` → tag name+color) and because the
//! domain types carry fields widgets don't care about. These models are just
//! the presentable slice.

use kantui_core::{Color, Complexity, Priority, Timestamp};

/// Resolved tag ready to render as a chip.
#[derive(Debug, Clone, Copy)]
pub struct TagChip<'a> {
    pub name: &'a str,
    pub color: Color,
}

/// Everything a task card needs to render.
#[derive(Debug, Clone, Copy)]
pub struct TaskCardView<'a> {
    pub title: &'a str,
    pub priority: Priority,
    pub complexity: Complexity,
    pub due_date: Option<Timestamp>,
    pub tags: &'a [TagChip<'a>],
}

/// A single state column with its task list.
#[derive(Debug, Clone, Copy)]
pub struct StateColumnView<'a> {
    pub name: &'a str,
    pub wip_limit: Option<u32>,
    pub tasks: &'a [TaskCardView<'a>],
    /// Index of the selected task within `tasks`, if any.
    pub selected: Option<usize>,
}

/// A whole board — ordered list of state columns.
#[derive(Debug, Clone, Copy)]
pub struct BoardViewModel<'a> {
    pub project_name: &'a str,
    pub states: &'a [StateColumnView<'a>],
    /// Index of the focused column.
    pub focused_column: usize,
}

/// UI mode for coloring the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
    Search,
}

impl Mode {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Mode::Normal => "NOR",
            Mode::Insert => "INS",
            Mode::Command => "CMD",
            Mode::Search => "SEA",
        }
    }
}
