use async_trait::async_trait;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use kantui_core::{
    CoreError, CoreResult, EntityKind, Project, ProjectId, ProjectRepository, State, StateId,
};

use super::sqlx_err;
use crate::mapping::{id_from_text, id_to_text, ts_from_millis, ts_to_millis};

pub struct SqliteProjectRepo {
    pool: SqlitePool,
}

impl SqliteProjectRepo {
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn row_to_state(row: SqliteRow) -> CoreResult<State> {
    let id: String = row.try_get("id").map_err(sqlx_err("state.id"))?;
    let project_id: String = row
        .try_get("project_id")
        .map_err(sqlx_err("state.project_id"))?;
    let name: String = row.try_get("name").map_err(sqlx_err("state.name"))?;
    let position: i64 = row
        .try_get("position")
        .map_err(sqlx_err("state.position"))?;
    let wip_limit: Option<i64> = row
        .try_get("wip_limit")
        .map_err(sqlx_err("state.wip_limit"))?;
    Ok(State {
        id: StateId::new(id_from_text(&id)?),
        project_id: ProjectId::new(id_from_text(&project_id)?),
        name,
        position: i32::try_from(position).unwrap_or(i32::MAX),
        wip_limit: wip_limit.map(|n| u32::try_from(n).unwrap_or(0)),
    })
}

fn row_to_project(row: &SqliteRow, states: Vec<State>) -> CoreResult<Project> {
    let id: String = row.try_get("id").map_err(sqlx_err("project.id"))?;
    let name: String = row.try_get("name").map_err(sqlx_err("project.name"))?;
    let description: Option<String> = row
        .try_get("description")
        .map_err(sqlx_err("project.description"))?;
    let created_at: i64 = row
        .try_get("created_at")
        .map_err(sqlx_err("project.created_at"))?;
    let updated_at: i64 = row
        .try_get("updated_at")
        .map_err(sqlx_err("project.updated_at"))?;
    Ok(Project {
        id: ProjectId::new(id_from_text(&id)?),
        name,
        description,
        states,
        created_at: ts_from_millis(created_at),
        updated_at: ts_from_millis(updated_at),
    })
}

async fn load_states(pool: &SqlitePool, project_id: ProjectId) -> CoreResult<Vec<State>> {
    let rows = sqlx::query(
        "SELECT id, project_id, name, position, wip_limit
         FROM states WHERE project_id = ? ORDER BY position ASC",
    )
    .bind(id_to_text(project_id.inner()))
    .fetch_all(pool)
    .await
    .map_err(sqlx_err("select states"))?;
    rows.into_iter().map(row_to_state).collect()
}

#[async_trait]
impl ProjectRepository for SqliteProjectRepo {
    async fn create(&self, project: &Project) -> CoreResult<()> {
        let mut tx = self.pool.begin().await.map_err(sqlx_err("begin tx"))?;

        sqlx::query(
            "INSERT INTO projects (id, name, description, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id_to_text(project.id.inner()))
        .bind(&project.name)
        .bind(project.description.as_deref())
        .bind(ts_to_millis(project.created_at))
        .bind(ts_to_millis(project.updated_at))
        .execute(&mut *tx)
        .await
        .map_err(sqlx_err("insert project"))?;

        for state in &project.states {
            sqlx::query(
                "INSERT INTO states (id, project_id, name, position, wip_limit)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(id_to_text(state.id.inner()))
            .bind(id_to_text(state.project_id.inner()))
            .bind(&state.name)
            .bind(i64::from(state.position))
            .bind(state.wip_limit.map(i64::from))
            .execute(&mut *tx)
            .await
            .map_err(sqlx_err("insert initial state"))?;
        }

        tx.commit().await.map_err(sqlx_err("commit tx"))?;
        Ok(())
    }

    async fn get(&self, id: ProjectId) -> CoreResult<Option<Project>> {
        let row = sqlx::query(
            "SELECT id, name, description, created_at, updated_at
             FROM projects WHERE id = ?",
        )
        .bind(id_to_text(id.inner()))
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_err("select project"))?;
        match row {
            None => Ok(None),
            Some(row) => {
                let states = load_states(&self.pool, id).await?;
                Ok(Some(row_to_project(&row, states)?))
            }
        }
    }

    async fn list(&self) -> CoreResult<Vec<Project>> {
        let rows = sqlx::query(
            "SELECT id, name, description, created_at, updated_at
             FROM projects ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_err("list projects"))?;
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            let id_text: String = row.try_get("id").map_err(sqlx_err("project.id"))?;
            let pid = ProjectId::new(id_from_text(&id_text)?);
            let states = load_states(&self.pool, pid).await?;
            out.push(row_to_project(row, states)?);
        }
        Ok(out)
    }

