use async_trait::async_trait;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use kantui_core::{CoreError, CoreResult, EntityKind, Tag, TagId, TagRepository, TaskId};

use super::sqlx_err;
use crate::mapping::{color_from_text, color_to_text, id_from_text, id_to_text};

pub struct SqliteTagRepo {
    pool: SqlitePool,
}

impl SqliteTagRepo {
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn row_to_tag(row: SqliteRow) -> CoreResult<Tag> {
    let id: String = row.try_get("id").map_err(sqlx_err("tag.id"))?;
    let name: String = row.try_get("name").map_err(sqlx_err("tag.name"))?;
    let color: String = row.try_get("color").map_err(sqlx_err("tag.color"))?;
    Ok(Tag {
        id: TagId::new(id_from_text(&id)?),
        name,
        color: color_from_text(&color)?,
    })
}

#[async_trait]
impl TagRepository for SqliteTagRepo {
    async fn create(&self, tag: &Tag) -> CoreResult<()> {
        sqlx::query("INSERT INTO tags (id, name, color) VALUES (?, ?, ?)")
            .bind(id_to_text(tag.id.inner()))
            .bind(&tag.name)
            .bind(color_to_text(tag.color))
            .execute(&self.pool)
            .await
            .map_err(sqlx_err("insert tag"))?;
        Ok(())
    }

    async fn get(&self, id: TagId) -> CoreResult<Option<Tag>> {
        let row = sqlx::query("SELECT id, name, color FROM tags WHERE id = ?")
            .bind(id_to_text(id.inner()))
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlx_err("select tag"))?;
        match row {
            Some(r) => Ok(Some(row_to_tag(r)?)),
            None => Ok(None),
        }
    }

    async fn find_by_name(&self, name: &str) -> CoreResult<Option<Tag>> {
        let row = sqlx::query("SELECT id, name, color FROM tags WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlx_err("find tag by name"))?;
        match row {
            Some(r) => Ok(Some(row_to_tag(r)?)),
            None => Ok(None),
        }
    }

    async fn list(&self) -> CoreResult<Vec<Tag>> {
        let rows = sqlx::query("SELECT id, name, color FROM tags ORDER BY name ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(sqlx_err("list tags"))?;
        rows.into_iter().map(row_to_tag).collect()
    }

    async fn update(&self, tag: &Tag) -> CoreResult<()> {
        let affected = sqlx::query("UPDATE tags SET name = ?, color = ? WHERE id = ?")
            .bind(&tag.name)
            .bind(color_to_text(tag.color))
            .bind(id_to_text(tag.id.inner()))
            .execute(&self.pool)
            .await
            .map_err(sqlx_err("update tag"))?
            .rows_affected();
        if affected == 0 {
            return Err(CoreError::NotFound {
                entity: EntityKind::Tag,
                id: tag.id.inner(),
            });
        }
        Ok(())
    }

    async fn delete(&self, id: TagId) -> CoreResult<()> {
        let affected = sqlx::query("DELETE FROM tags WHERE id = ?")
            .bind(id_to_text(id.inner()))
            .execute(&self.pool)
            .await
            .map_err(sqlx_err("delete tag"))?
            .rows_affected();
        if affected == 0 {
            return Err(CoreError::NotFound {
                entity: EntityKind::Tag,
                id: id.inner(),
            });
        }
        Ok(())
    }

    async fn attach_to_task(&self, task_id: TaskId, tag_id: TagId) -> CoreResult<()> {
        let mut tx = self.pool.begin().await.map_err(sqlx_err("begin tx"))?;
        let exists: Option<String> = sqlx::query_scalar("SELECT id FROM tags WHERE id = ?")
            .bind(id_to_text(tag_id.inner()))
            .fetch_optional(&mut *tx)
            .await
            .map_err(sqlx_err("check tag exists"))?;
        if exists.is_none() {
            return Err(CoreError::NotFound {
                entity: EntityKind::Tag,
                id: tag_id.inner(),
            });
        }
        sqlx::query("INSERT OR IGNORE INTO task_tags (task_id, tag_id) VALUES (?, ?)")
            .bind(id_to_text(task_id.inner()))
            .bind(id_to_text(tag_id.inner()))
            .execute(&mut *tx)
            .await
            .map_err(sqlx_err("attach tag"))?;
        tx.commit().await.map_err(sqlx_err("commit tx"))?;
        Ok(())
    }

    async fn detach_from_task(&self, task_id: TaskId, tag_id: TagId) -> CoreResult<()> {
        sqlx::query("DELETE FROM task_tags WHERE task_id = ? AND tag_id = ?")
            .bind(id_to_text(task_id.inner()))
            .bind(id_to_text(tag_id.inner()))
            .execute(&self.pool)
            .await
            .map_err(sqlx_err("detach tag"))?;
        Ok(())
    }

    async fn list_for_task(&self, task_id: TaskId) -> CoreResult<Vec<TagId>> {
        let rows = sqlx::query("SELECT tag_id FROM task_tags WHERE task_id = ?")
            .bind(id_to_text(task_id.inner()))
            .fetch_all(&self.pool)
            .await
            .map_err(sqlx_err("list tags for task"))?;
        rows.into_iter()
            .map(|row| {
                let s: String = row
                    .try_get("tag_id")
                    .map_err(sqlx_err("task_tags.tag_id"))?;
                Ok(TagId::new(id_from_text(&s)?))
            })
            .collect()
    }
}
