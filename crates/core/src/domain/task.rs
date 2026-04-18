use super::ids::{ProjectId, StateId, TagId, TaskId};
use super::time::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Complexity {
    Light,
    Deep,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub project_id: ProjectId,
    pub state_id: StateId,
    pub title: String,
    pub description: Option<String>,
    pub priority: Priority,
    pub complexity: Complexity,
    pub due_date: Option<Timestamp>,
    pub tags: Vec<TagId>,
    /// Ordering within the owning state.
    pub position: i32,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
