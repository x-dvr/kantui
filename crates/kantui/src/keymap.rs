//! Mode-aware key dispatcher. Normal mode drives navigation and commands
//! via Vim-style chords; prompt modes (Insert/Command/Search) route
//! printable keys to the active [`InputState`] with Enter/Esc as
//! submit/cancel. Jump mode expects two characters matching a label.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::app::Mode;

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

    pub fn dispatch(&mut self, mode: Mode, key: KeyEvent) -> Action {
        match mode {
            Mode::Normal => self.dispatch_normal(key),
            Mode::Insert | Mode::Command | Mode::Search => {
                self.pending = None;
                dispatch_prompt(key)
            }
            Mode::Jump => {
                self.pending = None;
                dispatch_jump(key)
            }
        }
    }

    fn dispatch_normal(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Esc {
            self.pending = None;
            return Action::Noop;
        }

        if let Some(prefix) = self.pending.take() {
            return match (prefix, key.code) {
                ('g', KeyCode::Char('g')) => Action::SelectFirstTask,
                ('g', KeyCode::Char('w')) => Action::BeginJump,
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

            // Task CRUD & movement.
            (KeyCode::Char('n'), KeyModifiers::NONE) => Action::BeginNewTaskBelow,
            (KeyCode::Char('N'), _) => Action::BeginNewTaskAbove,
            (KeyCode::Char('i'), KeyModifiers::NONE) => Action::BeginRenameTask,
            (KeyCode::Char('d'), KeyModifiers::NONE) => Action::DeleteTask,
            (KeyCode::Char('H'), _) => Action::MoveTaskPrevColumn,
            (KeyCode::Char('L'), _) => Action::MoveTaskNextColumn,
            (KeyCode::Char('K'), _) => Action::ShiftTaskUp,
            (KeyCode::Char('J'), _) => Action::ShiftTaskDown,

            // Prompt-mode entries.
            (KeyCode::Char(':'), _) => Action::BeginCommand,
            (KeyCode::Char('/'), _) => Action::BeginSearch,
            (KeyCode::Char('?'), _) => Action::ToggleHelp,

            _ => Action::Noop,
        }
    }
}

fn dispatch_prompt(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => Action::InsertCancel,
        (KeyCode::Enter, _) => Action::InsertSubmit,
        (KeyCode::Backspace, _) => Action::InsertBackspace,
        (KeyCode::Delete, _) => Action::InsertDelete,
        (KeyCode::Left, _) => Action::InsertMoveLeft,
        (KeyCode::Right, _) => Action::InsertMoveRight,
        (KeyCode::Home, _) => Action::InsertMoveHome,
        (KeyCode::End, _) => Action::InsertMoveEnd,
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::InsertCancel,
        (KeyCode::Char(ch), m) if m == KeyModifiers::NONE || m == KeyModifiers::SHIFT => {
            Action::InsertChar(ch)
        }
        _ => Action::Noop,
    }
}

fn dispatch_jump(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => Action::JumpCancel,
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::JumpCancel,
        (KeyCode::Char(ch), m) if m == KeyModifiers::NONE || m == KeyModifiers::SHIFT => {
            Action::JumpChar(ch.to_ascii_lowercase())
        }
        _ => Action::Noop,
    }
}
