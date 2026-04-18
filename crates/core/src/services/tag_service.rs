use crate::domain::{Color, EntityId, Tag, TagId, TaskId};
use crate::error::{CoreError, CoreResult, EntityKind};
use crate::ports::{IdGenerator, TagRepository};

pub struct TagService<R, G>
where
    R: TagRepository,
    G: IdGenerator,
{
    repo: R,
    ids: G,
}

impl<R, G> TagService<R, G>
where
    R: TagRepository,
    G: IdGenerator,
{
    pub fn new(repo: R, ids: G) -> Self {
        Self { repo, ids }
    }

    pub async fn create(&self, name: &str, color: Color) -> CoreResult<Tag> {
        let clean = validate_name(name)?;
        if self.repo.find_by_name(&clean).await?.is_some() {
            return Err(CoreError::conflict(format!("tag '{clean}' already exists")));
        }
        let tag = Tag {
            id: TagId::new(self.ids.new_id()),
            name: clean,
            color,
        };
        self.repo.create(&tag).await?;
        Ok(tag)
    }

    pub async fn list(&self) -> CoreResult<Vec<Tag>> {
        self.repo.list().await
    }

    pub async fn get(&self, id: TagId) -> CoreResult<Tag> {
        self.repo
            .get(id)
            .await?
            .ok_or_else(|| not_found(EntityKind::Tag, id.inner()))
    }

    pub async fn rename(&self, id: TagId, new_name: &str) -> CoreResult<Tag> {
        let clean = validate_name(new_name)?;
        if let Some(existing) = self.repo.find_by_name(&clean).await?
            && existing.id.inner() != id.inner()
        {
            return Err(CoreError::conflict(format!("tag '{clean}' already exists")));
        }
        let mut tag = self.get(id).await?;
        tag.name = clean;
        self.repo.update(&tag).await?;
        Ok(tag)
    }

    pub async fn set_color(&self, id: TagId, color: Color) -> CoreResult<Tag> {
        let mut tag = self.get(id).await?;
        tag.color = color;
        self.repo.update(&tag).await?;
        Ok(tag)
    }

    pub async fn delete(&self, id: TagId) -> CoreResult<()> {
        self.repo.delete(id).await
    }

    pub async fn attach(&self, task_id: TaskId, tag_id: TagId) -> CoreResult<()> {
        self.repo.attach_to_task(task_id, tag_id).await
    }

    pub async fn detach(&self, task_id: TaskId, tag_id: TagId) -> CoreResult<()> {
        self.repo.detach_from_task(task_id, tag_id).await
    }
}

fn validate_name(raw: &str) -> CoreResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        Err(CoreError::validation("tag name must not be empty"))
    } else {
        Ok(trimmed.to_owned())
    }
}

fn not_found(kind: EntityKind, id: EntityId) -> CoreError {
    CoreError::NotFound { entity: kind, id }
}