    async fn update(&self, project: &Project) -> CoreResult<()> {
        let affected = sqlx::query(
            "UPDATE projects SET name = ?, description = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&project.name)
        .bind(project.description.as_deref())
        .bind(ts_to_millis(project.updated_at))
        .bind(id_to_text(project.id.inner()))
        .execute(&self.pool)
        .await
        .map_err(sqlx_err("update project"))?
        .rows_affected();
        if affected == 0 {
            return Err(CoreError::NotFound {
                entity: EntityKind::Project,
                id: project.id.inner(),
            });
        }
        Ok(())
    }

    async fn delete(&self, id: ProjectId) -> CoreResult<()> {
        let affected = sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id_to_text(id.inner()))
            .execute(&self.pool)
            .await
            .map_err(sqlx_err("delete project"))?
            .rows_affected();
        if affected == 0 {
            return Err(CoreError::NotFound {
                entity: EntityKind::Project,
                id: id.inner(),
            });
        }
        Ok(())
    }

    async fn add_state(&self, state: &State) -> CoreResult<()> {
        let exists: Option<String> = sqlx::query_scalar("SELECT id FROM projects WHERE id = ?")
            .bind(id_to_text(state.project_id.inner()))
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlx_err("check project for add_state"))?;
        if exists.is_none() {
            return Err(CoreError::NotFound {
                entity: EntityKind::Project,
                id: state.project_id.inner(),
            });
        }

        sqlx::query(
            "INSERT INTO states (id, project_id, name, position, wip_limit)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id_to_text(state.id.inner()))
        .bind(id_to_text(state.project_id.inner()))
        .bind(&state.name)
        .bind(i64::from(state.position))
        .bind(state.wip_limit.map(i64::from))
        .execute(&self.pool)
        .await
        .map_err(sqlx_err("add state"))?;
        Ok(())
    }

    async fn update_state(&self, state: &State) -> CoreResult<()> {
        let affected = sqlx::query(
            "UPDATE states SET name = ?, position = ?, wip_limit = ?
             WHERE id = ?",
        )
        .bind(&state.name)
        .bind(i64::from(state.position))
        .bind(state.wip_limit.map(i64::from))
        .bind(id_to_text(state.id.inner()))
        .execute(&self.pool)
        .await
        .map_err(sqlx_err("update state"))?
        .rows_affected();
        if affected == 0 {
            return Err(CoreError::NotFound {
                entity: EntityKind::State,
                id: state.id.inner(),
            });
        }
        Ok(())
    }

    async fn remove_state(&self, id: StateId) -> CoreResult<()> {
        let affected = sqlx::query("DELETE FROM states WHERE id = ?")
            .bind(id_to_text(id.inner()))
            .execute(&self.pool)
            .await
            .map_err(sqlx_err("remove state"))?
            .rows_affected();
        if affected == 0 {
            return Err(CoreError::NotFound {
                entity: EntityKind::State,
                id: id.inner(),
            });
        }
        Ok(())
    }

    async fn reorder_states(&self, project_id: ProjectId, ordered: &[StateId]) -> CoreResult<()> {
        let mut tx = self.pool.begin().await.map_err(sqlx_err("begin tx"))?;

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM states WHERE project_id = ?")
            .bind(id_to_text(project_id.inner()))
            .fetch_one(&mut *tx)
            .await
            .map_err(sqlx_err("count states"))?;
        if usize::try_from(count).unwrap_or(usize::MAX) != ordered.len() {
            return Err(CoreError::validation("reorder length mismatch"));
        }

        let step = 1024i64;
        for (i, sid) in ordered.iter().enumerate() {
            let pos = step.saturating_mul(i64::try_from(i + 1).unwrap_or(i64::MAX));
            let affected = sqlx::query(
                "UPDATE states SET position = ?
                 WHERE id = ? AND project_id = ?",
            )
            .bind(pos)
            .bind(id_to_text(sid.inner()))
            .bind(id_to_text(project_id.inner()))
            .execute(&mut *tx)
            .await
            .map_err(sqlx_err("reorder state"))?
            .rows_affected();
            if affected == 0 {
                return Err(CoreError::NotFound {
                    entity: EntityKind::State,
                    id: sid.inner(),
                });
            }
        }

        tx.commit().await.map_err(sqlx_err("commit tx"))?;
        Ok(())
    }

    async fn get_state(&self, id: StateId) -> CoreResult<Option<State>> {
        let row = sqlx::query(
            "SELECT id, project_id, name, position, wip_limit
             FROM states WHERE id = ?",
        )
        .bind(id_to_text(id.inner()))
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_err("select state"))?;
        match row {
            Some(r) => Ok(Some(row_to_state(r)?)),
            None => Ok(None),
        }
    }
}
