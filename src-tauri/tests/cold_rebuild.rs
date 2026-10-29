use knowledge_garden::application::{AcquisitionMethod, Application, PageSearchRequest};
use std::fs;

#[test]
fn obsolete_checkpoint_cleanup_respects_reconciliation_budget() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let mut app = Application::open(&root).unwrap();
    let pages = root.join("pages");
    fs::create_dir_all(&pages).unwrap();
    for index in 0..24 {
        fs::write(pages.join(format!("owner-{index}.md")), "owner content").unwrap();
    }
    while !app.reconcile_collection(100).unwrap().complete {}
    let database = rusqlite::Connection::open(root.join(".derived/lookup.sqlite")).unwrap();
    let generation: String = database
        .query_row(
            "SELECT generation FROM reconciliation_page_candidates LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let count_old = || -> i64 {
        database
            .query_row(
                "SELECT count(*) FROM reconciliation_page_candidates WHERE generation=?1",
                [&generation],
                |row| row.get(0),
            )
            .unwrap()
    };
    let mut previous = count_old();
    assert_eq!(previous, 24);
    fs::remove_file(pages.join("owner-0.md")).unwrap();
    for _ in 0..200 {
        let status = app.reconcile_collection(1).unwrap();
        let current = count_old();
        assert!(
            previous - current <= 1,
            "each budget unit may prune at most one checkpoint"
        );
        previous = current;
        if status.complete {
            assert_eq!(current, 0);
            return;
        }
    }
    panic!("obsolete checkpoints should eventually be pruned");
}

#[test]
fn interrupted_candidate_inspection_keeps_a_reusable_derived_checkpoint() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let mut app = Application::open(&root).unwrap();
    let pages = root.join("pages");
    fs::create_dir_all(&pages).unwrap();
    for index in 0..24 {
        fs::write(pages.join(format!("owner-{index}.md")), "owner content").unwrap();
    }
    for _ in 0..32 {
        assert!(!app.reconcile_collection(1).unwrap().complete);
    }
    drop(app);
    let database = rusqlite::Connection::open(root.join(".derived/lookup.sqlite")).unwrap();
    let saved: i64 = database
        .query_row(
            "SELECT count(*) FROM reconciliation_page_candidates",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        saved > 0,
        "completed candidate work must survive process exit"
    );
    database.execute(
        "UPDATE reconciliation_page_candidates SET outcome = x'ff' WHERE path = (SELECT path FROM reconciliation_page_candidates LIMIT 1)", [],
    ).unwrap();
    database
        .execute_batch(
            "CREATE TRIGGER verify_checkpoint_reuse BEFORE INSERT ON reconciliation_page_candidates
         WHEN EXISTS (SELECT 1 FROM reconciliation_page_candidates
                      WHERE generation=NEW.generation AND path=NEW.path AND typeof(outcome) = 'text')
         BEGIN SELECT RAISE(ABORT, 'valid checkpoint was recomputed'); END;",
        )
        .unwrap();
    drop(database);
    let app = Application::open(&root).unwrap();
    let status = app.collection_status();
    assert!(status.complete);
    assert_eq!(
        status
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.kind == "malformed_metadata")
            .count(),
        24
    );
    let database = rusqlite::Connection::open(root.join(".derived/lookup.sqlite")).unwrap();
    let corrupt: i64 = database
        .query_row(
            "SELECT count(*) FROM reconciliation_page_candidates WHERE typeof(outcome) <> 'text'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(corrupt, 0, "unreadable checkpoint entries must be reparsed");
    drop(database);
    for index in 0..24 {
        assert_eq!(
            fs::read_to_string(pages.join(format!("owner-{index}.md"))).unwrap(),
            "owner content"
        );
    }
    drop(app);
    fs::remove_file(pages.join("owner-0.md")).unwrap();
    let app = Application::open(&root).unwrap();
    let status = app.collection_status();
    assert_eq!(
        status
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.kind == "malformed_metadata")
            .count(),
        23
    );
    assert!(!status.diagnostics.iter().any(|diagnostic| diagnostic
        .paths
        .iter()
        .any(|path| path == "pages/owner-0.md")));
}

#[cfg(unix)]
#[test]
fn inaccessible_directory_retries_without_omitting_its_pages() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let mut app = Application::open(&root).unwrap();
    let blocked = root.join("pages/blocked");
    fs::create_dir_all(&blocked).unwrap();
    fs::write(blocked.join("owner.md"), "owner content").unwrap();
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o0)).unwrap();
    if fs::read_dir(&blocked).is_ok() {
        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o700)).unwrap();
        return;
    }
    let failed = app.reconcile_collection(100).is_err();
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(
        failed,
        "an inaccessible subtree must not be silently omitted"
    );
    for _ in 0..10 {
        let status = app.reconcile_collection(100).unwrap();
        if status.complete {
            assert!(status.diagnostics.iter().any(|diagnostic| diagnostic
                .paths
                .iter()
                .any(|path| path == "pages/blocked/owner.md")));
            return;
        }
    }
    panic!("the accessible subtree should be reconciled after retry");
}

#[test]
fn reconciliation_restarts_when_a_page_is_added_during_candidate_inspection() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let mut app = Application::open(&root).unwrap();
    let pages = root.join("pages");
    fs::create_dir_all(&pages).unwrap();
    for index in 0..24 {
        fs::write(pages.join(format!("owner-{index}.md")), "owner content").unwrap();
    }
    for _ in 0..32 {
        assert!(!app.reconcile_collection(1).unwrap().complete);
    }
    fs::write(pages.join("added.md"), "new owner content").unwrap();
    for _ in 0..200 {
        let status = app.reconcile_collection(1).unwrap();
        if status.complete {
            assert!(status
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.paths.iter().any(|path| path == "pages/added.md")));
            return;
        }
    }
    panic!("a stable collection should eventually reconcile");
}

#[test]
fn reconciliation_budgets_candidate_inspection_before_publishing() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let mut app = Application::open(&root).unwrap();
    let pages = root.join("pages");
    fs::create_dir_all(&pages).unwrap();
    for index in 0..24 {
        fs::write(pages.join(format!("owner-{index}.md")), "owner content").unwrap();
    }
    let original_revision = app.collection_status().revision;
    let mut calls = 0;
    loop {
        calls += 1;
        let status = app.reconcile_collection(1).unwrap();
        if status.complete {
            assert!(
                calls >= 48,
                "candidate inspection must share the work budget"
            );
            assert!(status.revision > original_revision);
            break;
        }
        assert_eq!(status.revision, original_revision);
        assert!(status.diagnostics.is_empty());
        assert!(calls < 200);
    }
}

#[test]
fn changed_files_keep_reconciliation_status_incomplete_until_retry_succeeds() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let mut app = Application::open(&root).unwrap();
    let pages = root.join("pages");
    fs::create_dir_all(&pages).unwrap();
    for index in 0..12 {
        fs::write(pages.join(format!("owner-{index}.md")), "initial").unwrap();
    }
    for revision in 0..200 {
        let status = app.reconcile_collection(1).unwrap();
        assert!(
            !status.complete,
            "a continually changing scan cannot complete"
        );
        assert!(
            !app.collection_status().complete,
            "a rejected snapshot still needs a retry"
        );
        for index in 0..12 {
            fs::write(
                pages.join(format!("owner-{index}.md")),
                format!("revision {revision}"),
            )
            .unwrap();
        }
    }
    for _ in 0..100 {
        if app.reconcile_collection(100).unwrap().complete {
            assert!(app.collection_status().complete);
            return;
        }
    }
    panic!("a stable collection should eventually reconcile");
}

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
