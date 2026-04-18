use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use kantui_core::{
    CoreError, CoreResult, EntityKind, ProjectId, StateId, Task, TaskId, TaskRepository,
    TaskTransition, Timestamp,
};

fn eq_id<L: AsId, R: AsId>(a: &L, b: &R) -> bool {
    a.id_bytes() == b.id_bytes()
}
trait AsId {
    fn id_bytes(&self) -> [u8; 16];
}
impl AsId for TaskId {
    fn id_bytes(&self) -> [u8; 16] {
        *self.inner().as_bytes()
    }
}
impl AsId for StateId {
    fn id_bytes(&self) -> [u8; 16] {
        *self.inner().as_bytes()
    }
}
impl AsId for ProjectId {
    fn id_bytes(&self) -> [u8; 16] {
        *self.inner().as_bytes()
    }
}

struct Store {
    tasks: Vec<Task>,
    transitions: Vec<TaskTransition>,
}

#[derive(Clone)]
pub struct InMemoryTaskRepo {
    inner: Arc<Mutex<Store>>,
}

impl InMemoryTaskRepo {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Store {
                tasks: Vec::new(),
                transitions: Vec::new(),
            })),
        }
    }
}

#[async_trait]
impl TaskRepository for InMemoryTaskRepo {
    async fn create(&self, task: &Task, at: Timestamp) -> CoreResult<()> {
        let mut store = self.inner.lock().unwrap();
        if store.tasks.iter().any(|t| eq_id(&t.id, &task.id)) {
            return Err(CoreError::conflict("duplicate task id"));
        }
        store.tasks.push(task.clone());
        store.transitions.push(TaskTransition {
            task_id: task.id,
            from_state: None,
            to_state: task.state_id,
            at,
        });
        Ok(())
    }

    async fn get(&self, id: TaskId) -> CoreResult<Option<Task>> {
        let store = self.inner.lock().unwrap();
        Ok(store.tasks.iter().find(|t| eq_id(&t.id, &id)).cloned())
    }

    async fn list_by_state(&self, state_id: StateId) -> CoreResult<Vec<Task>> {
        let store = self.inner.lock().unwrap();
        let mut out: Vec<Task> = store
            .tasks
            .iter()
            .filter(|t| eq_id(&t.state_id, &state_id))
            .cloned()
            .collect();
        out.sort_by_key(|t| t.position);
        Ok(out)
    }

    async fn list_by_project(&self, project_id: ProjectId) -> CoreResult<Vec<Task>> {
        let store = self.inner.lock().unwrap();
        Ok(store
            .tasks
            .iter()
            .filter(|t| eq_id(&t.project_id, &project_id))
            .cloned()
            .collect())
    }

    async fn count_in_state(&self, state_id: StateId) -> CoreResult<u32> {
        let store = self.inner.lock().unwrap();
        let c = store
            .tasks
            .iter()
            .filter(|t| eq_id(&t.state_id, &state_id))
            .count();
        Ok(u32::try_from(c).unwrap_or(u32::MAX))
    }

    async fn update(&self, task: &Task) -> CoreResult<()> {
        let mut store = self.inner.lock().unwrap();
        let slot = store
            .tasks
            .iter_mut()
            .find(|t| eq_id(&t.id, &task.id))
            .ok_or(CoreError::NotFound {
                entity: EntityKind::Task,
                id: task.id.inner(),
            })?;
        *slot = task.clone();
        Ok(())
    }

    async fn move_task(
        &self,
        task_id: TaskId,
        target_state: StateId,
        target_position: i32,
        at: Timestamp,
    ) -> CoreResult<()> {
        let mut store = self.inner.lock().unwrap();
        let task = store
            .tasks
            .iter_mut()
            .find(|t| eq_id(&t.id, &task_id))
            .ok_or(CoreError::NotFound {
                entity: EntityKind::Task,
                id: task_id.inner(),
            })?;
        let from = task.state_id;
        task.state_id = target_state;
        task.position = target_position;
        task.updated_at = at;
        store.transitions.push(TaskTransition {
            task_id,
            from_state: Some(from),
            to_state: target_state,
            at,
        });
        Ok(())
    }

    async fn delete(&self, id: TaskId) -> CoreResult<()> {
        let mut store = self.inner.lock().unwrap();
        let before = store.tasks.len();
        store.tasks.retain(|t| !eq_id(&t.id, &id));
        if store.tasks.len() == before {
            return Err(CoreError::NotFound {
                entity: EntityKind::Task,
                id: id.inner(),
            });
        }
        Ok(())
    }

    async fn list_transitions(&self, task_id: TaskId) -> CoreResult<Vec<TaskTransition>> {
        let store = self.inner.lock().unwrap();
        let mut out: Vec<TaskTransition> = store
            .transitions
            .iter()
            .filter(|t| eq_id(&t.task_id, &task_id))
            .copied()
            .collect();
        out.sort_by_key(|t| t.at);
        Ok(out)
    }

    async fn list_project_transitions(
        &self,
        project_id: ProjectId,
    ) -> CoreResult<Vec<TaskTransition>> {
        let store = self.inner.lock().unwrap();
        let task_ids: Vec<TaskId> = store
            .tasks
            .iter()
            .filter(|t| eq_id(&t.project_id, &project_id))
            .map(|t| t.id)
            .collect();
        Ok(store
            .transitions
            .iter()
            .filter(|tr| task_ids.iter().any(|tid| eq_id(&tr.task_id, tid)))
            .copied()
            .collect())
    }
}
