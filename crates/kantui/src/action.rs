//! User intent — the thing a keymap produces and the app consumes. The
//! application's state machine is the sole place that mutates App state.

/// A single discrete action resulting from a key (or key chord).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Do nothing. Used for unmapped keys and the quiescent path after key
    /// chord prefixes (e.g. `g` by itself).
    Noop,
    Quit,

    // Navigation — all scoped to the current board.
    FocusPrevColumn,
    FocusNextColumn,
    SelectPrevTask,
    SelectNextTask,
    SelectFirstTask,
    SelectLastTask,

    // Help overlay toggles — placeholder for later milestones.
    ToggleHelp,
}
