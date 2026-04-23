mod fakes;

use std::time::Duration;

use fakes::{CountingIds, FakeClock, InMemoryProjectRepo, InMemoryTaskRepo};
use kantui_core::{
    CoreError, NewProject, NewState, NewTask, Project, ProjectService, StateId, StatsService,
    TaskService,
};

struct Ctx {
    projects: ProjectService<InMemoryProjectRepo, FakeClock, CountingIds>,
    tasks: TaskService<InMemoryProjectRepo, InMemoryTaskRepo, FakeClock, CountingIds>,
    stats: StatsService<InMemoryTaskRepo, FakeClock>,
    clock: FakeClock,
    project: Project,
    todo: StateId,
    doing: StateId,
    done: StateId,
}

async fn setup() -> Ctx {
    let projects_repo = InMemoryProjectRepo::new();
    let tasks_repo = InMemoryTaskRepo::new();
    let clock = FakeClock::new();
    let ids = CountingIds::new();

    let project_svc = ProjectService::new(projects_repo.clone(), clock.clone(), ids.clone());
    let task_svc = TaskService::new(
        projects_repo.clone(),
        tasks_repo.clone(),
        clock.clone(),
        ids.clone(),
    );
    let stats_svc = StatsService::new(tasks_repo.clone(), clock.clone());

    let project = project_svc
        .create(NewProject {
            name: "Board".into(),
            description: None,
            initial_states: vec!["Todo".into(), "Doing".into(), "Done".into()],
        })
        .await
        .unwrap();

    let todo = project.states[0].id;
    let doing = project.states[1].id;
    let done = project.states[2].id;

    Ctx {
        projects: project_svc,
        tasks: task_svc,
        stats: stats_svc,
        clock,
        project,
        todo,
        doing,
        done,
    }
}

#[tokio::test]
async fn create_task_then_list_by_state() {
    let ctx = setup().await;
    let task = ctx
        .tasks
        .create(NewTask::new(ctx.project.id, ctx.todo, "write plan"))
        .await
        .unwrap();
    assert_eq!(task.title, "write plan");

    let in_todo = ctx.tasks.list_in_state(ctx.todo).await.unwrap();
    assert_eq!(in_todo.len(), 1);
}

#[tokio::test]
async fn move_task_records_transition_and_updates_state() {
    let ctx = setup().await;
    let task = ctx
        .tasks
        .create(NewTask::new(ctx.project.id, ctx.todo, "write plan"))
        .await
        .unwrap();

    ctx.clock.advance(Duration::from_secs(60));
    ctx.tasks.move_task(task.id, ctx.doing, None).await.unwrap();

    let reloaded = ctx.tasks.get(task.id).await.unwrap();
    assert_eq!(reloaded.state_id.inner(), ctx.doing.inner());
    assert!(ctx.tasks.list_in_state(ctx.todo).await.unwrap().is_empty());
    assert_eq!(ctx.tasks.list_in_state(ctx.doing).await.unwrap().len(), 1);
}

#[tokio::test]
async fn wip_limit_blocks_move_into_full_state() {
    let ctx = setup().await;
    ctx.projects
        .set_wip_limit(ctx.doing, Some(1))
        .await
        .unwrap();

    let a = ctx
        .tasks
        .create(NewTask::new(ctx.project.id, ctx.todo, "A"))
        .await
        .unwrap();
    let b = ctx
        .tasks
        .create(NewTask::new(ctx.project.id, ctx.todo, "B"))
        .await
        .unwrap();

    ctx.tasks.move_task(a.id, ctx.doing, None).await.unwrap();
    let err = ctx
        .tasks
        .move_task(b.id, ctx.doing, None)
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::WipLimitExceeded { .. }));
}

