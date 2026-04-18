mod fakes;

use fakes::{CountingIds, InMemoryTagRepo};
use kantui_core::{Color, CoreError, TagService};

#[tokio::test]
async fn create_and_list_tag() {
    let svc = TagService::new(InMemoryTagRepo::new(), CountingIds::new());
    let tag = svc.create("bug", Color::Red).await.unwrap();
    assert_eq!(tag.name, "bug");

    let listed = svc.list().await.unwrap();
    assert_eq!(listed.len(), 1);
}

#[tokio::test]
async fn duplicate_tag_name_conflicts() {
    let svc = TagService::new(InMemoryTagRepo::new(), CountingIds::new());
    svc.create("bug", Color::Red).await.unwrap();
    let err = svc.create("bug", Color::Blue).await.unwrap_err();
    assert!(matches!(err, CoreError::Conflict(_)));
}

#[tokio::test]
async fn blank_name_rejected() {
    let svc = TagService::new(InMemoryTagRepo::new(), CountingIds::new());
    let err = svc.create("   ", Color::Red).await.unwrap_err();
    assert!(matches!(err, CoreError::Validation(_)));
}
