//! One collection worker, independent of reader-window visibility.
use crate::{application::Application, semantic::SemanticProvider};
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundStatus {
    pub phase: String,
    pub reader_visible: bool,
    pub completed_turns: u64,
    pub last_issue: Option<String>,
}
struct State {
    stop: bool,
    stopped: bool,
    wake: bool,
    active: usize,
    reader_visible: bool,
    turns: u64,
    last_issue: Option<String>,
}
type StatusObserver = Arc<dyn Fn(BackgroundStatus) + Send + Sync>;
pub struct BackgroundControl {
    state: Mutex<State>,
    changed: Condvar,
    observer: Mutex<Option<StatusObserver>>,
}
pub struct WorkGuard(Arc<BackgroundControl>);
impl Drop for WorkGuard {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        state.active = state.active.saturating_sub(1);
        drop(state);
        self.0.publish();
    }
}
impl BackgroundControl {
    pub fn new(reader_visible: bool) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                stop: false,
                stopped: false,
                wake: true,
                active: 0,
                reader_visible,
                turns: 0,
                last_issue: None,
            }),
            changed: Condvar::new(),
            observer: Mutex::new(None),
        })
    }
    pub fn status(&self) -> BackgroundStatus {
        let s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        BackgroundStatus {
            phase: if s.stopped {
                "stopped"
            } else if s.stop {
                "stopping"
            } else if s.active > 0 {
                "working"
            } else {
                "waiting"
            }
            .into(),
            reader_visible: s.reader_visible,
            completed_turns: s.turns,
            last_issue: s.last_issue.clone(),
        }
    }
    pub fn observe(&self, observer: Arc<dyn Fn(BackgroundStatus) + Send + Sync>) {
        *self.observer.lock().unwrap_or_else(|e| e.into_inner()) = Some(observer);
        self.publish();
    }
    fn publish(&self) {
        let observer = self
            .observer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(observer) = observer {
            observer(self.status());
        }
    }
    pub fn enter(self: &Arc<Self>) -> WorkGuard {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).active += 1;
        self.publish();
        WorkGuard(Arc::clone(self))
    }
    pub fn reader_visible(&self, visible: bool) {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .reader_visible = visible;
        self.publish();
    }
    pub fn wake(&self) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).wake = true;
        self.changed.notify_all();
    }
    pub fn stop(&self) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).stop = true;
        self.changed.notify_all();
        self.publish();
    }
    pub fn stopping(&self) -> bool {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).stop
    }
    fn wait(&self, interval: Duration) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let (mut state, _) = self
            .changed
            .wait_timeout_while(state, interval, |s| !s.wake && !s.stop)
            .unwrap_or_else(|e| e.into_inner());
        if state.stop {
            return false;
        }
        state.wake = false;
        true
    }
    fn finished_turn(&self, issue: Option<String>, processed: bool) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.turns += 1;
        if issue.is_some() || processed {
            state.last_issue = issue;
        }
        drop(state);
        self.publish();
    }
}
/// Bounded maintenance uses the same public operations as foreground ingestion.
/// The system pauses this thread during sleep; wake resumes from durable job state.
pub fn spawn_worker(
    app: Arc<Mutex<Application>>,
    provider: Arc<dyn SemanticProvider>,
    control: Arc<BackgroundControl>,
    interval: Duration,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("knowledge-garden-maintenance".into())
        .spawn(move || {
            while control.wait(interval) {
                let work = control.enter();
                let mut issue = None;
                let mut processed = false;
                let mut run_local =
                    |operation: fn(&mut Application) -> crate::application::Result<()>| {
                        if control.stopping() {
                            return;
                        }
                        match app.lock() {
                            Ok(mut app) => {
                                if let Err(error) = operation(&mut app) {
                                    issue = Some(error.to_string());
                                }
                            }
                            Err(_) => {
                                issue = Some("The collection worker stopped unexpectedly.".into())
                            }
                        }
                    };
                run_local(|app| {
                    app.reconcile_collection(256)?;
                    Ok(())
                });
                run_local(|app| app.reconcile_watched_directories(8));
                run_local(|app| app.resume_due_url_acquisitions());
                run_local(|app| app.resume_due_audio_jobs());
                let jobs = if control.stopping() {
                    Vec::new()
                } else {
                    match app.lock() {
                        Ok(mut app) => match app.claim_due_semantic_jobs(2) {
                            Ok(jobs) => jobs,
                            Err(error) => {
                                issue = Some(error.to_string());
                                Vec::new()
                            }
                        },
                        Err(_) => {
                            issue =
                                Some("The semantic queue could not access the collection.".into());
                            Vec::new()
                        }
                    }
                };
                for job in jobs {
                    if control.stopping() {
                        break;
                    }
                    processed = true;
                    let result = provider.form_knowledge_with_prior(
                        &job.source_text,
                        job.prior_source_text.as_deref(),
                    );
                    if let Err(error) = &result {
                        issue = Some(error.to_string());
                    }
                    if control.stopping() {
                        break;
                    }
                    match app.lock() {
                        Ok(mut app) => {
                            if let Err(error) = app.finish_semantic_job(job, result) {
                                issue = Some(error.to_string());
                            }
                        }
                        Err(_) => {
                            issue =
                                Some("The semantic result could not access the collection.".into())
                        }
                    }
                }
                control.finished_turn(issue, processed);
                drop(work);
            }
            control
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .stopped = true;
            control.publish();
        })
}
