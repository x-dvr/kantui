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

/// Throughput over a rolling day window: total completions plus per-day
/// buckets where `per_day[0]` is the oldest day and the last entry is today.
#[derive(Debug, Clone)]
pub struct Throughput {
    pub done_state: StateId,
    pub total: u32,
    pub per_day: Vec<u32>,
}
