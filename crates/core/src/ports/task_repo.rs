use async_trait::async_trait;

use crate::CoreResult;
use crate::domain::{ProjectId, StateId, Task, TaskId, TaskTransition, Timestamp};

#[async_trait]
pub trait TaskRepository: Send + Sync {
    /// Insert a task and append its creation [`TaskTransition`]
    /// (from_state = `None`) atomically.
    async fn create(&self, task: &Task, at: Timestamp) -> CoreResult<()>;

    async fn get(&self, id: TaskId) -> CoreResult<Option<Task>>;
    async fn list_by_state(&self, state_id: StateId) -> CoreResult<Vec<Task>>;
    async fn list_by_project(&self, project_id: ProjectId) -> CoreResult<Vec<Task>>;
    async fn count_in_state(&self, state_id: StateId) -> CoreResult<u32>;

    async fn update(&self, task: &Task) -> CoreResult<()>;

    /// Move a task to `target_state` at `target_position`. Updates the task row
    /// and appends a [`TaskTransition`] row in a single transaction.
    async fn move_task(
        &self,
        task_id: TaskId,
        target_state: StateId,
        target_position: i32,
        at: Timestamp,
    ) -> CoreResult<()>;

    async fn delete(&self, id: TaskId) -> CoreResult<()>;

    async fn list_transitions(&self, task_id: TaskId) -> CoreResult<Vec<TaskTransition>>;
    async fn list_project_transitions(
        &self,
        project_id: ProjectId,
    ) -> CoreResult<Vec<TaskTransition>>;
}
