use crate::domain::{Complexity, EntityId, Priority, ProjectId, StateId, Task, TaskId, Timestamp};
use crate::error::{CoreError, CoreResult, EntityKind};
use crate::ports::{Clock, IdGenerator, ProjectRepository, TaskRepository};
use crate::services::position::{append_after, between};

#[derive(Debug, Clone)]
pub struct NewTask {
    pub project_id: ProjectId,
    pub state_id: StateId,
    pub title: String,
    pub description: Option<String>,
    pub priority: Priority,
    pub complexity: Complexity,
}

impl NewTask {
    #[must_use]
    pub fn new(project_id: ProjectId, state_id: StateId, title: impl Into<String>) -> Self {
        Self {
            project_id,
            state_id,
            title: title.into(),
            description: None,
            priority: Priority::Normal,
            complexity: Complexity::Light,
        }
    }
}

/// Partial update for [`TaskService::update`]. Only `Some` fields are changed.
#[derive(Debug, Clone, Default)]
pub struct TaskUpdate {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<Priority>,
    pub complexity: Option<Complexity>,
    pub due_date: Option<Option<Timestamp>>,
}

pub struct TaskService<PR, TR, C, G>
where
    PR: ProjectRepository,
    TR: TaskRepository,
    C: Clock,
    G: IdGenerator,
{
    projects: PR,
    tasks: TR,
    clock: C,
    ids: G,
}

impl<PR, TR, C, G> TaskService<PR, TR, C, G>
where
    PR: ProjectRepository,
    TR: TaskRepository,
    C: Clock,
    G: IdGenerator,
{
    pub fn new(projects: PR, tasks: TR, clock: C, ids: G) -> Self {
        Self {
            projects,
            tasks,
            clock,
            ids,
        }
    }

    pub async fn create(&self, input: NewTask) -> CoreResult<Task> {
        let title = validate_title(&input.title)?;
        let state = self.load_state(input.state_id).await?;
        if state.project_id.inner() != input.project_id.inner() {
            return Err(CoreError::validation(
                "state does not belong to the given project",
            ));
        }
        self.enforce_wip(input.state_id, state.wip_limit).await?;

        let now = self.clock.now();
        let last_pos = self.last_position_in_state(input.state_id).await?;
        let task = Task {
            id: TaskId::new(self.ids.new_id()),
            project_id: input.project_id,
            state_id: input.state_id,
            title,
            description: input.description.map(|d| d.trim().to_owned()),
            priority: input.priority,
            complexity: input.complexity,
            due_date: None,
            tags: Vec::new(),
            position: append_after(last_pos),
            created_at: now,
            updated_at: now,
        };
        self.tasks.create(&task, now).await?;
        Ok(task)
    }

    pub async fn get(&self, id: TaskId) -> CoreResult<Task> {
        self.tasks
            .get(id)
            .await?
            .ok_or_else(|| not_found(EntityKind::Task, id.inner()))
    }

    pub async fn list_in_state(&self, state_id: StateId) -> CoreResult<Vec<Task>> {
        self.tasks.list_by_state(state_id).await
    }

    pub async fn list_in_project(&self, project_id: ProjectId) -> CoreResult<Vec<Task>> {
        self.tasks.list_by_project(project_id).await
    }

    pub async fn update(&self, id: TaskId, update: TaskUpdate) -> CoreResult<Task> {
        let mut task = self.get(id).await?;
        if let Some(title) = update.title {
            task.title = validate_title(&title)?;
        }
        if let Some(desc) = update.description {
            task.description = desc.map(|d| d.trim().to_owned());
        }
        if let Some(priority) = update.priority {
            task.priority = priority;
        }
        if let Some(complexity) = update.complexity {
            task.complexity = complexity;
        }
        if let Some(due) = update.due_date {
            task.due_date = due;
        }
        task.updated_at = self.clock.now();
        self.tasks.update(&task).await?;
        Ok(task)
    }

    /// Move a task to `target_state`. If `after` is `None`, the task goes to
    /// the front of the column; otherwise it goes just after the named task.
    pub async fn move_task(
        &self,
        task_id: TaskId,
        target_state: StateId,
        after: Option<TaskId>,
    ) -> CoreResult<()> {
        let task = self.get(task_id).await?;
        let target = self.load_state(target_state).await?;
        if target.project_id.inner() != task.project_id.inner() {
            return Err(CoreError::validation("cannot move task across projects"));
        }

        if target_state.inner() != task.state_id.inner() {
            let current_in_target = self.tasks.count_in_state(target_state).await?;
            if let Some(limit) = target.wip_limit
                && current_in_target >= limit
            {
                return Err(CoreError::WipLimitExceeded {
                    state: target_state,
                    limit,
                });
            }
        }

        let position = self
            .resolve_insert_position(target_state, after, Some(task_id))
            .await?;
        let at = self.clock.now();
        self.tasks
            .move_task(task_id, target_state, position, at)
            .await
    }

    pub async fn delete(&self, id: TaskId) -> CoreResult<()> {
        self.tasks.delete(id).await
    }

    async fn resolve_insert_position(
        &self,
        state_id: StateId,
        after: Option<TaskId>,
        skip: Option<TaskId>,
    ) -> CoreResult<i32> {
        let mut siblings: Vec<_> = self
            .tasks
            .list_by_state(state_id)
            .await?
            .into_iter()
            .filter(|t| skip.map(|s| s.inner() != t.id.inner()).unwrap_or(true))
            .collect();
        siblings.sort_by_key(|t| t.position);

        let (prev, next) = match after {
            None => (None, siblings.first().map(|t| t.position)),
            Some(anchor) => {
                let idx = siblings
                    .iter()
                    .position(|t| t.id.inner() == anchor.inner())
                    .ok_or_else(|| {
                        CoreError::validation("anchor task not found in target state")
                    })?;
                let prev = Some(siblings[idx].position);
                let next = siblings.get(idx + 1).map(|t| t.position);
                (prev, next)
            }
        };

        between(prev, next)
            .ok_or_else(|| CoreError::conflict("position gap exhausted; rebalance required"))
    }

    async fn load_state(&self, id: StateId) -> CoreResult<crate::domain::State> {
        self.projects
            .get_state(id)
            .await?
            .ok_or_else(|| not_found(EntityKind::State, id.inner()))
    }

    async fn last_position_in_state(&self, state_id: StateId) -> CoreResult<Option<i32>> {
        Ok(self
            .tasks
            .list_by_state(state_id)
            .await?
            .iter()
            .map(|t| t.position)
            .max())
    }

    async fn enforce_wip(&self, state_id: StateId, limit: Option<u32>) -> CoreResult<()> {
        let Some(limit) = limit else { return Ok(()) };
        let current = self.tasks.count_in_state(state_id).await?;
        if current >= limit {
            Err(CoreError::WipLimitExceeded {
                state: state_id,
                limit,
            })
        } else {
            Ok(())
        }
    }
}

fn validate_title(raw: &str) -> CoreResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        Err(CoreError::validation("task title must not be empty"))
    } else {
        Ok(trimmed.to_owned())
    }
}

fn not_found(kind: EntityKind, id: EntityId) -> CoreError {
    CoreError::NotFound { entity: kind, id }
}
