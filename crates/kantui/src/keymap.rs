//! Normal-mode key dispatcher. A tiny state machine handles Vim-style chords
//! like `gg` / `G`. Kept intentionally simple — count prefixes and command
//! mode arrive in M5/M6.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;

/// Result of dispatching a key: either an action to run or a chord-pending
/// indicator so the UI can show it.
#[derive(Debug, Default)]
pub struct Keymap {
    /// Currently-accumulated chord prefix (e.g. Some('g') after pressing `g`).
    pending: Option<char>,
}

impl Keymap {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn dispatch(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Esc {
            self.pending = None;
            return Action::Noop;
        }

        // Chord: `g` followed by `g` → top, `w` → jump (unimplemented).
        if let Some('g') = self.pending {
            self.pending = None;
            return match key.code {
                KeyCode::Char('g') => Action::SelectFirstTask,
                _ => Action::Noop,
            };
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::NONE) => Action::Quit,
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::Quit,
            (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::Left, _) => {
                Action::FocusPrevColumn
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) | (KeyCode::Right, _) => {
                Action::FocusNextColumn
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => Action::SelectNextTask,
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => Action::SelectPrevTask,
            (KeyCode::Char('G'), _) => Action::SelectLastTask,
            (KeyCode::Char('g'), KeyModifiers::NONE) => {
                self.pending = Some('g');
                Action::Noop
            }
            (KeyCode::Char('?'), _) => Action::ToggleHelp,
            _ => Action::Noop,
        }
    }
}
