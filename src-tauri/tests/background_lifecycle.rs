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
#[ignore = "Run via npm run test:rust:startup without unrelated test I/O"]
fn clean_restart_of_nine_small_sources_opens_the_reader_promptly() {
    use knowledge_garden::{
        application::AcquisitionMethod, extraction::LocalExtractor, media::NativeAudioProcessor,
    };
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let mut existing = Application::open(&root).unwrap();
    for index in 0..9 {
        let input = temp.path().join(format!("note-{index}.txt"));
        fs::write(
            &input,
            format!("Garden note {index}. The north beds use drip irrigation."),
        )
        .unwrap();
        existing
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
    }
    Application::record_controlled_shutdown(&root).unwrap();
    drop(existing);
    let started = Instant::now();
    let app = Arc::new(Mutex::new(
        Application::open_pending_with_all_providers_and_meaning_assets(
            &root,
            Arc::new(Offline),
            Arc::new(LocalExtractor),
            Arc::new(NativeAudioProcessor),
            None::<&std::path::Path>,
        )
        .unwrap(),
    ));
    let control = BackgroundControl::new(true);
    control.set_startup_pending(true);
    let events = observe(&control);
    let worker = spawn_worker(
        Arc::clone(&app),
        Arc::new(Offline),
        Arc::clone(&control),
        Duration::from_secs(1),
    )
    .unwrap();
    let deadline = started + Duration::from_secs(60);
    let mut transitions = Vec::new();
    while control.startup_pending() && Instant::now() < deadline {
        if let Ok(event) = events.recv_timeout(Duration::from_millis(20)) {
            if !event
                .recovery_detail
                .as_deref()
                .is_some_and(|detail| detail.starts_with("Backup:"))
            {
                transitions.push((started.elapsed(), event.phase, event.recovery_detail));
            }
        }
    }
    let ready = !control.startup_pending();
    let elapsed = started.elapsed();
    control.stop();
    worker.join().unwrap();
    eprintln!(
        "Reader startup took {elapsed:?}; last transitions: {:?}",
        &transitions[transitions.len().saturating_sub(8)..]
    );
    assert!(
        ready && elapsed < Duration::from_secs(5),
        "Reader startup took {elapsed:?}"
    );
    assert_eq!(
        app.lock().unwrap().list_sources(0).unwrap().sources.len(),
        9
    );
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

#[test]
fn pending_existing_collection_recovers_through_the_background_coordinator() {
    use knowledge_garden::{extraction::LocalExtractor, media::NativeAudioProcessor};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let input = temp.path().join("source.txt");
    fs::write(&input, "A retained fixture source.").unwrap();
    let mut existing = Application::open(&root).unwrap();
    let source = existing
        .import_source(
            &input,
            knowledge_garden::application::AcquisitionMethod::Picker,
        )
        .unwrap();
    drop(existing);
    let app = Arc::new(Mutex::new(
        Application::open_pending_with_all_providers_and_meaning_assets(
            &root,
            Arc::new(Offline),
            Arc::new(LocalExtractor),
            Arc::new(NativeAudioProcessor),
            None::<&std::path::Path>,
        )
        .unwrap(),
    ));
    let control = BackgroundControl::new(true);
    control.set_startup_pending(true);
    let worker = spawn_worker(
        Arc::clone(&app),
        Arc::new(Offline),
        Arc::clone(&control),
        Duration::from_millis(20),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while control.startup_pending() && Instant::now() < deadline {
        assert_eq!(control.status().phase, "recovering");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !control.startup_pending(),
        "recovery did not finish: {:?}",
        control.status()
    );
    assert_eq!(
        app.lock()
            .unwrap()
            .open_source(&source.info.source_id)
            .unwrap()
            .info
            .source_id,
        source.info.source_id
    );
    assert!(root
        .join(".recovery/migration/startup-coordinator.json")
        .is_file());
    control.stop();
    worker.join().unwrap();
}

#[test]
fn unchanged_background_reopen_reuses_the_retained_snapshot() {
    use knowledge_garden::{extraction::LocalExtractor, media::NativeAudioProcessor};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("owner"), b"preserved").unwrap();
    for run in 0..3 {
        let app = Arc::new(Mutex::new(
            Application::open_pending_with_all_providers_and_meaning_assets(
                &root,
                Arc::new(Offline),
                Arc::new(LocalExtractor),
                Arc::new(NativeAudioProcessor),
                None::<&std::path::Path>,
            )
            .unwrap(),
        ));
        let control = BackgroundControl::new(true);
        control.set_startup_pending(true);
        let worker = spawn_worker(
            Arc::clone(&app),
            Arc::new(Offline),
            Arc::clone(&control),
            Duration::from_millis(20),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        while control.startup_pending() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !control.startup_pending(),
            "run {run}: {:?}",
            control.status()
        );
        control.stop();
        worker.join().unwrap();
        Application::record_controlled_shutdown(&root).unwrap();
        drop(app);
        let count = fs::read_dir(root.join(".recovery/migration"))
            .unwrap()
            .filter(|entry| {
                entry
                    .as_ref()
                    .is_ok_and(|entry| entry.file_name().to_string_lossy().starts_with("backup-"))
            })
            .count();
        if run == 1 {
            fs::write(temp.path().join("count"), count.to_string()).unwrap();
        }
        if run == 2 {
            assert_eq!(
                count,
                fs::read_to_string(temp.path().join("count"))
                    .unwrap()
                    .parse::<usize>()
                    .unwrap()
            );
        }
    }
    assert_eq!(fs::read(root.join("owner")).unwrap(), b"preserved");
}

#[test]
fn pending_new_collection_completes_v3_cutover() {
    use knowledge_garden::{extraction::LocalExtractor, media::NativeAudioProcessor};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("retained.bin"), b"retained original").unwrap();
    let app = Arc::new(Mutex::new(
        Application::open_pending_with_all_providers_and_meaning_assets(
            &root,
            Arc::new(Offline),
            Arc::new(LocalExtractor),
            Arc::new(NativeAudioProcessor),
            None::<&std::path::Path>,
        )
        .unwrap(),
    ));
    let control = BackgroundControl::new(true);
    control.set_startup_pending(true);
    let worker = spawn_worker(
        Arc::clone(&app),
        Arc::new(Offline),
        Arc::clone(&control),
        Duration::from_millis(20),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while control.startup_pending() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !control.startup_pending(),
        "recovery did not finish: {:?}",
        control.status()
    );
    assert_eq!(
        fs::read(root.join("retained.bin")).unwrap(),
        b"retained original"
    );
    assert!(fs::read_dir(root.join(".recovery/migration"))
        .unwrap()
        .filter_map(Result::ok)
        .any(|e| e.path().join("manifest.sqlite").is_file()));
    assert!(app.lock().unwrap().list_sources(0).is_ok());
    control.stop();
    worker.join().unwrap();
}

