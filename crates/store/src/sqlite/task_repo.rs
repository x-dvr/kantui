use async_trait::async_trait;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use kantui_core::{
    CoreError, CoreResult, EntityKind, ProjectId, StateId, Task, TaskId, TaskRepository,
    TaskTransition, Timestamp,
};

use super::sqlx_err;
use crate::mapping::{
    complexity_from_text, complexity_to_text, id_from_text, id_to_text, priority_from_text,
    priority_to_text, ts_from_millis, ts_to_millis,
};

pub struct SqliteTaskRepo {
    pool: SqlitePool,
}

impl SqliteTaskRepo {
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn row_to_task(row: SqliteRow) -> CoreResult<Task> {
    let id: String = row.try_get("id").map_err(sqlx_err("task.id"))?;
    let project_id: String = row
        .try_get("project_id")
        .map_err(sqlx_err("task.project_id"))?;
    let state_id: String = row.try_get("state_id").map_err(sqlx_err("task.state_id"))?;
    let title: String = row.try_get("title").map_err(sqlx_err("task.title"))?;
    let description: Option<String> = row
        .try_get("description")
        .map_err(sqlx_err("task.description"))?;
    let priority: String = row.try_get("priority").map_err(sqlx_err("task.priority"))?;
    let complexity: String = row
        .try_get("complexity")
        .map_err(sqlx_err("task.complexity"))?;
    let due_date: Option<i64> = row.try_get("due_date").map_err(sqlx_err("task.due_date"))?;
    let position: i64 = row.try_get("position").map_err(sqlx_err("task.position"))?;
    let created_at: i64 = row
        .try_get("created_at")
        .map_err(sqlx_err("task.created_at"))?;
    let updated_at: i64 = row
        .try_get("updated_at")
        .map_err(sqlx_err("task.updated_at"))?;

    Ok(Task {
        id: TaskId::new(id_from_text(&id)?),
        project_id: ProjectId::new(id_from_text(&project_id)?),
        state_id: StateId::new(id_from_text(&state_id)?),
        title,
        description,
        priority: priority_from_text(&priority)?,
        complexity: complexity_from_text(&complexity)?,
        due_date: due_date.map(ts_from_millis),
        tags: Vec::new(),
        position: i32::try_from(position).unwrap_or(i32::MAX),
        created_at: ts_from_millis(created_at),
        updated_at: ts_from_millis(updated_at),
    })
}

fn row_to_transition(row: SqliteRow) -> CoreResult<TaskTransition> {
    let task_id: String = row
        .try_get("task_id")
        .map_err(sqlx_err("transition.task_id"))?;
    let from_state: Option<String> = row
        .try_get("from_state")
        .map_err(sqlx_err("transition.from_state"))?;
    let to_state: String = row
        .try_get("to_state")
        .map_err(sqlx_err("transition.to_state"))?;
    let at: i64 = row.try_get("at").map_err(sqlx_err("transition.at"))?;
    let from_state = match from_state {
        Some(s) => Some(StateId::new(id_from_text(&s)?)),
        None => None,
    };
    Ok(TaskTransition {
        task_id: TaskId::new(id_from_text(&task_id)?),
        from_state,
        to_state: StateId::new(id_from_text(&to_state)?),
        at: ts_from_millis(at),
    })
}

#[async_trait]
impl TaskRepository for SqliteTaskRepo {
    async fn create(&self, task: &Task, at: Timestamp) -> CoreResult<()> {
        let mut tx = self.pool.begin().await.map_err(sqlx_err("begin tx"))?;

        sqlx::query(
            "INSERT INTO tasks
                (id, project_id, state_id, title, description, priority, complexity,
                 due_date, position, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_text(task.id.inner()))
        .bind(id_to_text(task.project_id.inner()))
        .bind(id_to_text(task.state_id.inner()))
        .bind(&task.title)
        .bind(task.description.as_deref())
        .bind(priority_to_text(task.priority))
        .bind(complexity_to_text(task.complexity))
        .bind(task.due_date.map(ts_to_millis))
        .bind(i64::from(task.position))
        .bind(ts_to_millis(task.created_at))
        .bind(ts_to_millis(task.updated_at))
        .execute(&mut *tx)
        .await
        .map_err(sqlx_err("insert task"))?;

        sqlx::query(
            "INSERT INTO task_transitions (task_id, from_state, to_state, at)
             VALUES (?, NULL, ?, ?)",
        )
        .bind(id_to_text(task.id.inner()))
        .bind(id_to_text(task.state_id.inner()))
        .bind(ts_to_millis(at))
        .execute(&mut *tx)
        .await
        .map_err(sqlx_err("insert creation transition"))?;

        tx.commit().await.map_err(sqlx_err("commit tx"))?;
        Ok(())
    }

    async fn get(&self, id: TaskId) -> CoreResult<Option<Task>> {
        let row = sqlx::query(
            "SELECT id, project_id, state_id, title, description, priority, complexity,
                    due_date, position, created_at, updated_at
             FROM tasks WHERE id = ?",
        )
        .bind(id_to_text(id.inner()))
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_err("select task"))?;
        match row {
            Some(r) => Ok(Some(row_to_task(r)?)),
            None => Ok(None),
        }
    }

