//! Mode-aware key dispatcher. Normal mode drives navigation and commands
//! via Vim-style chords; prompt modes (Insert/Command/Search) route
//! printable keys to the active [`InputState`] with Enter/Esc as
//! submit/cancel. Jump mode expects two characters matching a label.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::app::Mode;
use crate::keybinds::{Binding, Keybinds};

/// Mode-aware dispatcher. Holds the configured [`Keybinds`] plus the in-flight
/// chord prefix (for keys like `gg`).
#[derive(Debug)]
pub struct Keymap {
    binds: Keybinds,
    pending: Option<KeyEvent>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new()
    }
}

impl Keymap {
    #[must_use]
    pub fn new() -> Self {
        Self::with_binds(Keybinds::vim_default())
    }

    #[must_use]
    pub fn with_binds(binds: Keybinds) -> Self {
        Self {
            binds,
            pending: None,
        }
    }

    /// Swap in a new binding table. Clears any pending chord.
    pub fn set_binds(&mut self, binds: Keybinds) {
        self.binds = binds;
        self.pending = None;
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
            Mode::TagPicker => {
                self.pending = None;
                dispatch_tag_picker(key)
            }
            Mode::Dashboard => {
                self.pending = None;
                dispatch_dashboard(key)
            }
            Mode::TaskDetail => {
                self.pending = None;
                dispatch_task_detail(key)
            }
        }
    }

    fn dispatch_normal(&mut self, key: KeyEvent) -> Action {
        // Esc clears any pending chord and asks the controller to dismiss
        // overlays (help, ...).
        if key.code == KeyCode::Esc {
            self.pending = None;
            return Action::Escape;
        }

        // Ctrl-C is an unconditional quit; it never participates in chords.
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.pending = None;
            return Action::Quit;
        }

        // Chord resolution: if we had a pending prefix, try to match a chord.
        if let Some(first) = self.pending.take()
            && let Some(action) = match_chord(&self.binds, &first, &key)
        {
            return action;
        }

        // Single-key match.
        if let Some(action) = match_single(&self.binds, &key) {
            return action;
        }

        // First half of a chord? Stash and wait for the second key.
        if self.binds.is_chord_prefix(&key) {
            self.pending = Some(key);
        }

        Action::Noop
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

fn dispatch_tag_picker(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => Action::TagPickerCancel,
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::TagPickerCancel,
        (KeyCode::Char(ch), m) if m == KeyModifiers::NONE || m == KeyModifiers::SHIFT => {
            Action::TagPickerChar(ch.to_ascii_lowercase())
        }
        _ => Action::Noop,
    }
}

fn dispatch_dashboard(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => Action::CloseDashboard,
        (KeyCode::Char('q'), KeyModifiers::NONE) => Action::CloseDashboard,
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::CloseDashboard,
        _ => Action::Noop,
    }
}

fn dispatch_task_detail(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => Action::CloseTaskDetail,
        (KeyCode::Char('q'), KeyModifiers::NONE) => Action::CloseTaskDetail,
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::CloseTaskDetail,
        (KeyCode::Char('p'), KeyModifiers::NONE) => Action::CycleTaskPriority,
        (KeyCode::Char('e'), KeyModifiers::NONE) => Action::BeginEditDescription,
        (KeyCode::Char('D'), _) => Action::BeginEditDueDate,
        (KeyCode::Char('i'), KeyModifiers::NONE) => Action::BeginRenameTask,
        (KeyCode::Char('t'), KeyModifiers::NONE) => Action::BeginTagPicker,
        _ => Action::Noop,
    }
}

/// Walk every action's bindings and return the matching single-key action.
fn match_single(binds: &Keybinds, key: &KeyEvent) -> Option<Action> {
    for (list, action) in entries(binds) {
        for b in list {
            if let Binding::Single(k) = b
                && k.matches(key)
            {
                return Some(action);
            }
        }
    }
    None
}

/// Walk every action's bindings and return the matching chord action.
fn match_chord(binds: &Keybinds, first: &KeyEvent, second: &KeyEvent) -> Option<Action> {
    for (list, action) in entries(binds) {
        for b in list {
            if let Binding::Chord(a, z) = b
                && a.matches(first)
                && z.matches(second)
            {
                return Some(action);
            }
        }
    }
    None
}

fn entries(b: &Keybinds) -> [(&Vec<Binding>, Action); 22] {
    [
        (&b.quit, Action::Quit),
        (&b.focus_prev_column, Action::FocusPrevColumn),
        (&b.focus_next_column, Action::FocusNextColumn),
        (&b.select_next_task, Action::SelectNextTask),
        (&b.select_prev_task, Action::SelectPrevTask),
        (&b.select_first_task, Action::SelectFirstTask),
        (&b.select_last_task, Action::SelectLastTask),
        (&b.begin_jump, Action::BeginJump),
        (&b.open_dashboard, Action::OpenDashboard),
        (&b.begin_new_task_below, Action::BeginNewTaskBelow),
        (&b.begin_new_task_above, Action::BeginNewTaskAbove),
        (&b.begin_rename_task, Action::BeginRenameTask),
        (&b.delete_task, Action::DeleteTask),
        (&b.move_task_prev_column, Action::MoveTaskPrevColumn),
        (&b.move_task_next_column, Action::MoveTaskNextColumn),
        (&b.shift_task_up, Action::ShiftTaskUp),
        (&b.shift_task_down, Action::ShiftTaskDown),
        (&b.begin_tag_picker, Action::BeginTagPicker),
        (&b.begin_command, Action::BeginCommand),
        (&b.begin_search, Action::BeginSearch),
        (&b.toggle_help, Action::ToggleHelp),
        (&b.open_task_detail, Action::OpenTaskDetail),
    ]
}
