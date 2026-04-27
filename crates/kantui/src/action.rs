//! User intent — the thing a keymap produces and the controller consumes.

/// A single discrete action resulting from a key (or key chord). Some
/// actions are pure state mutations; others require the controller to call
/// into `core` services asynchronously.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Do nothing. Used for unmapped keys and for the quiescent path after
    /// a chord prefix (e.g. `g` by itself).
    Noop,
    Quit,

    // --- Navigation (Normal mode) ---
    FocusPrevColumn,
    FocusNextColumn,
    SelectPrevTask,
    SelectNextTask,
    SelectFirstTask,
    SelectLastTask,

    // --- Begin Insert-mode flows (Normal mode) ---
    /// Start creating a new task *below* the currently selected one (or at
    /// the end of the column if nothing is selected).
    BeginNewTaskBelow,
    /// Start creating a new task *above* the currently selected one (or at
    /// the start of the column if nothing is selected).
    BeginNewTaskAbove,
    /// Start renaming the currently selected task.
    BeginRenameTask,

    // --- Task mutations executed immediately (Normal mode) ---
    DeleteTask,
    MoveTaskPrevColumn,
    MoveTaskNextColumn,
    ShiftTaskUp,
    ShiftTaskDown,

    // --- Prompt-mode key events (shared between Insert / Command / Search) ---
    InsertChar(char),
    InsertBackspace,
    InsertDelete,
    InsertMoveLeft,
    InsertMoveRight,
    InsertMoveHome,
    InsertMoveEnd,
    InsertSubmit,
    InsertCancel,

    // --- Begin Command / Search / Jump / Help ---
    BeginCommand,
    BeginSearch,
    BeginJump,
    ToggleHelp,
    /// Esc in Normal mode: close any overlay (help, ...), else noop.
    Escape,

    // --- Jump-mode key events ---
    JumpChar(char),
    JumpCancel,

    // --- Tag-picker mode ---
    /// Open the tag-picker overlay for the currently-selected task.
    BeginTagPicker,
    /// Press a single-char label to toggle the matching tag on/off.
    TagPickerChar(char),
    /// Dismiss the picker without changes.
    TagPickerCancel,

    // --- Dashboard overlay ---
    /// Refresh and open the statistics dashboard.
    OpenDashboard,
    /// Dismiss the dashboard.
    CloseDashboard,

    // --- Task detail overlay ---
    /// Open the task-detail overlay for the currently-selected task.
    OpenTaskDetail,
    /// Close the task-detail overlay (no save).
    CloseTaskDetail,
    /// Cycle the priority of the currently-detailed task.
    CycleTaskPriority,
    /// Cycle the complexity of the currently-detailed task.
    CycleTaskComplexity,
    /// Open a single-line prompt to edit the task description.
    BeginEditDescription,
    /// Open a single-line prompt to edit the due date (`YYYY-MM-DD` or empty
    /// to clear).
    BeginEditDueDate,

    // --- Project picker ---
    /// Open the project picker overlay.
    OpenProjectPicker,
    /// Dismiss the picker.
    CloseProjectPicker,
    /// Move selection up in the picker.
    PickerSelectPrev,
    /// Move selection down in the picker.
    PickerSelectNext,
    /// Switch the active project to the highlighted entry and return to the
    /// board.
    PickerActivate,
    /// Open the project editor on the highlighted entry.
    PickerEditSelected,
    /// Begin creating a new project (prompts for name).
    PickerNewProject,
    /// Delete the highlighted project.
    PickerDeleteSelected,

    // --- Project editor ---
    /// Dismiss the project editor (returns to picker if it was opened from
    /// the picker, else to the board).
    CloseProjectEditor,
    /// Move focus to the previous editor field / state row.
    EditorFocusPrev,
    /// Move focus to the next editor field / state row.
    EditorFocusNext,
    /// Begin editing the focused field (rename name/state, edit description).
    EditorBeginEdit,
    /// Begin editing the WIP limit of the focused state.
    EditorBeginEditWip,
    /// Begin adding a new state to the project under edit.
    EditorAddState,
    /// Delete the focused state.
    EditorDeleteState,
    /// Move the focused state up one position.
    EditorShiftStateUp,
    /// Move the focused state down one position.
    EditorShiftStateDown,
}

impl Action {
    /// Short, human-readable label for an action — used by the chord-help
    /// overlay to describe what each second key does.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Noop => "",
            Self::Quit => "Quit",
            Self::FocusPrevColumn => "Focus previous column",
            Self::FocusNextColumn => "Focus next column",
            Self::SelectPrevTask => "Select previous task",
            Self::SelectNextTask => "Select next task",
            Self::SelectFirstTask => "Top of column",
            Self::SelectLastTask => "Bottom of column",
            Self::BeginNewTaskBelow => "New task below",
            Self::BeginNewTaskAbove => "New task above",
            Self::BeginRenameTask => "Rename task",
            Self::DeleteTask => "Delete task",
            Self::MoveTaskPrevColumn => "Move task to previous column",
            Self::MoveTaskNextColumn => "Move task to next column",
            Self::ShiftTaskUp => "Shift task up",
            Self::ShiftTaskDown => "Shift task down",
            Self::InsertChar(_)
            | Self::InsertBackspace
            | Self::InsertDelete
            | Self::InsertMoveLeft
            | Self::InsertMoveRight
            | Self::InsertMoveHome
            | Self::InsertMoveEnd
            | Self::InsertSubmit
            | Self::InsertCancel => "",
            Self::BeginCommand => "Command mode",
            Self::BeginSearch => "Search",
            Self::BeginJump => "Jump to task",
            Self::ToggleHelp => "Toggle help",
            Self::Escape => "",
            Self::JumpChar(_) | Self::JumpCancel => "",
            Self::BeginTagPicker => "Tag picker",
            Self::TagPickerChar(_) | Self::TagPickerCancel => "",
            Self::OpenDashboard => "Statistics dashboard",
            Self::CloseDashboard => "",
            Self::OpenTaskDetail => "Task detail",
            Self::CloseTaskDetail => "",
            Self::CycleTaskPriority => "Cycle priority",
            Self::CycleTaskComplexity => "Cycle complexity",
            Self::BeginEditDescription => "Edit description",
            Self::BeginEditDueDate => "Edit due date",
            Self::OpenProjectPicker => "Project picker",
            Self::CloseProjectPicker => "",
            Self::PickerSelectPrev | Self::PickerSelectNext => "",
            Self::PickerActivate => "",
            Self::PickerEditSelected => "",
            Self::PickerNewProject => "",
            Self::PickerDeleteSelected => "",
            Self::CloseProjectEditor => "",
            Self::EditorFocusPrev | Self::EditorFocusNext => "",
            Self::EditorBeginEdit | Self::EditorBeginEditWip => "",
            Self::EditorAddState | Self::EditorDeleteState => "",
            Self::EditorShiftStateUp | Self::EditorShiftStateDown => "",
        }
    }
}