    async fn list_by_state(&self, state_id: StateId) -> CoreResult<Vec<Task>> {
        let rows = sqlx::query(
            "SELECT id, project_id, state_id, title, description, priority, complexity,
                    due_date, position, created_at, updated_at
             FROM tasks WHERE state_id = ? ORDER BY position ASC",
        )
        .bind(id_to_text(state_id.inner()))
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_err("list tasks by state"))?;
        rows.into_iter().map(row_to_task).collect()
    }

    async fn list_by_project(&self, project_id: ProjectId) -> CoreResult<Vec<Task>> {
        let rows = sqlx::query(
            "SELECT id, project_id, state_id, title, description, priority, complexity,
                    due_date, position, created_at, updated_at
             FROM tasks WHERE project_id = ? ORDER BY position ASC",
        )
        .bind(id_to_text(project_id.inner()))
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_err("list tasks by project"))?;
        rows.into_iter().map(row_to_task).collect()
    }

    async fn count_in_state(&self, state_id: StateId) -> CoreResult<u32> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE state_id = ?")
            .bind(id_to_text(state_id.inner()))
            .fetch_one(&self.pool)
            .await
            .map_err(sqlx_err("count tasks"))?;
        Ok(u32::try_from(count).unwrap_or(u32::MAX))
    }

    async fn update(&self, task: &Task) -> CoreResult<()> {
        let affected = sqlx::query(
            "UPDATE tasks SET title = ?, description = ?, priority = ?, complexity = ?,
                              due_date = ?, position = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&task.title)
        .bind(task.description.as_deref())
        .bind(priority_to_text(task.priority))
        .bind(complexity_to_text(task.complexity))
        .bind(task.due_date.map(ts_to_millis))
        .bind(i64::from(task.position))
        .bind(ts_to_millis(task.updated_at))
        .bind(id_to_text(task.id.inner()))
        .execute(&self.pool)
        .await
        .map_err(sqlx_err("update task"))?
        .rows_affected();
        if affected == 0 {
            return Err(CoreError::NotFound {
                entity: EntityKind::Task,
                id: task.id.inner(),
            });
        }
        Ok(())
    }

    async fn move_task(
        &self,
        task_id: TaskId,
        target_state: StateId,
        target_position: i32,
        at: Timestamp,
    ) -> CoreResult<()> {
        let mut tx = self.pool.begin().await.map_err(sqlx_err("begin tx"))?;

        let from_state_text: Option<String> =
            sqlx::query_scalar("SELECT state_id FROM tasks WHERE id = ?")
                .bind(id_to_text(task_id.inner()))
                .fetch_optional(&mut *tx)
                .await
                .map_err(sqlx_err("load current state"))?;
        let Some(from_state_text) = from_state_text else {
            return Err(CoreError::NotFound {
                entity: EntityKind::Task,
                id: task_id.inner(),
            });
        };

        sqlx::query(
            "UPDATE tasks SET state_id = ?, position = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(id_to_text(target_state.inner()))
        .bind(i64::from(target_position))
        .bind(ts_to_millis(at))
        .bind(id_to_text(task_id.inner()))
        .execute(&mut *tx)
        .await
        .map_err(sqlx_err("update task state"))?;

        sqlx::query(
            "INSERT INTO task_transitions (task_id, from_state, to_state, at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(id_to_text(task_id.inner()))
        .bind(from_state_text)
        .bind(id_to_text(target_state.inner()))
        .bind(ts_to_millis(at))
        .execute(&mut *tx)
        .await
        .map_err(sqlx_err("insert transition"))?;

        tx.commit().await.map_err(sqlx_err("commit tx"))?;
        Ok(())
    }

    async fn delete(&self, id: TaskId) -> CoreResult<()> {
        let affected = sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(id_to_text(id.inner()))
            .execute(&self.pool)
            .await
            .map_err(sqlx_err("delete task"))?
            .rows_affected();
        if affected == 0 {
            return Err(CoreError::NotFound {
                entity: EntityKind::Task,
                id: id.inner(),
            });
        }
        Ok(())
    }

    async fn list_transitions(&self, task_id: TaskId) -> CoreResult<Vec<TaskTransition>> {
        let rows = sqlx::query(
            "SELECT task_id, from_state, to_state, at
             FROM task_transitions
             WHERE task_id = ? ORDER BY at ASC",
        )
        .bind(id_to_text(task_id.inner()))
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_err("list transitions"))?;
        rows.into_iter().map(row_to_transition).collect()
    }

    async fn list_project_transitions(
        &self,
        project_id: ProjectId,
    ) -> CoreResult<Vec<TaskTransition>> {
        let rows = sqlx::query(
            "SELECT tt.task_id, tt.from_state, tt.to_state, tt.at
             FROM task_transitions tt
             JOIN tasks t ON t.id = tt.task_id
             WHERE t.project_id = ?
             ORDER BY tt.at ASC",
        )
        .bind(id_to_text(project_id.inner()))
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_err("list project transitions"))?;
        rows.into_iter().map(row_to_transition).collect()
    }
}
