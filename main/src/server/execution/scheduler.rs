use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use tokio::sync::{Mutex, Notify, RwLock};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledExecutionKind {
    E2e,
    Load,
}

#[derive(Debug, Clone, Copy)]
pub struct SchedulerConfig {
    pub e2e_per_runner_limit: usize,
    pub load_per_runner_limit: usize,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            e2e_per_runner_limit: 1,
            load_per_runner_limit: 1,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SchedulerSnapshot {
    pub queued: usize,
    pub project_locks: HashMap<String, String>,
    pub load_locks: HashMap<String, String>,
    pub runner_usage: HashMap<String, RunnerUsage>,
}

#[derive(Debug, Clone, Default)]
pub struct RunnerUsage {
    pub e2e: usize,
    pub load: usize,
}

#[derive(Debug, Clone)]
struct QueueRequest {
    execution_id: String,
    kind: ScheduledExecutionKind,
    project_id: String,
    requested_nodes: usize,
    lock_key: Option<String>,
}

#[derive(Debug, Clone)]
struct ActiveReservation {
    kind: ScheduledExecutionKind,
    project_id: String,
    runners: Vec<String>,
    lock_key: Option<String>,
}

#[derive(Debug, Default)]
struct SchedulerState {
    queued: VecDeque<QueueRequest>,
    active: HashMap<String, ActiveReservation>,
    project_locks: HashMap<String, String>,
    load_locks: HashMap<String, String>,
    runner_usage: HashMap<String, RunnerUsage>,
}

#[derive(Debug, Clone)]
pub struct ExecutionScheduler {
    config: SchedulerConfig,
    state: Arc<Mutex<SchedulerState>>,
    notify: Arc<Notify>,
}

#[derive(Debug, Clone)]
pub enum AcquireOutcome {
    Reserved(Vec<String>),
    Pending { position: usize },
    Missing,
}

impl ExecutionScheduler {
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(SchedulerState::default())),
            notify: Arc::new(Notify::new()),
        }
    }

    pub async fn enqueue(
        &self,
        execution_id: String,
        kind: ScheduledExecutionKind,
        project_id: String,
        requested_nodes: usize,
    ) -> usize {
        self.enqueue_with_lock(execution_id, kind, project_id, requested_nodes, None)
            .await
    }

    pub async fn enqueue_with_lock(
        &self,
        execution_id: String,
        kind: ScheduledExecutionKind,
        project_id: String,
        requested_nodes: usize,
        lock_key: Option<String>,
    ) -> usize {
        let mut state = self.state.lock().await;
        if let Some((index, _)) = state
            .queued
            .iter()
            .enumerate()
            .find(|(_, item)| item.execution_id == execution_id)
        {
            return index + 1;
        }
        state.queued.push_back(QueueRequest {
            execution_id,
            kind,
            project_id,
            requested_nodes: requested_nodes.max(1),
            lock_key,
        });
        let position = state.queued.len();
        self.notify.notify_waiters();
        position
    }

    pub async fn try_acquire(
        &self,
        execution_id: &str,
        active_runners: &[String],
    ) -> AcquireOutcome {
        let mut state = self.state.lock().await;
        let Some(position) = state
            .queued
            .iter()
            .position(|item| item.execution_id == execution_id)
        else {
            return AcquireOutcome::Missing;
        };

        if position != 0 {
            return AcquireOutcome::Pending {
                position: position + 1,
            };
        }

        let request = state.queued.front().cloned().expect("front queue request");
        if request.kind == ScheduledExecutionKind::E2e
            && state
                .project_locks
                .get(&request.project_id)
                .is_some_and(|holder| holder != execution_id)
        {
            return AcquireOutcome::Pending { position: 1 };
        }
        if request
            .lock_key
            .as_ref()
            .and_then(|key| state.load_locks.get(key))
            .is_some_and(|holder| holder != execution_id)
        {
            return AcquireOutcome::Pending { position: 1 };
        }

        let mut selected = Vec::new();
        for runner in active_runners {
            let usage = state.runner_usage.get(runner).cloned().unwrap_or_default();
            let available = match request.kind {
                ScheduledExecutionKind::E2e => usage.e2e < self.config.e2e_per_runner_limit,
                ScheduledExecutionKind::Load => usage.load < self.config.load_per_runner_limit,
            };
            if available {
                selected.push(runner.clone());
            }
            if selected.len() >= request.requested_nodes {
                break;
            }
        }

        if selected.len() < request.requested_nodes {
            return AcquireOutcome::Pending { position: 1 };
        }

        state.queued.pop_front();
        for runner in &selected {
            let usage = state.runner_usage.entry(runner.clone()).or_default();
            match request.kind {
                ScheduledExecutionKind::E2e => usage.e2e += 1,
                ScheduledExecutionKind::Load => usage.load += 1,
            }
        }
        if request.kind == ScheduledExecutionKind::E2e {
            state
                .project_locks
                .insert(request.project_id.clone(), request.execution_id.clone());
        }
        if let Some(lock_key) = request.lock_key.clone() {
            state
                .load_locks
                .insert(lock_key, request.execution_id.clone());
        }
        state.active.insert(
            request.execution_id.clone(),
            ActiveReservation {
                kind: request.kind,
                project_id: request.project_id,
                runners: selected.clone(),
                lock_key: request.lock_key,
            },
        );
        AcquireOutcome::Reserved(selected)
    }

    pub async fn cancel_queued(&self, execution_id: &str) -> bool {
        let mut state = self.state.lock().await;
        let before = state.queued.len();
        state.queued.retain(|item| item.execution_id != execution_id);
        let changed = before != state.queued.len();
        if changed {
            self.notify.notify_waiters();
        }
        changed
    }

    pub async fn release(&self, execution_id: &str) {
        let mut state = self.state.lock().await;
        let Some(active) = state.active.remove(execution_id) else {
            self.notify.notify_waiters();
            return;
        };

        for runner in &active.runners {
            if let Some(usage) = state.runner_usage.get_mut(runner) {
                match active.kind {
                    ScheduledExecutionKind::E2e => usage.e2e = usage.e2e.saturating_sub(1),
                    ScheduledExecutionKind::Load => usage.load = usage.load.saturating_sub(1),
                }
                if usage.e2e == 0 && usage.load == 0 {
                    state.runner_usage.remove(runner);
                }
            }
        }

        if active.kind == ScheduledExecutionKind::E2e {
            state.project_locks.remove(&active.project_id);
        }
        if let Some(lock_key) = active.lock_key {
            state.load_locks.remove(&lock_key);
        }
        self.notify.notify_waiters();
    }

    #[allow(dead_code)]
    pub async fn queued_position(&self, execution_id: &str) -> Option<usize> {
        let state = self.state.lock().await;
        state
            .queued
            .iter()
            .position(|item| item.execution_id == execution_id)
            .map(|position| position + 1)
    }

    pub async fn wait_for_change(&self, cancel: &CancellationToken) -> bool {
        tokio::select! {
            _ = self.notify.notified() => true,
            _ = cancel.cancelled() => false,
        }
    }

    #[allow(dead_code)]
    pub async fn snapshot(&self) -> SchedulerSnapshot {
        let state = self.state.lock().await;
        SchedulerSnapshot {
            queued: state.queued.len(),
            project_locks: state.project_locks.clone(),
            load_locks: state.load_locks.clone(),
            runner_usage: state.runner_usage.clone(),
        }
    }
}

