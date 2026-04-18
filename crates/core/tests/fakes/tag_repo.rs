use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use kantui_core::{CoreError, CoreResult, EntityKind, Tag, TagId, TagRepository, TaskId};

fn eq16(a: &[u8; 16], b: &[u8; 16]) -> bool {
    a == b
}

struct Store {
    tags: Vec<Tag>,
    attached: Vec<(TaskId, TagId)>,
}

#[derive(Clone)]
pub struct InMemoryTagRepo {
    inner: Arc<Mutex<Store>>,
}

impl InMemoryTagRepo {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Store {
                tags: Vec::new(),
                attached: Vec::new(),
            })),
        }
    }
}

#[async_trait]
impl TagRepository for InMemoryTagRepo {
    async fn create(&self, tag: &Tag) -> CoreResult<()> {
        let mut store = self.inner.lock().unwrap();
        if store
            .tags
            .iter()
            .any(|t| eq16(t.id.inner().as_bytes(), tag.id.inner().as_bytes()))
        {
            return Err(CoreError::conflict("duplicate tag id"));
        }
        store.tags.push(tag.clone());
        Ok(())
    }

    async fn get(&self, id: TagId) -> CoreResult<Option<Tag>> {
        let store = self.inner.lock().unwrap();
        Ok(store
            .tags
            .iter()
            .find(|t| eq16(t.id.inner().as_bytes(), id.inner().as_bytes()))
            .cloned())
    }

    async fn find_by_name(&self, name: &str) -> CoreResult<Option<Tag>> {
        let store = self.inner.lock().unwrap();
        Ok(store.tags.iter().find(|t| t.name == name).cloned())
    }

    async fn list(&self) -> CoreResult<Vec<Tag>> {
        Ok(self.inner.lock().unwrap().tags.clone())
    }

    async fn update(&self, tag: &Tag) -> CoreResult<()> {
        let mut store = self.inner.lock().unwrap();
        let slot = store
            .tags
            .iter_mut()
            .find(|t| eq16(t.id.inner().as_bytes(), tag.id.inner().as_bytes()))
            .ok_or(CoreError::NotFound {
                entity: EntityKind::Tag,
                id: tag.id.inner(),
            })?;
        *slot = tag.clone();
        Ok(())
    }

    async fn delete(&self, id: TagId) -> CoreResult<()> {
        let mut store = self.inner.lock().unwrap();
        let before = store.tags.len();
        store
            .tags
            .retain(|t| !eq16(t.id.inner().as_bytes(), id.inner().as_bytes()));
        if store.tags.len() == before {
            return Err(CoreError::NotFound {
                entity: EntityKind::Tag,
                id: id.inner(),
            });
        }
        store
            .attached
            .retain(|(_, g)| !eq16(g.inner().as_bytes(), id.inner().as_bytes()));
        Ok(())
    }

    async fn attach_to_task(&self, task_id: TaskId, tag_id: TagId) -> CoreResult<()> {
        let mut store = self.inner.lock().unwrap();
        if !store
            .tags
            .iter()
            .any(|t| eq16(t.id.inner().as_bytes(), tag_id.inner().as_bytes()))
        {
            return Err(CoreError::NotFound {
                entity: EntityKind::Tag,
                id: tag_id.inner(),
            });
        }
        if !store.attached.iter().any(|(t, g)| {
            eq16(t.inner().as_bytes(), task_id.inner().as_bytes())
                && eq16(g.inner().as_bytes(), tag_id.inner().as_bytes())
        }) {
            store.attached.push((task_id, tag_id));
        }
        Ok(())
    }

    async fn detach_from_task(&self, task_id: TaskId, tag_id: TagId) -> CoreResult<()> {
        let mut store = self.inner.lock().unwrap();
        store.attached.retain(|(t, g)| {
            !(eq16(t.inner().as_bytes(), task_id.inner().as_bytes())
                && eq16(g.inner().as_bytes(), tag_id.inner().as_bytes()))
        });
        Ok(())
    }

    async fn list_for_task(&self, task_id: TaskId) -> CoreResult<Vec<TagId>> {
        let store = self.inner.lock().unwrap();
        Ok(store
            .attached
            .iter()
            .filter(|(t, _)| eq16(t.inner().as_bytes(), task_id.inner().as_bytes()))
            .map(|(_, g)| *g)
            .collect())
    }
}
