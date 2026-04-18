#![cfg(feature = "sqlite")]

use std::time::{Duration, SystemTime};

use kantui_core::{
    Color, Complexity, CoreError, EntityId, Priority, Project, ProjectId, ProjectRepository, State,
    StateId, Tag, TagId, TagRepository, Task, TaskId, TaskRepository, Timestamp,
};
use kantui_store::sqlite::{SqliteProjectRepo, SqliteTagRepo, SqliteTaskRepo, connect_memory};

fn eid(n: u8) -> EntityId {
    let mut b = [0u8; 16];
    b[0] = n;
    EntityId::from_bytes(b)
}

fn base_ts() -> Timestamp {
    Timestamp::from_system_time(SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000))
}

fn ts_plus(secs: u64) -> Timestamp {
    Timestamp::from_system_time(SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + secs))
}

fn a_project(id: u8, states: Vec<(u8, &str, Option<u32>)>) -> Project {
    let pid = ProjectId::new(eid(id));
    let states = states
        .into_iter()
        .enumerate()
        .map(|(i, (sid, name, wip))| {
            let idx = i32::try_from(i + 1).unwrap_or(i32::MAX);
            State {
                id: StateId::new(eid(sid)),
                project_id: pid,
                name: name.to_owned(),
                position: 1024 * idx,
                wip_limit: wip,
            }
        })
        .collect();
    Project {
        id: pid,
        name: format!("Project {id}"),
        description: None,
        states,
        created_at: base_ts(),
        updated_at: base_ts(),
    }
}

fn a_task(tid: u8, project: ProjectId, state: StateId, title: &str, position: i32) -> Task {
    Task {
        id: TaskId::new(eid(tid)),
        project_id: project,
        state_id: state,
        title: title.to_owned(),
        description: None,
        priority: Priority::Normal,
        complexity: Complexity::Light,
        due_date: None,
        tags: Vec::new(),
        position,
        created_at: base_ts(),
        updated_at: base_ts(),
    }
}

#[tokio::test]
async fn projects_crud_roundtrip() {
    let pool = connect_memory().await.unwrap();
    let repo = SqliteProjectRepo::new(pool);

    let project = a_project(1, vec![(10, "Todo", None), (11, "Doing", Some(3))]);
    repo.create(&project).await.unwrap();

    let got = repo.get(project.id).await.unwrap().unwrap();
    assert_eq!(got.name, "Project 1");
    assert_eq!(got.states.len(), 2);
    assert_eq!(got.states[0].name, "Todo");
    assert_eq!(got.states[1].wip_limit, Some(3));
    assert!(got.states[0].position < got.states[1].position);

    assert_eq!(repo.list().await.unwrap().len(), 1);

    repo.delete(project.id).await.unwrap();
    assert!(repo.get(project.id).await.unwrap().is_none());
    assert!(matches!(
        repo.delete(project.id).await,
        Err(CoreError::NotFound { .. })
    ));
}

#[tokio::test]
async fn reorder_states_requires_full_list() {
    let pool = connect_memory().await.unwrap();
    let repo = SqliteProjectRepo::new(pool);

    let p = a_project(1, vec![(10, "A", None), (11, "B", None), (12, "C", None)]);
    repo.create(&p).await.unwrap();

    let err = repo
        .reorder_states(p.id, &[p.states[0].id, p.states[1].id])
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Validation(_)));

    let reversed: Vec<_> = p.states.iter().rev().map(|s| s.id).collect();
    repo.reorder_states(p.id, &reversed).await.unwrap();

    let got = repo.get(p.id).await.unwrap().unwrap();
    assert_eq!(got.states[0].name, "C");
    assert_eq!(got.states[2].name, "A");
}

#[tokio::test]
async fn add_update_remove_state() {
    let pool = connect_memory().await.unwrap();
    let repo = SqliteProjectRepo::new(pool);

    let p = a_project(1, vec![(10, "Todo", None)]);
    repo.create(&p).await.unwrap();

    let new_state = State {
        id: StateId::new(eid(11)),
        project_id: p.id,
        name: "Doing".into(),
        position: 2048,
        wip_limit: Some(2),
    };
    repo.add_state(&new_state).await.unwrap();

    let reloaded = repo.get(p.id).await.unwrap().unwrap();
    assert_eq!(reloaded.states.len(), 2);
    assert_eq!(reloaded.states[1].name, "Doing");

    let mut renamed = new_state.clone();
    renamed.name = "In Progress".into();
    renamed.wip_limit = Some(5);
    repo.update_state(&renamed).await.unwrap();

    let fetched = repo.get_state(renamed.id).await.unwrap().unwrap();
    assert_eq!(fetched.name, "In Progress");
    assert_eq!(fetched.wip_limit, Some(5));

    repo.remove_state(renamed.id).await.unwrap();
    assert!(repo.get_state(renamed.id).await.unwrap().is_none());
}

