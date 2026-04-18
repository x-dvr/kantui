mod fakes;

use fakes::{CountingIds, FakeClock, InMemoryProjectRepo};
use kantui_core::{CoreError, NewProject, NewState, ProjectService};

fn service() -> ProjectService<InMemoryProjectRepo, FakeClock, CountingIds> {
    ProjectService::new(
        InMemoryProjectRepo::new(),
        FakeClock::new(),
        CountingIds::new(),
    )
}

#[tokio::test]
async fn create_project_with_initial_states() {
    let svc = service();
    let project = svc
        .create(NewProject {
            name: "Inbox".into(),
            description: None,
            initial_states: vec!["Todo".into(), "Doing".into(), "Done".into()],
        })
        .await
        .unwrap();

    assert_eq!(project.name, "Inbox");
    assert_eq!(project.states.len(), 3);
    assert_eq!(project.states[0].name, "Todo");
    assert_eq!(project.states[2].name, "Done");
    assert!(project.states[0].position < project.states[1].position);
}

#[tokio::test]
async fn blank_project_name_is_rejected() {
    let svc = service();
    let err = svc
        .create(NewProject {
            name: "   ".into(),
            description: None,
            initial_states: vec![],
        })
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Validation(_)));
}

#[tokio::test]
async fn add_and_rename_state() {
    let svc = service();
    let project = svc
        .create(NewProject {
            name: "Board".into(),
            description: None,
            initial_states: vec!["Todo".into()],
        })
        .await
        .unwrap();

    let state = svc
        .add_state(
            project.id,
            NewState {
                name: "Doing".into(),
                wip_limit: Some(3),
            },
        )
        .await
        .unwrap();
    assert_eq!(state.wip_limit, Some(3));

    let renamed = svc.rename_state(state.id, "In Progress").await.unwrap();
    assert_eq!(renamed.name, "In Progress");

    let reloaded = svc.get(project.id).await.unwrap();
    assert_eq!(reloaded.states.len(), 2);
}

#[tokio::test]
async fn reorder_states_requires_all_ids() {
    let svc = service();
    let project = svc
        .create(NewProject {
            name: "Board".into(),
            description: None,
            initial_states: vec!["A".into(), "B".into(), "C".into()],
        })
        .await
        .unwrap();

    // Partial list -> rejected.
    let err = svc
        .reorder_states(project.id, &[project.states[0].id, project.states[1].id])
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Validation(_)));

    // Full reversed list -> accepted.
    let reversed: Vec<_> = project.states.iter().rev().map(|s| s.id).collect();
    svc.reorder_states(project.id, &reversed).await.unwrap();

    let reloaded = svc.get(project.id).await.unwrap();
    assert_eq!(reloaded.states[0].name, "C");
    assert_eq!(reloaded.states[2].name, "A");
}
