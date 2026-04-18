use crate::domain::{EntityId, Project, ProjectId, State, StateId, Timestamp};
use crate::error::{CoreError, CoreResult, EntityKind};
use crate::ports::{Clock, IdGenerator, ProjectRepository};
use crate::services::position::{POSITION_STEP, append_after};

/// Input for [`ProjectService::create`]. `initial_states` are created in the
/// given order at equally-spaced positions.
#[derive(Debug, Clone)]
pub struct NewProject {
    pub name: String,
    pub description: Option<String>,
    pub initial_states: Vec<String>,
}

/// Input for [`ProjectService::add_state`].
#[derive(Debug, Clone)]
pub struct NewState {
    pub name: String,
    pub wip_limit: Option<u32>,
}

pub struct ProjectService<R, C, G>
where
    R: ProjectRepository,
    C: Clock,
    G: IdGenerator,
{
    repo: R,
    clock: C,
    ids: G,
}

impl<R, C, G> ProjectService<R, C, G>
where
    R: ProjectRepository,
    C: Clock,
    G: IdGenerator,
{
    pub fn new(repo: R, clock: C, ids: G) -> Self {
        Self { repo, clock, ids }
    }

    pub async fn create(&self, input: NewProject) -> CoreResult<Project> {
        let name = validate_name(&input.name, "project name")?;
        let now = self.clock.now();
        let project_id = ProjectId::new(self.ids.new_id());

        let mut states = Vec::with_capacity(input.initial_states.len());
        for (i, raw) in input.initial_states.iter().enumerate() {
            let clean = validate_name(raw, "state name")?;
            let position = POSITION_STEP.saturating_mul(i32::try_from(i + 1).unwrap_or(i32::MAX));
            states.push(State {
                id: StateId::new(self.ids.new_id()),
                project_id,
                name: clean,
                position,
                wip_limit: None,
            });
        }

        let project = Project {
            id: project_id,
            name,
            description: input.description.map(|d| d.trim().to_owned()),
            states,
            created_at: now,
            updated_at: now,
        };
        self.repo.create(&project).await?;
        Ok(project)
    }

    pub async fn get(&self, id: ProjectId) -> CoreResult<Project> {
        self.repo
            .get(id)
            .await?
            .ok_or_else(|| not_found(EntityKind::Project, id.inner()))
    }

    pub async fn list(&self) -> CoreResult<Vec<Project>> {
        self.repo.list().await
    }

    pub async fn rename(&self, id: ProjectId, new_name: &str) -> CoreResult<Project> {
        let name = validate_name(new_name, "project name")?;
        let mut project = self.get(id).await?;
        project.name = name;
        project.updated_at = self.clock.now();
        self.repo.update(&project).await?;
        Ok(project)
    }

    pub async fn delete(&self, id: ProjectId) -> CoreResult<()> {
        self.repo.delete(id).await
    }

    pub async fn add_state(&self, project_id: ProjectId, input: NewState) -> CoreResult<State> {
        let name = validate_name(&input.name, "state name")?;
        let project = self.get(project_id).await?;
        let last_pos = project.states.iter().map(|s| s.position).max();
        let state = State {
            id: StateId::new(self.ids.new_id()),
            project_id,
            name,
            position: append_after(last_pos),
            wip_limit: input.wip_limit,
        };
        self.repo.add_state(&state).await?;
        Ok(state)
    }

    pub async fn rename_state(&self, id: StateId, new_name: &str) -> CoreResult<State> {
        let name = validate_name(new_name, "state name")?;
        let mut state = self
            .repo
            .get_state(id)
            .await?
            .ok_or_else(|| not_found(EntityKind::State, id.inner()))?;
        state.name = name;
        self.repo.update_state(&state).await?;
        Ok(state)
    }

    pub async fn set_wip_limit(&self, id: StateId, wip_limit: Option<u32>) -> CoreResult<State> {
        let mut state = self
            .repo
            .get_state(id)
            .await?
            .ok_or_else(|| not_found(EntityKind::State, id.inner()))?;
        state.wip_limit = wip_limit;
        self.repo.update_state(&state).await?;
        Ok(state)
    }

    pub async fn remove_state(&self, id: StateId) -> CoreResult<()> {
        self.repo.remove_state(id).await
    }

    /// Reorder `project_id`'s states to match `ordered`. Every existing state
    /// must appear exactly once; unknown ids are rejected.
    pub async fn reorder_states(
        &self,
        project_id: ProjectId,
        ordered: &[StateId],
    ) -> CoreResult<()> {
        let project = self.get(project_id).await?;
        if ordered.len() != project.states.len() {
            return Err(CoreError::validation(
                "reorder list length must match existing states",
            ));
        }
        for s in &project.states {
            if !ordered.iter().any(|id| id.inner() == s.id.inner()) {
                return Err(CoreError::validation(format!(
                    "state {} missing from reorder list",
                    s.id
                )));
            }
        }
        self.repo.reorder_states(project_id, ordered).await
    }

    pub async fn get_state(&self, id: StateId) -> CoreResult<State> {
        self.repo
            .get_state(id)
            .await?
            .ok_or_else(|| not_found(EntityKind::State, id.inner()))
    }

    #[doc(hidden)]
    pub fn clock(&self) -> &C {
        &self.clock
    }

    #[doc(hidden)]
    pub fn _new_timestamp(&self) -> Timestamp {
        self.clock.now()
    }
}

fn validate_name(raw: &str, field: &str) -> CoreResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        Err(CoreError::validation(format!("{field} must not be empty")))
    } else {
        Ok(trimmed.to_owned())
    }
}

fn not_found(kind: EntityKind, id: EntityId) -> CoreError {
    CoreError::NotFound { entity: kind, id }
}
