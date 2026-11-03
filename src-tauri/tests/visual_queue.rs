#![cfg(target_os = "macos")]
use knowledge_garden::{
    application::{AcquisitionMethod, Application},
    visual_inference::VisualInterpretation,
};
use std::{cell::Cell, path::Path};

fn fixture() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/photos/river-no-date.png"
    ))
}
fn observation() -> VisualInterpretation {
    VisualInterpretation {
        description: "A river is visible.".into(),
        model: "qwen2.5vl:7b".into(),
        observed_revision: "a".repeat(64),
    }
}

#[test]
fn busy_visual_job_retries_after_backoff_and_restart_then_stays_idle() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Application::open(dir.path()).unwrap();
    let page = app
        .import_source(fixture(), AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_visual_jobs_at_with(100, |_| Err("busy".into()))
        .unwrap();
    let pending = app.open_source(&page.info.source_id).unwrap();
    assert_eq!(pending.info.visual_jobs.len(), 1);
    assert_eq!(pending.info.visual_jobs[0].attempts, 1);
    assert_eq!(pending.info.visual_jobs[0].retry_at_ms, 2100);
    assert!(app.claim_due_semantic_jobs_at(1, 200).unwrap().is_empty());
    drop(app);
    let mut app = Application::open(dir.path()).unwrap();
    app.resume_due_visual_jobs_at_with(2099, |_| panic!("backoff was bypassed"))
        .unwrap();
    app.resume_due_visual_jobs_at_with(2100, |_| Ok(observation()))
        .unwrap();
    let done = app.open_source(&page.info.source_id).unwrap();
    assert_eq!(done.info.visual_jobs[0].state, "complete");
    assert!(done.body.contains("A river is visible"));
    let jobs = app.claim_due_semantic_jobs_at(1, 2100).unwrap();
    assert_eq!(jobs.len(), 1);
    assert!(jobs[0]
        .source_text
        .contains("Qwen interpretation: A river is visible."));
    let calls = Cell::new(0);
    app.resume_due_visual_jobs_at_with(99_999, |_| {
        calls.set(calls.get() + 1);
        Ok(observation())
    })
    .unwrap();
    assert_eq!(calls.get(), 0);
}

#[test]
fn interrupted_attempt_is_retried_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Application::open(dir.path()).unwrap();
    let page = app
        .import_source(fixture(), AcquisitionMethod::Picker)
        .unwrap();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.resume_due_visual_jobs_at_with(500, |_| panic!("simulated interruption"))
            .unwrap();
    }));
    assert!(outcome.is_err());
    drop(app);
    let mut app = Application::open(dir.path()).unwrap();
    app.resume_due_visual_jobs_at_with(400_499, |_| panic!("backoff was bypassed"))
        .unwrap();
    app.resume_due_visual_jobs_at_with(400_500, |_| Ok(observation()))
        .unwrap();
    let page = app.open_source(&page.info.source_id).unwrap();
    assert_eq!(page.info.visual_jobs[0].state, "complete");
    assert_eq!(page.info.visual_jobs[0].attempts, 2);
}

#[test]
fn unavailable_visual_runtime_stops_after_finite_retry_budget() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Application::open(dir.path()).unwrap();
    let page = app
        .import_source(fixture(), AcquisitionMethod::Picker)
        .unwrap();
    for now in [0, 2_000, 6_000, 14_000, 30_000] {
        app.resume_due_visual_jobs_at_with(now, |_| Err("resource pressure".into()))
            .unwrap();
    }
    let info = app.open_source(&page.info.source_id).unwrap().info;
    assert_eq!(info.visual_jobs[0].attempts, 5);
    assert_eq!(info.visual_jobs[0].state, "failed");
    app.resume_due_visual_jobs_at_with(1_000_000, |_| panic!("exhausted job ran again"))
        .unwrap();
}

#[test]
fn slow_visual_inference_does_not_hold_collection_lock() {
    use std::sync::{mpsc, Arc, Mutex};
    let dir = tempfile::tempdir().unwrap();
    let mut app = Application::open(dir.path()).unwrap();
    let page = app
        .import_source(fixture(), AcquisitionMethod::Picker)
        .unwrap();
    let claim = app.claim_due_visual_job_at(1_000).unwrap().unwrap();
    let app = Arc::new(Mutex::new(app));
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker_app = app.clone();
    let worker = std::thread::spawn(move || {
        let result = claim.run_with(|_| {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(observation())
        });
        worker_app
            .lock()
            .unwrap()
            .finish_visual_job(claim, result)
            .unwrap();
    });
    started_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    let mut guard = app.try_lock().expect("reading was blocked by inference");
    assert_eq!(
        guard
            .open_source(&page.info.source_id)
            .unwrap()
            .info
            .source_id,
        page.info.source_id
    );
    assert!(guard
        .claim_due_semantic_jobs_at(1, 1_100)
        .unwrap()
        .is_empty());
    assert!(guard.claim_due_visual_job_at(1_100).unwrap().is_none());
    drop(guard);
    release_tx.send(()).unwrap();
    worker.join().unwrap();
    assert_eq!(
        app.lock()
            .unwrap()
            .open_source(&page.info.source_id)
            .unwrap()
            .info
            .visual_jobs[0]
            .state,
        "complete"
    );
}

#[test]
fn removed_and_readded_source_rejects_stale_visual_result() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Application::open(dir.path()).unwrap();
    let source = app
        .import_source(fixture(), AcquisitionMethod::Picker)
        .unwrap();
    let old_claim = app.claim_due_visual_job_at(1_000).unwrap().unwrap();
    app.remove_source(&source.info.source_id).unwrap();
    let readded = app
        .import_source(fixture(), AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(readded.info.source_id, source.info.source_id);
    app.finish_visual_job_at(old_claim, Ok(observation()), 2_000)
        .unwrap();
    let page = app.open_source(&source.info.source_id).unwrap();
    assert!(!page.body.contains("A river is visible"));
    app.resume_due_visual_jobs_at_with(2_000, |_| Ok(observation()))
        .unwrap();
    assert!(app
        .open_source(&source.info.source_id)
        .unwrap()
        .body
        .contains("A river is visible"));
}
