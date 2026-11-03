use knowledge_garden::{
    application::{AcquisitionMethod, Application},
    semantic::ProviderError,
};
use std::fs;

#[test]
fn empty_sources_keep_replacement_failures_bound_to_the_pending_original() {
    for replacement in [b"\xffreplacement".as_slice(), b"\0binary", b" \r\n"] {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("empty.txt");
        fs::write(&input, []).unwrap();
        let root = temp.path().join("collection");
        let mut app = Application::open(&root).unwrap();
        let initial = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        let gaps = app.inspect_gaps(&initial.info.source_id).unwrap();
        assert_eq!(gaps.gaps.len(), 1);
        assert_eq!(gaps.gaps[0].category, "empty_source");

        fs::write(&input, replacement).unwrap();
        let pending = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        assert_eq!(pending.info.source_id, initial.info.source_id);
        assert_eq!(pending.info.update_status.as_deref(), Some("incomplete"));
        let pending_id = pending.info.pending_version_id.clone().unwrap();
        drop(app);
        let app = Application::open(&root).unwrap();
        let gaps = app.inspect_gaps(&pending.info.source_id).unwrap();
        assert!(gaps.gaps.iter().any(|gap| gap.category == "empty_source"));
        let failed = gaps
            .gaps
            .iter()
            .find(|gap| gap.category == "update_failed")
            .expect("The retained empty version must not hide a replacement failure");
        assert_eq!(failed.detail, pending.info.semantic_error.unwrap());
        let (version, asset) = failed.original_version.as_ref().unwrap();
        assert_eq!(version, &pending_id);
        assert!(failed.original_available);
        assert_eq!(
            fs::read(
                app.original_version_path(&pending.info.source_id, version, asset)
                    .unwrap()
            )
            .unwrap(),
            replacement
        );
    }
}

#[test]
fn exhausted_initial_processing_agrees_with_its_version_and_retry_state() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let input = temp.path().join("garden.txt");
    fs::write(&input, "The garden opens on November 15, 2026.").unwrap();
    let mut app = Application::open(&root).unwrap();
    let source = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    for _ in 0..3 {
        let job = app
            .claim_due_semantic_jobs_at(1, u128::MAX / 2)
            .unwrap()
            .pop()
            .unwrap();
        app.finish_semantic_job(job, Err(ProviderError::recoverable(
            "No supported event identity or acquired reference was found; semantic coverage remains incomplete and will retry automatically.".into(),
        ))).unwrap();
    }
    let failed = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(failed.info.semantic_state, "failed");
    assert_eq!(failed.info.update_status.as_deref(), Some("failed"));
    assert!(failed.info.semantic_retry_at.is_none());
    assert!(!failed
        .info
        .semantic_error
        .unwrap()
        .contains("will retry automatically"));
    assert!(app
        .claim_due_semantic_jobs_at(1, u128::MAX / 2)
        .unwrap()
        .is_empty());
}
