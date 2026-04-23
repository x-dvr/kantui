use std::collections::BTreeMap;

use crate::domain::{
    Duration, ProjectId, StateId, StateSojourn, TaskId, TaskTransition, Throughput, Timestamp,
};
use crate::error::CoreResult;
use crate::ports::{Clock, TaskRepository};

const SECS_PER_DAY: u64 = 24 * 60 * 60;

pub struct StatsService<TR, C>
where
    TR: TaskRepository,
    C: Clock,
{
    tasks: TR,
    clock: C,
}

impl<TR, C> StatsService<TR, C>
where
    TR: TaskRepository,
    C: Clock,
{
    pub fn new(tasks: TR, clock: C) -> Self {
        Self { tasks, clock }
    }

    /// Aggregate sojourn time per state across all tasks in `project_id`.
    /// The *currently occupied* state for each task gets live credit
    /// (`now - last_transition.at`).
    pub async fn project_sojourns(&self, project_id: ProjectId) -> CoreResult<Vec<StateSojourn>> {
        let now = self.clock.now();
        let transitions = self.tasks.list_project_transitions(project_id).await?;
        let per_task = group_by_task(transitions);
        let totals = accumulate(per_task.into_values(), now);
        Ok(totals
            .into_iter()
            .map(|(state_id, (total, count))| StateSojourn {
                state_id: state_id.0,
                total,
                count,
            })
            .collect())
    }

    /// Tasks completed per day over the last `days`, where a completion is a
    /// transition whose `to_state` matches `done_state`. The returned
    /// `per_day` vector has exactly `days` entries; element 0 is the oldest
    /// day, the last entry is today.
    pub async fn throughput(
        &self,
        project_id: ProjectId,
        done_state: StateId,
        days: u32,
    ) -> CoreResult<Throughput> {
        let now = self.clock.now();
        let transitions = self.tasks.list_project_transitions(project_id).await?;
        let days_usize = days as usize;
        let mut per_day = vec![0u32; days_usize];
        let mut total = 0u32;
        let now_secs = now
            .to_system_time()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        let today_day = now_secs / SECS_PER_DAY;
        for t in transitions {
            if t.to_state != done_state {
                continue;
            }
            let at_secs = t
                .at
                .to_system_time()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs();
            let at_day = at_secs / SECS_PER_DAY;
            if at_day > today_day {
                continue;
            }
            let age = today_day - at_day;
            if (age as usize) < days_usize {
                let slot = days_usize - 1 - age as usize;
                per_day[slot] = per_day[slot].saturating_add(1);
                total = total.saturating_add(1);
            }
        }
        Ok(Throughput {
            done_state,
            total,
            per_day,
        })
    }

    /// Per-task breakdown: how long the task spent in each state.
    pub async fn task_history(&self, task_id: TaskId) -> CoreResult<Vec<(StateId, Duration)>> {
        let now = self.clock.now();
        let mut transitions = self.tasks.list_transitions(task_id).await?;
        transitions.sort_by_key(|t| t.at);
        let mut buckets: BTreeMap<BytesKey<StateId>, Duration> = BTreeMap::new();
        for pair in transitions.windows(2) {
            let stay = pair[1].at.saturating_since(pair[0].at);
            *buckets
                .entry(BytesKey(pair[0].to_state))
                .or_insert(Duration::ZERO) += stay;
        }
        if let Some(last) = transitions.last() {
            let live = now.saturating_since(last.at);
            *buckets
                .entry(BytesKey(last.to_state))
                .or_insert(Duration::ZERO) += live;
        }
        Ok(buckets.into_iter().map(|(k, v)| (k.0, v)).collect())
    }
}

fn group_by_task(
    transitions: Vec<TaskTransition>,
) -> BTreeMap<BytesKey<TaskId>, Vec<TaskTransition>> {
    let mut map: BTreeMap<BytesKey<TaskId>, Vec<TaskTransition>> = BTreeMap::new();
    for t in transitions {
        map.entry(BytesKey(t.task_id)).or_default().push(t);
    }
    for v in map.values_mut() {
        v.sort_by_key(|t| t.at);
    }
    map
}

fn accumulate<I>(per_task: I, now: Timestamp) -> BTreeMap<BytesKey<StateId>, (Duration, u32)>
where
    I: IntoIterator<Item = Vec<TaskTransition>>,
{
    let mut totals: BTreeMap<BytesKey<StateId>, (Duration, u32)> = BTreeMap::new();
    for transitions in per_task {
        for pair in transitions.windows(2) {
            let stay = pair[1].at.saturating_since(pair[0].at);
            let entry = totals
                .entry(BytesKey(pair[0].to_state))
                .or_insert((Duration::ZERO, 0));
            entry.0 += stay;
            entry.1 += 1;
        }
        if let Some(last) = transitions.last() {
            let live = now.saturating_since(last.at);
            let entry = totals
                .entry(BytesKey(last.to_state))
                .or_insert((Duration::ZERO, 0));
            entry.0 += live;
        }
    }
    totals
}

/// Wrapper so we can use typed IDs as BTreeMap keys without adding `Ord` to
/// the domain newtypes.
#[derive(Clone, Copy)]
struct BytesKey<T: HasBytes>(T);

impl<T: HasBytes> PartialEq for BytesKey<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.bytes() == other.0.bytes()
    }
}
impl<T: HasBytes> Eq for BytesKey<T> {}
impl<T: HasBytes> PartialOrd for BytesKey<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T: HasBytes> Ord for BytesKey<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.bytes().cmp(&other.0.bytes())
    }
}

trait HasBytes: Copy {
    fn bytes(&self) -> [u8; 16];
}
impl HasBytes for StateId {
    fn bytes(&self) -> [u8; 16] {
        *self.inner().as_bytes()
    }
}
impl HasBytes for TaskId {
    fn bytes(&self) -> [u8; 16] {
        *self.inner().as_bytes()
    }
}
