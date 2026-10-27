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

fn exercise_damaged_support(mismatched_identity: bool) {
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
    let broken = if mismatched_identity {
        fs::read_to_string(&metadata_path).unwrap().replacen(
            &source.info.source_id,
            &format!("source-{}", "0".repeat(64)),
            1,
        )
    } else {
        "---\nschema: [broken\n---\nPreserve owner source notes.\n".to_owned()
    };
    drop(app);
    fs::write(&metadata_path, &broken).unwrap();
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

#[test]
fn malformed_supporting_source_keeps_its_knowledge_and_does_not_block_rebuild() {
    exercise_damaged_support(false);
}
#[test]
fn mismatched_support_identity_does_not_point_to_an_unrelated_original() {
    exercise_damaged_support(true);
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn rebuilding_preserves_navigation_to_non_utf8_owner_page_paths() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt, sync::Arc};
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
    let page = source
        .knowledge_pages
        .iter()
        .find(|p| p.title == "Riverside")
        .unwrap();
    let moved = root
        .join("pages")
        .join(OsString::from_vec(b"owner-\xff.md".to_vec()));
    fs::rename(root.join(&page.path), &moved).unwrap();
    app.rebuild_index().unwrap();
    assert_eq!(
        app.open_knowledge_page(&page.page_id).unwrap().page_id,
        page.page_id
    );
    let matches = app
        .search_pages(PageSearchRequest {
            query: "Riverside".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(matches.pages.iter().any(|p| p.page_id == page.page_id));
    assert!(moved.exists());
}

#[test]
fn corrupt_acquisition_reports_its_own_path_and_preserves_other_sources() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let bad_input = temp.path().join("bad.txt");
    let good_input = temp.path().join("good.txt");
    fs::write(&bad_input, "The first report is retained.").unwrap();
    fs::write(&good_input, "The independent orchard report is searchable.").unwrap();
    let mut app = Application::open(&root).unwrap();
    let bad = app
        .import_source(&bad_input, AcquisitionMethod::Picker)
        .unwrap();
    let good = app
        .import_source(&good_input, AcquisitionMethod::Picker)
        .unwrap();
    let directory = app
        .page_path(&bad.info.source_id)
        .unwrap()
        .parent()
        .unwrap()
        .join("acquisitions");
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("damaged.md");
    let corrupt = "---\nmethod: [broken\n---\nKeep this original record.\n";
    drop(app);
    fs::write(&path, corrupt).unwrap();
    fs::remove_dir_all(root.join(".derived")).unwrap();
    let mut app = Application::open(&root).unwrap();
    let canonical_root = root.canonicalize().unwrap();
    let relative = path
        .strip_prefix(&canonical_root)
        .unwrap()
        .to_string_lossy();
    assert!(
        app.collection_status()
            .diagnostics
            .iter()
            .any(|d| d.kind == "malformed_acquisition"
                && d.paths.iter().any(|p| p == relative.as_ref())),
        "the exact bad acquisition path must be reported"
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
    assert_eq!(fs::read_to_string(path).unwrap(), corrupt);
}
