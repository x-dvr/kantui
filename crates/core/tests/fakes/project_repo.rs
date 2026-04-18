use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use kantui_core::{
    CoreError, CoreResult, EntityKind, Project, ProjectId, ProjectRepository, State, StateId,
};

#[derive(Clone)]
pub struct InMemoryProjectRepo {
    projects: Arc<Mutex<Vec<Project>>>,
}

impl InMemoryProjectRepo {
    pub fn new() -> Self {
        Self {
            projects: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

fn pid_eq(a: ProjectId, b: ProjectId) -> bool {
    a.inner().as_bytes() == b.inner().as_bytes()
}
fn sid_eq(a: StateId, b: StateId) -> bool {
    a.inner().as_bytes() == b.inner().as_bytes()
}

#[async_trait]
impl ProjectRepository for InMemoryProjectRepo {
    async fn create(&self, project: &Project) -> CoreResult<()> {
        let mut guard = self.projects.lock().unwrap();
        if guard.iter().any(|p| pid_eq(p.id, project.id)) {
            return Err(CoreError::conflict("duplicate project id"));
        }
        guard.push(project.clone());
        Ok(())
    }

    async fn get(&self, id: ProjectId) -> CoreResult<Option<Project>> {
        let guard = self.projects.lock().unwrap();
        Ok(guard.iter().find(|p| pid_eq(p.id, id)).cloned())
    }

    async fn list(&self) -> CoreResult<Vec<Project>> {
        Ok(self.projects.lock().unwrap().clone())
    }

    async fn update(&self, project: &Project) -> CoreResult<()> {
        let mut guard = self.projects.lock().unwrap();
        let slot =
            guard
                .iter_mut()
                .find(|p| pid_eq(p.id, project.id))
                .ok_or(CoreError::NotFound {
                    entity: EntityKind::Project,
                    id: project.id.inner(),
                })?;
        let states = std::mem::take(&mut slot.states);
        *slot = project.clone();
        slot.states = states;
        Ok(())
    }

    async fn delete(&self, id: ProjectId) -> CoreResult<()> {
        let mut guard = self.projects.lock().unwrap();
        let before = guard.len();
        guard.retain(|p| !pid_eq(p.id, id));
        if guard.len() == before {
            return Err(CoreError::NotFound {
                entity: EntityKind::Project,
                id: id.inner(),
            });
        }
        Ok(())
    }

    async fn add_state(&self, state: &State) -> CoreResult<()> {
        let mut guard = self.projects.lock().unwrap();
        let project = guard
            .iter_mut()
            .find(|p| pid_eq(p.id, state.project_id))
            .ok_or(CoreError::NotFound {
                entity: EntityKind::Project,
                id: state.project_id.inner(),
            })?;
        project.states.push(state.clone());
        project.states.sort_by_key(|s| s.position);
        Ok(())
    }

    async fn update_state(&self, state: &State) -> CoreResult<()> {
        let mut guard = self.projects.lock().unwrap();
        let project = guard
            .iter_mut()
            .find(|p| pid_eq(p.id, state.project_id))
            .ok_or(CoreError::NotFound {
                entity: EntityKind::Project,
                id: state.project_id.inner(),
            })?;
        let slot = project
            .states
            .iter_mut()
            .find(|s| sid_eq(s.id, state.id))
            .ok_or(CoreError::NotFound {
                entity: EntityKind::State,
                id: state.id.inner(),
            })?;
        *slot = state.clone();
        project.states.sort_by_key(|s| s.position);
        Ok(())
    }

    async fn remove_state(&self, id: StateId) -> CoreResult<()> {
        let mut guard = self.projects.lock().unwrap();
        for project in guard.iter_mut() {
            let before = project.states.len();
            project.states.retain(|s| !sid_eq(s.id, id));
            if project.states.len() != before {
                return Ok(());
            }
        }
        Err(CoreError::NotFound {
            entity: EntityKind::State,
            id: id.inner(),
        })
    }

    async fn reorder_states(&self, project_id: ProjectId, ordered: &[StateId]) -> CoreResult<()> {
        let mut guard = self.projects.lock().unwrap();
        let project =
            guard
                .iter_mut()
                .find(|p| pid_eq(p.id, project_id))
                .ok_or(CoreError::NotFound {
                    entity: EntityKind::Project,
                    id: project_id.inner(),
                })?;
        if project.states.len() != ordered.len() {
            return Err(CoreError::validation("reorder length mismatch"));
        }
        let step = 1024i32;
        for (i, sid) in ordered.iter().enumerate() {
            let pos = step.saturating_mul(i32::try_from(i + 1).unwrap_or(i32::MAX));
            let slot = project
                .states
                .iter_mut()
                .find(|s| sid_eq(s.id, *sid))
                .ok_or(CoreError::NotFound {
                    entity: EntityKind::State,
                    id: sid.inner(),
                })?;
            slot.position = pos;
        }
        project.states.sort_by_key(|s| s.position);
        Ok(())
    }

    async fn get_state(&self, id: StateId) -> CoreResult<Option<State>> {
        let guard = self.projects.lock().unwrap();
        for project in guard.iter() {
            if let Some(state) = project.states.iter().find(|s| sid_eq(s.id, id)) {
                return Ok(Some(state.clone()));
            }
        }
        Ok(None)
    }
}
