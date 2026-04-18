use super::ids::ProjectId;
use super::state::State;
use super::time::Timestamp;

/// A kanban board. Owns an ordered list of [`State`]s.
#[derive(Debug, Clone)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub description: Option<String>,
    /// States ordered by `State::position` ascending.
    pub states: Vec<State>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
