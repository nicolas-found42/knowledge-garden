use knowledge_garden::{
    application::{AcquisitionMethod, Application},
    semantic::ProviderError,
};
use std::fs;

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