#[tokio::test]
async fn add_state_rejects_unknown_project() {
    let pool = connect_memory().await.unwrap();
    let repo = SqliteProjectRepo::new(pool);

    let orphan = State {
        id: StateId::new(eid(50)),
        project_id: ProjectId::new(eid(99)),
        name: "Todo".into(),
        position: 1024,
        wip_limit: None,
    };
    assert!(matches!(
        repo.add_state(&orphan).await,
        Err(CoreError::NotFound { .. })
    ));
}

#[tokio::test]
async fn tasks_roundtrip_and_listings() {
    let pool = connect_memory().await.unwrap();
    let pr = SqliteProjectRepo::new(pool.clone());
    let tr = SqliteTaskRepo::new(pool);

    let p = a_project(1, vec![(10, "Todo", None), (11, "Doing", None)]);
    pr.create(&p).await.unwrap();

    tr.create(&a_task(20, p.id, p.states[0].id, "plan", 1024), base_ts())
        .await
        .unwrap();
    tr.create(&a_task(21, p.id, p.states[0].id, "write", 2048), base_ts())
        .await
        .unwrap();
    tr.create(&a_task(22, p.id, p.states[1].id, "review", 1024), base_ts())
        .await
        .unwrap();

    let in_todo = tr.list_by_state(p.states[0].id).await.unwrap();
    assert_eq!(in_todo.len(), 2);
    assert_eq!(in_todo[0].title, "plan");
    assert_eq!(in_todo[1].title, "write");

    assert_eq!(tr.count_in_state(p.states[0].id).await.unwrap(), 2);
    assert_eq!(tr.list_by_project(p.id).await.unwrap().len(), 3);

    let got = tr.get(TaskId::new(eid(20))).await.unwrap().unwrap();
    assert_eq!(got.title, "plan");
    assert_eq!(got.priority, Priority::Normal);
    assert_eq!(got.complexity, Complexity::Light);
    assert!(got.description.is_none());
    assert!(got.due_date.is_none());
    assert!(got.tags.is_empty());
}

#[tokio::test]
async fn move_task_is_transactional_and_logs_transition() {
    let pool = connect_memory().await.unwrap();
    let pr = SqliteProjectRepo::new(pool.clone());
    let tr = SqliteTaskRepo::new(pool);

    let p = a_project(1, vec![(10, "Todo", None), (11, "Doing", None)]);
    pr.create(&p).await.unwrap();

    let task = a_task(20, p.id, p.states[0].id, "work", 1024);
    tr.create(&task, base_ts()).await.unwrap();

    tr.move_task(task.id, p.states[1].id, 1024, ts_plus(30))
        .await
        .unwrap();

    let reloaded = tr.get(task.id).await.unwrap().unwrap();
    assert_eq!(
        reloaded.state_id.inner().as_bytes(),
        p.states[1].id.inner().as_bytes()
    );
    assert!(tr.list_by_state(p.states[0].id).await.unwrap().is_empty());
    assert_eq!(tr.list_by_state(p.states[1].id).await.unwrap().len(), 1);

    let task_log = tr.list_transitions(task.id).await.unwrap();
    assert_eq!(task_log.len(), 2);
    assert!(task_log[0].from_state.is_none());
    assert_eq!(
        task_log[0].to_state.inner().as_bytes(),
        p.states[0].id.inner().as_bytes()
    );
    assert_eq!(
        task_log[1].from_state.unwrap().inner().as_bytes(),
        p.states[0].id.inner().as_bytes()
    );
    assert_eq!(
        task_log[1].to_state.inner().as_bytes(),
        p.states[1].id.inner().as_bytes()
    );

    let project_log = tr.list_project_transitions(p.id).await.unwrap();
    assert_eq!(project_log.len(), 2);
}

#[tokio::test]
async fn move_missing_task_is_not_found_and_no_transition_written() {
    let pool = connect_memory().await.unwrap();
    let pr = SqliteProjectRepo::new(pool.clone());
    let tr = SqliteTaskRepo::new(pool);

    let p = a_project(1, vec![(10, "Todo", None)]);
    pr.create(&p).await.unwrap();

    let ghost = TaskId::new(eid(99));
    let err = tr
        .move_task(ghost, p.states[0].id, 1024, base_ts())
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { .. }));

    assert!(tr.list_transitions(ghost).await.unwrap().is_empty());
}

