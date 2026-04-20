//! Reusable ratatui widgets rendering `kantui_core` view models.
//!
//! Widgets are dumb: no I/O, no repository references. Each widget accepts a
//! small read-only view model (see [`view`]) and a [`Theme`] reference. The
//! binary assembles view models from domain entities and hands them in.

pub mod board;
pub mod input;
pub mod state_column;
pub mod status_bar;
pub mod task_card;
pub mod theme;
pub mod view;

pub use board::BoardView;
pub use input::{Input, InputState};
pub use state_column::StateColumn;
pub use status_bar::{StatusBar, StatusBarView, StatusCounts};
pub use task_card::TaskCard;
pub use theme::{Theme, map_domain_color};
pub use view::{BoardViewModel, Mode, StateColumnView, TagChip, TaskCardView};