#[tokio::test]
async fn cross_project_move_rejected() {
    let ctx = setup().await;
    let other = ctx
        .projects
        .create(NewProject {
            name: "Other".into(),
            description: None,
            initial_states: vec!["X".into()],
        })
        .await
        .unwrap();
    let task = ctx
        .tasks
        .create(NewTask::new(ctx.project.id, ctx.todo, "A"))
        .await
        .unwrap();

    let err = ctx
        .tasks
        .move_task(task.id, other.states[0].id, None)
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Validation(_)));
}

#[tokio::test]
async fn sojourn_accumulates_in_former_state() {
    let ctx = setup().await;
    let t = ctx
        .tasks
        .create(NewTask::new(ctx.project.id, ctx.todo, "A"))
        .await
        .unwrap();

    ctx.clock.advance(Duration::from_secs(100));
    ctx.tasks.move_task(t.id, ctx.doing, None).await.unwrap();
    ctx.clock.advance(Duration::from_secs(50));
    ctx.tasks.move_task(t.id, ctx.done, None).await.unwrap();
    ctx.clock.advance(Duration::from_secs(10));

    let sojourns = ctx.stats.project_sojourns(ctx.project.id).await.unwrap();
    let totals: std::collections::HashMap<[u8; 16], Duration> = sojourns
        .iter()
        .map(|s| (*s.state_id.inner().as_bytes(), s.total))
        .collect();

    assert_eq!(
        totals[ctx.todo.inner().as_bytes()],
        Duration::from_secs(100)
    );
    assert_eq!(
        totals[ctx.doing.inner().as_bytes()],
        Duration::from_secs(50)
    );
    assert_eq!(totals[ctx.done.inner().as_bytes()], Duration::from_secs(10));
}

#[tokio::test]
async fn throughput_buckets_completions_by_day() {
    const DAY: Duration = Duration::from_secs(24 * 60 * 60);

    let ctx = setup().await;
    let a = ctx
        .tasks
        .create(NewTask::new(ctx.project.id, ctx.todo, "A"))
        .await
        .unwrap();
    let b = ctx
        .tasks
        .create(NewTask::new(ctx.project.id, ctx.todo, "B"))
        .await
        .unwrap();
    let c = ctx
        .tasks
        .create(NewTask::new(ctx.project.id, ctx.todo, "C"))
        .await
        .unwrap();
    let d = ctx
        .tasks
        .create(NewTask::new(ctx.project.id, ctx.todo, "D"))
        .await
        .unwrap();

    ctx.clock.advance(DAY);
    ctx.tasks.move_task(a.id, ctx.done, None).await.unwrap();
    ctx.clock.advance(DAY);
    ctx.tasks.move_task(b.id, ctx.done, None).await.unwrap();
    ctx.clock.advance(DAY);
    ctx.tasks.move_task(c.id, ctx.done, None).await.unwrap();
    ctx.tasks.move_task(d.id, ctx.done, None).await.unwrap();

    let t = ctx
        .stats
        .throughput(ctx.project.id, ctx.done, 3)
        .await
        .unwrap();
    assert_eq!(t.total, 4);
    assert_eq!(t.per_day, vec![1, 1, 2]);
}

#[tokio::test]
async fn adding_after_anchor_preserves_order() {
    let ctx = setup().await;
    let _a = ctx
        .tasks
        .create(NewTask::new(ctx.project.id, ctx.todo, "A"))
        .await
        .unwrap();
    let b = ctx
        .tasks
        .create(NewTask::new(ctx.project.id, ctx.todo, "B"))
        .await
        .unwrap();
    let c = ctx
        .tasks
        .create(NewTask::new(ctx.project.id, ctx.todo, "C"))
        .await
        .unwrap();

    ctx.tasks
        .move_task(b.id, ctx.todo, Some(c.id))
        .await
        .unwrap();

    let order: Vec<_> = ctx
        .tasks
        .list_in_state(ctx.todo)
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(order, vec!["A", "C", "B"]);
    // Exercise unused service so rustc doesn't warn.
    let _ = ctx.projects.list().await.unwrap();
    let _ = NewState {
        name: String::new(),
        wip_limit: None,
    };
}
