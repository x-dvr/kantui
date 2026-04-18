use super::ids::{ProjectId, StateId};

/// A workflow stage within a project. Rendered in the TUI as a column.
#[derive(Debug, Clone)]
pub struct State {
    pub id: StateId,
    pub project_id: ProjectId,
    pub name: String,
    /// Ordering within the project. Sparse integer scheme (steps of 1024 in
    /// the adapter) to keep reorders O(1).
    pub position: i32,
    pub wip_limit: Option<u32>,
}
