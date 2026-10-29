use knowledge_garden::{
    application::Application,
    background::{spawn_worker, BackgroundControl, BackgroundStatus},
    semantic::{KnowledgeDraft, ProviderAvailability, ProviderError, SemanticProvider},
};
use std::{
    fs,
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
struct Offline;
impl SemanticProvider for Offline {
    fn form_knowledge(&self, _: &str) -> Result<KnowledgeDraft, ProviderError> {
        Err(ProviderError::unavailable(
            "Fixture provider is offline; queued work remains recoverable.".into(),
            ProviderAvailability::Network,
        ))
    }
}
fn wait_turn(events: &Receiver<BackgroundStatus>, turn: u64) {
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(10))
            .expect("bounded worker should finish a maintenance turn");
        if event.phase == "waiting" && event.completed_turns >= turn {
            return;
        }
    }
}
fn observe(control: &Arc<BackgroundControl>) -> Receiver<BackgroundStatus> {
    let (tx, rx) = mpsc::channel();
    control.observe(Arc::new(move |status| {
        let _ = tx.send(status);
    }));
    rx
}
#[test]
fn hidden_reader_continues_watching_quit_stops_and_restart_resumes_without_duplicate_sources() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let incoming = temp.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    fs::write(incoming.join("first.txt"), "First lifecycle source.").unwrap();
    let mut app = Application::open(&root).unwrap();
    app.watch_directory(&incoming).unwrap();
    let app = Arc::new(Mutex::new(app));
    let control = BackgroundControl::new(true);
    let events = observe(&control);
    let worker = spawn_worker(
        Arc::clone(&app),
        Arc::new(Offline),
        Arc::clone(&control),
        Duration::from_secs(3600),
    )
    .unwrap();
    wait_turn(&events, 1);
    let first = app.lock().unwrap().list_sources(0).unwrap().sources;
    assert_eq!(first.len(), 1);
    control.reader_visible(false);
    fs::write(
        incoming.join("second.txt"),
        "Second source supplied after the reader was hidden.",
    )
    .unwrap();
    control.wake();
    wait_turn(&events, 2);
    assert!(!control.status().reader_visible);
    assert_eq!(
        app.lock().unwrap().list_sources(0).unwrap().sources.len(),
        2
    );
    assert!(control.status().last_issue.is_some());
    let disconnected = temp.path().join("disconnected");
    fs::rename(&incoming, &disconnected).unwrap();
    control.wake();
    wait_turn(&events, 3);
    assert_eq!(
        app.lock().unwrap().list_sources(0).unwrap().sources.len(),
        2
    );
    assert_eq!(
        app.lock().unwrap().list_watched_directories().unwrap()[0].state,
        "unavailable"
    );
    fs::rename(disconnected, &incoming).unwrap();
    let stopped = Instant::now();
    control.stop();
    worker.join().unwrap();
    assert!(stopped.elapsed() < Duration::from_secs(2));
    assert_eq!(control.status().phase, "stopped");
    fs::write(
        incoming.join("after-quit.txt"),
        "Supplied after explicit worker stop.",
    )
    .unwrap();
    control.wake();
    assert_eq!(
        app.lock().unwrap().list_sources(0).unwrap().sources.len(),
        2
    );
    drop(app);
    let app = Arc::new(Mutex::new(Application::open(&root).unwrap()));
    let control = BackgroundControl::new(false);
    let events = observe(&control);
    let worker = spawn_worker(
        Arc::clone(&app),
        Arc::new(Offline),
        Arc::clone(&control),
        Duration::from_secs(3600),
    )
    .unwrap();
    wait_turn(&events, 1);
    let current = app.lock().unwrap().list_sources(0).unwrap().sources;
    assert_eq!(current.len(), 3);
    assert_eq!(
        current
            .iter()
            .filter(|s| s.source_id == first[0].source_id)
            .count(),
        1
    );
    control.stop();
    worker.join().unwrap();
}

#[test]
fn repeated_controlled_quits_preserve_semantic_and_media_retry_budgets_but_crashes_still_count() {
    use knowledge_garden::application::AcquisitionMethod;
    use std::path::Path;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let text = temp.path().join("pending.txt");
    fs::write(&text, "A pending lifecycle observation.").unwrap();
    let mut app = Application::open(&root).unwrap();
    let text = app.import_source(text, AcquisitionMethod::Picker).unwrap();
    let audio = app
        .import_source(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/heldout-spoken-acts.wav"),
            AcquisitionMethod::Picker,
        )
        .unwrap();
    for turn in 0..4 {
        let claimed = app
            .claim_due_semantic_jobs_at(1, 1000 + turn * 1000)
            .unwrap();
        assert_eq!(claimed.len(), 1);
        // This is the durable checkpoint present while a native media window is running.
        let mut pending = app.open_source(&audio.info.source_id).unwrap();
        pending.info.audio_processing.as_mut().unwrap().state = "processing".into();
        fs::write(
            app.page_path(&audio.info.source_id).unwrap(),
            format!(
                "---\n{}---\n{}",
                serde_yaml_ng::to_string(&pending.info).unwrap(),
                pending.body
            ),
        )
        .unwrap();
        Application::record_controlled_shutdown(&root).unwrap();
        drop(app);
        app = Application::open(&root).unwrap();
        let restored = app.open_source(&text.info.source_id).unwrap();
        assert_eq!(restored.info.semantic_state, "pending");
        assert_eq!(restored.info.versions_seen[0].processing_interruptions, 0);
        let restored = app.open_source(&audio.info.source_id).unwrap();
        let media = restored.info.audio_processing.as_ref().unwrap();
        assert_eq!(media.state, "pending");
        assert_eq!(media.processing_interruptions, 0);
        assert_eq!(media.next_start_ms, 0);
        assert!(!root.join(".runtime/controlled-exit").exists());
    }
    app.claim_due_semantic_jobs_at(1, 9000).unwrap();
    drop(app);
    let mut app = Application::open(&root).unwrap();
    assert!(app.claim_due_semantic_jobs_at(1, 10000).unwrap().is_empty());
    assert_eq!(
        app.open_source(&text.info.source_id)
            .unwrap()
            .info
            .versions_seen[0]
            .processing_interruptions,
        1
    );
}
