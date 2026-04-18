use async_trait::async_trait;

use crate::CoreResult;
use crate::domain::{Project, ProjectId, State, StateId};

#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn create(&self, project: &Project) -> CoreResult<()>;
    async fn get(&self, id: ProjectId) -> CoreResult<Option<Project>>;
    async fn list(&self) -> CoreResult<Vec<Project>>;
    async fn update(&self, project: &Project) -> CoreResult<()>;
    async fn delete(&self, id: ProjectId) -> CoreResult<()>;

    async fn add_state(&self, state: &State) -> CoreResult<()>;
    async fn update_state(&self, state: &State) -> CoreResult<()>;
    async fn remove_state(&self, id: StateId) -> CoreResult<()>;
    async fn reorder_states(&self, project_id: ProjectId, ordered: &[StateId]) -> CoreResult<()>;
    async fn get_state(&self, id: StateId) -> CoreResult<Option<State>>;
}
