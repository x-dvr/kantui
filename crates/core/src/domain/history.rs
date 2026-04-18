use super::ids::{StateId, TaskId};
use super::time::{Duration, Timestamp};

/// Append-only log row: a task moved from one state into another at `at`.
/// `from_state` is `None` on task creation.
#[derive(Debug, Clone, Copy)]
pub struct TaskTransition {
    pub task_id: TaskId,
    pub from_state: Option<StateId>,
    pub to_state: StateId,
    pub at: Timestamp,
}

/// Aggregated sojourn time for a state, computed from the transition log.
#[derive(Debug, Clone, Copy)]
pub struct StateSojourn {
    pub state_id: StateId,
    pub total: Duration,
    pub count: u32,
}