#[derive(Debug)]
pub struct SharedValue<T> {
    inner: Arc<RwLock<T>>,
}

impl<T> Clone for SharedValue<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> SharedValue<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(value)),
        }
    }

    pub async fn get(&self) -> T
    where
        T: Clone,
    {
        self.inner.read().await.clone()
    }

    pub async fn set(&self, value: T) {
        *self.inner.write().await = value;
    }
}

#[cfg(test)]
mod tests {
    use tokio_util::sync::CancellationToken;

    use super::{AcquireOutcome, ExecutionScheduler, ScheduledExecutionKind, SchedulerConfig};

    #[tokio::test]
    async fn enforces_project_lock_and_fifo_for_e2e() {
        let scheduler = ExecutionScheduler::new(SchedulerConfig::default());
        scheduler
            .enqueue(
                "exec-1".to_owned(),
                ScheduledExecutionKind::E2e,
                "project-1".to_owned(),
                1,
            )
            .await;
        scheduler
            .enqueue(
                "exec-2".to_owned(),
                ScheduledExecutionKind::E2e,
                "project-1".to_owned(),
                1,
            )
            .await;

        match scheduler.try_acquire("exec-1", &["runner-1".to_owned()]).await {
            AcquireOutcome::Reserved(runners) => assert_eq!(runners, vec!["runner-1".to_owned()]),
            other => panic!("unexpected acquire result: {other:?}"),
        }
        match scheduler.try_acquire("exec-2", &["runner-1".to_owned()]).await {
            AcquireOutcome::Pending { position } => assert_eq!(position, 1),
            other => panic!("unexpected acquire result: {other:?}"),
        }

        scheduler.release("exec-1").await;
        match scheduler.try_acquire("exec-2", &["runner-1".to_owned()]).await {
            AcquireOutcome::Reserved(runners) => assert_eq!(runners, vec!["runner-1".to_owned()]),
            other => panic!("unexpected acquire result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn reserves_multiple_runners_for_load() {
        let scheduler = ExecutionScheduler::new(SchedulerConfig {
            e2e_per_runner_limit: 1,
            load_per_runner_limit: 1,
        });
        scheduler
            .enqueue(
                "load-1".to_owned(),
                ScheduledExecutionKind::Load,
                "project-1".to_owned(),
                2,
            )
            .await;

        match scheduler
            .try_acquire(
                "load-1",
                &[
                    "runner-1".to_owned(),
                    "runner-2".to_owned(),
                    "runner-3".to_owned(),
                ],
            )
            .await
        {
            AcquireOutcome::Reserved(runners) => {
                assert_eq!(runners, vec!["runner-1".to_owned(), "runner-2".to_owned()])
            }
            other => panic!("unexpected acquire result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn blocks_parallel_load_for_same_pipeline_even_with_free_runner() {
        let scheduler = ExecutionScheduler::new(SchedulerConfig {
            e2e_per_runner_limit: 1,
            load_per_runner_limit: 1,
        });
        scheduler
            .enqueue_with_lock(
                "load-1".to_owned(),
                ScheduledExecutionKind::Load,
                "project-1".to_owned(),
                1,
                Some("project-1:pipeline-1".to_owned()),
            )
            .await;
        scheduler
            .enqueue_with_lock(
                "load-2".to_owned(),
                ScheduledExecutionKind::Load,
                "project-1".to_owned(),
                1,
                Some("project-1:pipeline-1".to_owned()),
            )
            .await;

        match scheduler
            .try_acquire("load-1", &["runner-1".to_owned(), "runner-2".to_owned()])
            .await
        {
            AcquireOutcome::Reserved(runners) => assert_eq!(runners, vec!["runner-1".to_owned()]),
            other => panic!("unexpected acquire result: {other:?}"),
        }

        match scheduler
            .try_acquire("load-2", &["runner-1".to_owned(), "runner-2".to_owned()])
            .await
        {
            AcquireOutcome::Pending { position } => assert_eq!(position, 1),
            other => panic!("unexpected acquire result: {other:?}"),
        }

        scheduler.release("load-1").await;
        match scheduler
            .try_acquire("load-2", &["runner-1".to_owned(), "runner-2".to_owned()])
            .await
        {
            AcquireOutcome::Reserved(runners) => assert_eq!(runners, vec!["runner-1".to_owned()]),
            other => panic!("unexpected acquire result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn removes_cancelled_request_from_queue() {
        let scheduler = ExecutionScheduler::new(SchedulerConfig::default());
        scheduler
            .enqueue(
                "exec-1".to_owned(),
                ScheduledExecutionKind::E2e,
                "project-1".to_owned(),
                1,
            )
            .await;
        assert!(scheduler.cancel_queued("exec-1").await);
        assert!(matches!(
            scheduler.try_acquire("exec-1", &["runner-1".to_owned()]).await,
            AcquireOutcome::Missing
        ));
    }

    #[tokio::test]
    async fn wait_for_change_unblocks_on_cancel() {
        let scheduler = ExecutionScheduler::new(SchedulerConfig::default());
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(!scheduler.wait_for_change(&cancel).await);
    }
}
