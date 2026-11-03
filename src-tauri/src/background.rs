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
    pub recovery_detail: Option<String>,
}
struct State {
    stop: bool,
    stopped: bool,
    wake: bool,
    active: usize,
    reader_visible: bool,
    startup_pending: bool,
    recovery_detail: Option<String>,
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
                startup_pending: false,
                recovery_detail: None,
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
            } else if s.startup_pending {
                "recovering"
            } else if s.active > 0 {
                "working"
            } else {
                "waiting"
            }
            .into(),
            reader_visible: s.reader_visible,
            completed_turns: s.turns,
            last_issue: s.last_issue.clone(),
            recovery_detail: s.recovery_detail.clone(),
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
    fn recovery_progress(&self, detail: &str) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let changed =
            state.recovery_detail.as_deref() != Some(detail) || state.last_issue.is_some();
        state.recovery_detail = Some(detail.into());
        state.last_issue = None;
        drop(state);
        if changed {
            self.publish();
        }
    }
    pub fn startup_pending(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .startup_pending
    }
    pub fn set_startup_pending(&self, pending: bool) {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .startup_pending = pending;
        self.publish();
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
            let mut startup_root = None;
            let mut startup_session =
                crate::application::migration_backup::startup_coordinator::Session::new();
            let mut startup_coordinated = false;
            let mut legacy_initialized = false;
            let mut legacy_complete = false;
            let mut legacy_adoption = None;
            let mut precutover_initialized = false;
            let mut precutover_complete = false;
            let mut precutover = None;
            let mut initial_checked = false;
            let mut preverified_enabled = false;
            let mut initial_verifier = None;
            let mut final_verifier = None;
            while control.wait(interval) {
                if control.startup_pending() && !startup_coordinated {
                    if startup_root.is_none() {
                        let root = match app.lock() {
                            Ok(app) => app.startup_collection_root(),
                            Err(_) => {
                                control.finished_turn(
                                    Some("The collection worker stopped unexpectedly.".into()),
                                    false,
                                );
                                continue;
                            }
                        };
                        if let Err(error) =
                            crate::application::migration_backup::startup_coordinator::begin(&root)
                        {
                            control.finished_turn(Some(error.to_string()), false);
                            continue;
                        }
                        startup_root = Some(root);
                    }
                    let root = startup_root.as_ref().unwrap();
                    if let Ok(detail) =
                        crate::application::migration_backup::startup_coordinator::description(root, &startup_session)
                    {
                        control.recovery_progress(&detail);
                    }
                    match crate::application::migration_backup::startup_coordinator::step(
                        root,
                        &mut startup_session,
                    ) {
                        Ok(true) => startup_coordinated = true,
                        Ok(false) => {
                            control.wake();
                            continue;
                        }
                        Err(error) => {
                            control.finished_turn(Some(error.to_string()), false);
                            continue;
                        }
                    }
                }
                if control.startup_pending() && startup_coordinated && !legacy_complete {
                    let root = startup_root.as_ref().unwrap();
                    if !legacy_initialized {
                        match crate::application::migration_backup::legacy_adopt::Session::new(root) {
                            Ok(Some(session)) => legacy_adoption = Some(session),
                            Ok(None) => legacy_complete = true,
                            Err(error) => { control.finished_turn(Some(error.to_string()), false); continue; }
                        }
                        legacy_initialized = true;
                    }
                    if let Some(session) = legacy_adoption.as_mut() {
                        control.recovery_progress("Verifying and preparing the previous migration backup.");
                        match session.step() {
                            Ok(true) => { legacy_complete = true; legacy_adoption = None; },
                            Ok(false) => { control.wake(); continue; },
                            Err(error) => { control.finished_turn(Some(error.to_string()), false); continue; }
                        }
                    }
                }
                if control.startup_pending() && startup_coordinated && !precutover_complete {
                    let root = startup_root.as_ref().unwrap();
                    if !precutover_initialized {
                        match crate::application::migration_backup::precutover::Session::new(root) {
                            Ok(Some(session)) => precutover = Some(session),
                            Ok(None) => precutover_complete = true,
                            Err(error) => {
                                control.finished_turn(Some(error.to_string()), false);
                                continue;
                            }
                        }
                        precutover_initialized = true;
                    }
                    if let Some(session) = precutover.as_mut() {
                        control.recovery_progress(
                            "Checking and preserving historical owner statements.",
                        );
                        let action = match session.step(root) {
                            Ok(action) => action,
                            Err(error) => {
                                control.finished_turn(Some(error.to_string()), false);
                                continue;
                            }
                        };
                        use crate::application::migration_backup::precutover::Action;
                        match action {
                            Action::More => {
                                control.wake();
                                continue;
                            }
                            Action::Import(id) => {
                                let result = match app.lock() {
                                    Ok(mut app) => session.import(&mut app, &id),
                                    Err(_) => {
                                        control.finished_turn(
                                            Some(
                                                "The collection worker stopped unexpectedly."
                                                    .into(),
                                            ),
                                            false,
                                        );
                                        continue;
                                    }
                                };
                                if let Err(error) = result {
                                    control.finished_turn(Some(error.to_string()), false);
                                    continue;
                                }
                                control.wake();
                                continue;
                            }
                            Action::Receipts => {
                                let result = match app.lock() {
                                    Ok(mut app) => session.verify_receipt(&mut app),
                                    Err(_) => {
                                        control.finished_turn(
                                            Some(
                                                "The collection worker stopped unexpectedly."
                                                    .into(),
                                            ),
                                            false,
                                        );
                                        continue;
                                    }
                                };
                                if let Err(error) = result {
                                    control.finished_turn(Some(error.to_string()), false);
                                    continue;
                                }
                                control.wake();
                                continue;
                            }
                            action @ (Action::VerifyPage(_)
                            | Action::VerifyImport(_)
                            | Action::Retire(_)
                            | Action::Trust(_)
                            | Action::TrustImport(_)) => {
                                let result = match app.lock() {
                                    Ok(mut app) => session.retirement_action(&mut app, action),
                                    Err(_) => {
                                        control.finished_turn(
                                            Some(
                                                "The collection worker stopped unexpectedly."
                                                    .into(),
                                            ),
                                            false,
                                        );
                                        continue;
                                    }
                                };
                                if let Err(error) = result {
                                    control.finished_turn(Some(error.to_string()), false);
                                    continue;
                                }
                                control.wake();
                                continue;
                            }
                            Action::Complete => {
                                precutover_complete = true;
                                precutover = None;
                            }
                        }
                    }
                }
                if control.startup_pending() && startup_coordinated && precutover_complete && !initial_checked {
                    let root = startup_root.as_ref().unwrap();
                    if initial_verifier.is_none() {
                        match crate::application::migration_backup::final_verify::eligible(root) {
                            Ok(false) => {
                                control.finished_turn(Some("Migration authority is not ready for bounded startup verification.".into()), false);
                                continue;
                            },
                            Ok(true) => match crate::application::migration_backup::final_verify::Session::new(root, true) {
                                Ok(session) => { initial_verifier = Some(session); preverified_enabled = true; },
                                Err(error) => { control.finished_turn(Some(error.to_string()), false); continue; }
                            },
                            Err(error) => { control.finished_turn(Some(error.to_string()), false); continue; }
                        }
                    }
                    if let Some(session) = initial_verifier.as_mut() {
                        control.recovery_progress("Verifying recovery backups before opening the library.");
                        match session.step() {
                            Ok(Some(proof)) => {
                                let result = match app.lock() {
                                    Ok(mut app) => app.accept_verified_startup(&proof),
                                    Err(_) => { control.finished_turn(Some("The collection worker stopped unexpectedly.".into()), false); continue; }
                                };
                                if let Err(error) = result { control.finished_turn(Some(error.to_string()), false); continue; }
                                initial_checked = true; initial_verifier = None;
                            }
                            Ok(None) => { control.wake(); continue; }
                            Err(error) => { control.finished_turn(Some(error.to_string()), false); continue; }
                        }
                    }
                }
                if control.startup_pending() && startup_coordinated && precutover_complete && initial_checked {
                    let ready = match app.lock() {
                        Ok(app) => app.startup_ready_for_finish(),
                        Err(_) => { control.finished_turn(Some("The collection worker stopped unexpectedly.".into()), false); continue; }
                    };
                    if ready && preverified_enabled {
                        let root = startup_root.as_ref().unwrap();
                        if final_verifier.is_none() {
                            match crate::application::migration_backup::final_verify::Session::new(root, false) {
                                Ok(session) => final_verifier = Some(session),
                                Err(error) => { control.finished_turn(Some(error.to_string()), false); continue; }
                            }
                        }
                        control.recovery_progress("Checking the recovery backup before completing startup.");
                        match final_verifier.as_mut().unwrap().step() {
                            Ok(Some(proof)) => {
                                let result = match app.lock() {
                                    Ok(mut app) => app.finish_verified_startup(&proof),
                                    Err(_) => { control.finished_turn(Some("The collection worker stopped unexpectedly.".into()), false); continue; }
                                };
                                if let Err(error) = result { control.finished_turn(Some(error.to_string()), false); continue; }
                                control.set_startup_pending(false);
                            }
                            Ok(None) => { control.wake(); continue; }
                            Err(error) => { control.finished_turn(Some(error.to_string()), false); continue; }
                        }
                    } else {
                        control.recovery_progress("Preparing library indexes and recovering queued work.");
                        let outcome = match app.lock() {
                            Ok(mut app) => app.resume_pending_startup_step(),
                            Err(_) => { control.finished_turn(Some("The collection worker stopped unexpectedly.".into()), false); continue; }
                        };
                        match outcome {
                            Ok(true) => control.set_startup_pending(false),
                            Ok(false) => { control.wake(); continue; }
                            Err(error) => { control.finished_turn(Some(error.to_string()), false); continue; }
                        }
                    }
                }
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
                                let startup = app.resume_pending_startup();
                                if startup.is_ok() {
                                    control.set_startup_pending(false);
                                }
                                if let Err(error) = startup
                                    .and_then(|_| app.recover_integrity_exclusions())
                                    .and_then(|_| operation(&mut app))
                                {
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
                let visual = if control.stopping() {
                    None
                } else {
                    match app.lock() {
                        Ok(mut app) => match app.claim_due_visual_job() {
                            Ok(job) => job,
                            Err(error) => {
                                issue = Some(error.to_string());
                                None
                            }
                        },
                        Err(_) => {
                            issue =
                                Some("The visual queue could not access the collection.".into());
                            None
                        }
                    }
                };
                if let Some(visual) = visual {
                    let result = visual.run();
                    match app.lock() {
                        Ok(mut app) => {
                            if let Err(error) = app.finish_visual_job(visual, result) {
                                issue = Some(error.to_string());
                            }
                        }
                        Err(_) => {
                            issue =
                                Some("The visual result could not access the collection.".into())
                        }
                    }
                }
                let jobs = if control.stopping() {
                    Vec::new()
                } else {
                    match app.lock() {
                        Ok(mut app) => match app
                            .resume_pending_startup()
                            .and_then(|_| app.recover_integrity_exclusions())
                            .and_then(|_| app.claim_due_semantic_jobs(2))
                        {
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
                    let result = if job.needs_provider() { provider.form_knowledge_for_source(&job.source_text, job.prior_source_text.as_deref(), &job.source_format) } else { Ok(crate::semantic::KnowledgeDraft::default()) };
                    if let Err(error) = &result {
                        issue = Some(error.to_string());
                    }
                    if control.stopping() {
                        break;
                    }
                    match app.lock() {
                        Ok(mut app) => {
                            if let Err(error) = app
                                .resume_pending_startup()
                                .and_then(|_| app.recover_integrity_exclusions())
                                .and_then(|_| app.finish_semantic_job(job, result))
                            {
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

#[cfg(test)]
mod startup_status_tests {
    use super::*;
    #[test]
    fn recovery_status_is_available_without_collection_lock() {
        let control = BackgroundControl::new(true);
        control.set_startup_pending(true);
        let guard = control.enter();
        assert_eq!(control.status().phase, "recovering");
        assert!(control.startup_pending());
        control.set_startup_pending(false);
        assert_eq!(control.status().phase, "working");
        drop(guard);
        assert_eq!(control.status().phase, "waiting");
    }
}