#[tokio::test]
async fn delete_task_cascades_transitions() {
    let pool = connect_memory().await.unwrap();
    let pr = SqliteProjectRepo::new(pool.clone());
    let tr = SqliteTaskRepo::new(pool);

    let p = a_project(1, vec![(10, "Todo", None), (11, "Doing", None)]);
    pr.create(&p).await.unwrap();

    let t = a_task(20, p.id, p.states[0].id, "x", 1024);
    tr.create(&t, base_ts()).await.unwrap();
    tr.move_task(t.id, p.states[1].id, 1024, ts_plus(10))
        .await
        .unwrap();

    tr.delete(t.id).await.unwrap();
    assert!(tr.list_transitions(t.id).await.unwrap().is_empty());
    assert!(matches!(
        tr.delete(t.id).await,
        Err(CoreError::NotFound { .. })
    ));
}

#[tokio::test]
async fn task_update_roundtrip_preserves_enums() {
    let pool = connect_memory().await.unwrap();
    let pr = SqliteProjectRepo::new(pool.clone());
    let tr = SqliteTaskRepo::new(pool);

    let p = a_project(1, vec![(10, "Todo", None)]);
    pr.create(&p).await.unwrap();

    let mut t = a_task(20, p.id, p.states[0].id, "x", 1024);
    tr.create(&t, base_ts()).await.unwrap();

    t.priority = Priority::Critical;
    t.complexity = Complexity::Deep;
    t.description = Some("details".into());
    t.due_date = Some(ts_plus(3600));
    t.updated_at = ts_plus(10);
    tr.update(&t).await.unwrap();

    let got = tr.get(t.id).await.unwrap().unwrap();
    assert_eq!(got.priority, Priority::Critical);
    assert_eq!(got.complexity, Complexity::Deep);
    assert_eq!(got.description.as_deref(), Some("details"));
    assert_eq!(got.due_date, Some(ts_plus(3600)));
}

#[tokio::test]
async fn tags_crud_and_attachment() {
    let pool = connect_memory().await.unwrap();
    let pr = SqliteProjectRepo::new(pool.clone());
    let tr = SqliteTaskRepo::new(pool.clone());
    let gr = SqliteTagRepo::new(pool);

    let p = a_project(1, vec![(10, "Todo", None)]);
    pr.create(&p).await.unwrap();
    let task = a_task(20, p.id, p.states[0].id, "x", 1024);
    tr.create(&task, base_ts()).await.unwrap();

    let tag = Tag {
        id: TagId::new(eid(40)),
        name: "bug".into(),
        color: Color::Custom([0xab, 0xcd, 0xef]),
    };
    gr.create(&tag).await.unwrap();

    let got = gr.get(tag.id).await.unwrap().unwrap();
    assert_eq!(got.name, "bug");
    assert_eq!(got.color, Color::Custom([0xab, 0xcd, 0xef]));

    assert!(gr.find_by_name("bug").await.unwrap().is_some());
    assert!(gr.find_by_name("nope").await.unwrap().is_none());

    gr.attach_to_task(task.id, tag.id).await.unwrap();
    gr.attach_to_task(task.id, tag.id).await.unwrap(); // idempotent
    let ids = gr.list_for_task(task.id).await.unwrap();
    assert_eq!(ids.len(), 1);

    gr.detach_from_task(task.id, tag.id).await.unwrap();
    assert!(gr.list_for_task(task.id).await.unwrap().is_empty());
    gr.detach_from_task(task.id, tag.id).await.unwrap(); // idempotent

    let missing = TagId::new(eid(99));
    let err = gr.attach_to_task(task.id, missing).await.unwrap_err();
    assert!(matches!(err, CoreError::NotFound { .. }));
}

#[tokio::test]
async fn deleting_project_cascades_states_tasks_and_transitions() {
    let pool = connect_memory().await.unwrap();
    let pr = SqliteProjectRepo::new(pool.clone());
    let tr = SqliteTaskRepo::new(pool);

    let p = a_project(1, vec![(10, "Todo", None), (11, "Doing", None)]);
    pr.create(&p).await.unwrap();

    let t = a_task(20, p.id, p.states[0].id, "x", 1024);
    tr.create(&t, base_ts()).await.unwrap();
    tr.move_task(t.id, p.states[1].id, 1024, ts_plus(5))
        .await
        .unwrap();

    pr.delete(p.id).await.unwrap();

    assert!(tr.get(t.id).await.unwrap().is_none());
    assert!(tr.list_transitions(t.id).await.unwrap().is_empty());
    assert!(tr.list_by_project(p.id).await.unwrap().is_empty());
}
