use knowledge_garden::application::{AcquisitionMethod, Application, PageSearchRequest};
use std::fs;

#[test]
fn malformed_source_is_reported_without_blocking_cold_rebuild_or_other_pending_work() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let input = temp.path().join("healthy.txt");
    fs::write(
        &input,
        "A healthy orchard report records a spring observation.",
    )
    .unwrap();
    let mut app = Application::open(&root).unwrap();
    let good = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    let broken = root.join("sources/broken-source");
    fs::create_dir_all(&broken).unwrap();
    let metadata = "---\nschema: [unclosed\n---\nOwner text must survive.\n";
    fs::write(broken.join("index.md"), metadata).unwrap();
    fs::write(broken.join("original.bin"), b"preserve original bytes").unwrap();
    let nested = broken.join("attachments");
    fs::create_dir_all(&nested).unwrap();
    fs::write(
        nested.join("index.md"),
        "# Owner attachment, not source metadata\n",
    )
    .unwrap();
    let missing = root.join("sources/missing-metadata");
    fs::create_dir_all(&missing).unwrap();
    fs::write(missing.join("original.bin"), b"also preserve this original").unwrap();
    drop(app);
    fs::remove_dir_all(root.join(".derived")).unwrap();
    let mut app = Application::open(&root)
        .expect("a broken record must not prevent access to healthy knowledge");
    let diagnostics = app.collection_status().diagnostics;
    assert!(!diagnostics
        .iter()
        .any(|d| d.paths.iter().any(|p| p.ends_with("attachments/index.md"))));
    assert!(diagnostics.iter().any(|d| d.kind == "malformed_metadata"
        && d.paths
            .iter()
            .any(|p| p.ends_with("sources/broken-source/index.md"))));
    assert!(diagnostics.iter().any(|d| d.kind == "missing_metadata"
        && d.paths
            .iter()
            .any(|p| p.ends_with("sources/missing-metadata/index.md"))));
    assert_eq!(
        fs::read(missing.join("original.bin")).unwrap(),
        b"also preserve this original"
    );
    assert!(app
        .search_pages(PageSearchRequest {
            query: "orchard".into(),
            ..Default::default()
        })
        .unwrap()
        .pages
        .iter()
        .any(|p| p.source_id == good.info.source_id));
    app.resume_due_audio_jobs().unwrap();
    let jobs = app.claim_due_semantic_jobs_at(1, u128::MAX).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].source_id, good.info.source_id);
    assert_eq!(
        fs::read_to_string(broken.join("index.md")).unwrap(),
        metadata
    );
    assert_eq!(
        fs::read(broken.join("original.bin")).unwrap(),
        b"preserve original bytes"
    );
}

#[path = "fixtures/queries/provider.rs"]
mod labeled;

#[test]
fn malformed_supporting_source_keeps_its_knowledge_and_does_not_block_rebuild() {
    use std::sync::Arc;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let input = temp.path().join("fieldwork.txt");
    fs::write(&input, labeled::TEXT).unwrap();
    let mut app =
        Application::open_with_semantic_provider(&root, Arc::new(labeled::LabeledGraph)).unwrap();
    let source = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&source.info.source_id).unwrap();
    let place = source
        .knowledge_pages
        .iter()
        .find(|p| p.title == "Riverside")
        .unwrap()
        .clone();
    let original = fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap();
    let metadata_path = app.page_path(&source.info.source_id).unwrap();
    let broken = "---\nschema: [broken\n---\nPreserve owner source notes.\n";
    drop(app);
    fs::write(&metadata_path, broken).unwrap();
    fs::remove_dir_all(root.join(".derived")).unwrap();
    let mut app =
        Application::open(&root).expect("broken support must not prevent reopening durable pages");
    assert!(app
        .collection_status()
        .diagnostics
        .iter()
        .any(|d| d.kind == "malformed_metadata"
            && d.paths.iter().any(|p| metadata_path.ends_with(p))));
    let page = app.open_knowledge_page(&place.page_id).unwrap();
    assert_eq!(page.page_id, place.page_id);
    let matches = app
        .search_pages(PageSearchRequest {
            query: "Riverside".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(matches
        .pages
        .iter()
        .any(|p| p.page_id == place.page_id && p.processing_status == "source_unavailable"));
    let paths = app
        .explore_paths(knowledge_garden::application::PathExploreRequest {
            start_page_id: place.page_id.clone(),
            max_hops: 1,
            page_size: 10,
            relationship_work_budget: 50,
            continuation: None,
        })
        .expect("path navigation must disclose missing support without failing");
    assert!(paths.diagnostics.iter().any(
        |d| d.kind == knowledge_garden::application::PathExploreDiagnosticKind::MissingSupport
    ));
    assert_eq!(fs::read_to_string(&metadata_path).unwrap(), broken);
    assert_eq!(
        fs::read(metadata_path.parent().unwrap().join("original.txt")).unwrap(),
        original
    );
}
