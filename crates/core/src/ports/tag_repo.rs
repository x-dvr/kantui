use async_trait::async_trait;

use crate::CoreResult;
use crate::domain::{Tag, TagId, TaskId};

#[async_trait]
pub trait TagRepository: Send + Sync {
    async fn create(&self, tag: &Tag) -> CoreResult<()>;
    async fn get(&self, id: TagId) -> CoreResult<Option<Tag>>;
    async fn find_by_name(&self, name: &str) -> CoreResult<Option<Tag>>;
    async fn list(&self) -> CoreResult<Vec<Tag>>;
    async fn update(&self, tag: &Tag) -> CoreResult<()>;
    async fn delete(&self, id: TagId) -> CoreResult<()>;

    async fn attach_to_task(&self, task_id: TaskId, tag_id: TagId) -> CoreResult<()>;
    async fn detach_from_task(&self, task_id: TaskId, tag_id: TagId) -> CoreResult<()>;
    async fn list_for_task(&self, task_id: TaskId) -> CoreResult<Vec<TagId>>;
}