#[test]
fn desktop_startup_preserves_owner_prose_as_a_source() {
    use knowledge_garden::{extraction::LocalExtractor, media::NativeAudioProcessor};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    fs::create_dir_all(root.join("pages")).unwrap();
    let page = r#"---
schema: 1
page_id: page-1
source_id: source-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
title: Historical page
kind: event
evidence:
  quote: source
  byte_start: 0
  byte_end: 6
  line_start: 1
  line_end: 1
  origin: source
  qualifier: null
facts: []
owner_content:
  - A historical owner statement.
uninterpreted_owner_edits: []
---
Historical page.
"#;
    fs::write(root.join("pages/page.md"), page).unwrap();
    let app = Arc::new(Mutex::new(
        Application::open_pending_with_all_providers_and_meaning_assets(
            &root,
            Arc::new(Offline),
            Arc::new(LocalExtractor),
            Arc::new(NativeAudioProcessor),
            None::<&std::path::Path>,
        )
        .unwrap(),
    ));
    let control = BackgroundControl::new(true);
    control.set_startup_pending(true);
    let worker = spawn_worker(
        Arc::clone(&app),
        Arc::new(Offline),
        Arc::clone(&control),
        Duration::from_millis(20),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while control.startup_pending() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !control.startup_pending(),
        "recovery did not finish: {:?}",
        control.status()
    );
    let app = app.lock().unwrap();
    let sources = app.list_sources(0).unwrap().sources;
    assert_eq!(sources.len(), 1);
    let source = app.open_source(&sources[0].source_id).unwrap();
    assert!(source.markdown.contains("historical owner statement"));
    drop(app);
    control.stop();
    worker.join().unwrap();
}
