use knowledge_garden::{
    application::{AcquisitionMethod, Application, PageSearchRequest},
    semantic::{EntityDraft, EvidenceDraft, KnowledgeDraft, ProviderError, SemanticProvider},
};
use std::{fs, sync::Arc};

struct MarkerProvider;
impl SemanticProvider for MarkerProvider {
    fn form_knowledge(&self, text: &str) -> Result<KnowledgeDraft, ProviderError> {
        assert!(text.len() <= 32 * 1024, "provider input must stay bounded");
        let Some(start) = text.find("LongTextMarker") else {
            return Ok(KnowledgeDraft::default());
        };
        let evidence = EvidenceDraft {
            quote: "LongTextMarker".into(),
            byte_start: start,
            byte_end: start + "LongTextMarker".len(),
            origin: "source".into(),
            qualifier: None,
            offset_basis: None,
            source_location: None,
        };
        Ok(KnowledgeDraft {
            entities: vec![EntityDraft {
                kind: "topic".into(),
                label: "LongTextMarker".into(),
                evidence,
            }],
            ..Default::default()
        })
    }
}

#[test]
fn long_source_is_searchable_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("collection");
    let input = dir.path().join("long.txt");
    let contents = format!("{}🙂 LongTextMarker", "x".repeat(2 * 1024 * 1024));
    fs::write(&input, contents.as_bytes()).unwrap();
    let id;
    {
        let mut app =
            Application::open_with_semantic_provider(&root, Arc::new(MarkerProvider)).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        id = source.info.source_id;
        app.resume_due_semantic_jobs().unwrap();
    }
    let mut app =
        Application::open_with_semantic_provider(&root, Arc::new(MarkerProvider)).unwrap();
    for _ in 0..100 {
        app.resume_due_semantic_jobs().unwrap();
    }
    let source = app.open_source(&id).unwrap();
    assert_eq!(
        fs::read(app.original_path(&id).unwrap()).unwrap(),
        contents.as_bytes()
    );
    assert_eq!(
        source.info.semantic_state, "complete",
        "{}",
        source.info.extraction_detail
    );
    let result = app
        .search_pages(PageSearchRequest {
            query: "LongTextMarker".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(!result.pages.is_empty());
}

fn finish(app: &mut Application) {
    for _ in 0..100 {
        app.resume_due_semantic_jobs().unwrap();
    }
}

#[test]
fn overlap_restart_and_global_utf8_offsets_survive_a_partial_checkpoint() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let input = temp.path().join("long.txt");
    let contents = format!(
        "{}LongTextMarker\n{}🙂 LongTextMarker",
        "é".repeat((32 * 1024 - 6) / 2),
        "x".repeat(2 * 1024 * 1024)
    );
    fs::write(&input, &contents).unwrap();
    let mut app =
        Application::open_with_semantic_provider(&root, Arc::new(MarkerProvider)).unwrap();
    let source = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    for _ in 0..4 {
        let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
        let draft = MarkerProvider.form_knowledge(&job.source_text).unwrap();
        app.finish_semantic_job(job, Ok(draft)).unwrap();
    }
    assert!(
        app.open_source(&source.info.source_id)
            .unwrap()
            .knowledge_pages
            .is_empty(),
        "partial drafts must not publish"
    );
    drop(app);
    let mut app =
        Application::open_with_semantic_provider(&root, Arc::new(MarkerProvider)).unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    assert!(
        job.text_chunk.as_ref().unwrap().start > 0,
        "restart must resume its checkpoint"
    );
    let draft = MarkerProvider.form_knowledge(&job.source_text).unwrap();
    app.finish_semantic_job(job, Ok(draft)).unwrap();
    finish(&mut app);
    let source = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(
        source.info.semantic_state, "complete",
        "{:?}",
        source.info.semantic_error
    );
    let page = app
        .open_knowledge_page(&source.knowledge_pages[0].page_id)
        .unwrap();
    let header: serde_yaml_ng::Value = serde_yaml_ng::from_str(
        page.markdown
            .strip_prefix("---\n")
            .unwrap()
            .split_once("\n---\n")
            .unwrap()
            .0,
    )
    .unwrap();
    let start = header["evidence"]["byte_start"].as_u64().unwrap() as usize;
    let end = header["evidence"]["byte_end"].as_u64().unwrap() as usize;
    assert_eq!(&contents[start..end], "LongTextMarker");
    assert!(
        source.body.len() < 3 * 1024 * 1024,
        "source reader must remain a bounded preview"
    );
    assert_eq!(
        fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        contents.as_bytes()
    );
}

#[test]
fn removed_and_readded_long_source_rejects_the_old_chunk_lease() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("long.txt");
    fs::write(
        &input,
        format!("{} LongTextMarker", "x".repeat(2 * 1024 * 1024)),
    )
    .unwrap();
    let mut app = Application::open_with_semantic_provider(
        temp.path().join("collection"),
        Arc::new(MarkerProvider),
    )
    .unwrap();
    let source = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    app.remove_source(&source.info.source_id).unwrap();
    app.import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    let draft = MarkerProvider.form_knowledge(&job.source_text).unwrap();
    assert!(app.finish_semantic_job(job, Ok(draft)).is_err());
    finish(&mut app);
    assert_eq!(
        app.open_source(&source.info.source_id)
            .unwrap()
            .info
            .semantic_state,
        "complete"
    );
}

#[test]
fn invalid_later_text_and_sources_beyond_reconciliation_limit_remain_incomplete() {
    for oversized in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("long.txt");
        let mut bytes = vec![
            b'x';
            if oversized {
                32 * 1024 * 1024 + 1
            } else {
                2 * 1024 * 1024 + 1
            }
        ];
        if !oversized {
            *bytes.last_mut().unwrap() = 0xff;
        }
        fs::write(&input, &bytes).unwrap();
        let mut app = Application::open_with_semantic_provider(
            temp.path().join("collection"),
            Arc::new(MarkerProvider),
        )
        .unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        finish(&mut app);
        let source = app.open_source(&source.info.source_id).unwrap();
        assert_ne!(source.info.semantic_state, "complete");
        assert!(source.knowledge_pages.is_empty());
        assert_eq!(
            fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
            bytes
        );
    }
}

#[test]
fn changed_retained_bytes_cannot_publish_a_claimed_long_text_chunk() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("long.txt");
    fs::write(
        &input,
        format!("{} LongTextMarker", "x".repeat(2 * 1024 * 1024)),
    )
    .unwrap();
    let mut app = Application::open_with_semantic_provider(
        temp.path().join("collection"),
        Arc::new(MarkerProvider),
    )
    .unwrap();
    let source = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    fs::write(
        app.original_path(&source.info.source_id).unwrap(),
        b"changed retained bytes",
    )
    .unwrap();
    let draft = MarkerProvider.form_knowledge(&job.source_text).unwrap();
    assert!(app.finish_semantic_job(job, Ok(draft)).is_err());
    assert!(app
        .open_source(&source.info.source_id)
        .unwrap()
        .knowledge_pages
        .is_empty());
}

struct RevisionProvider;
impl SemanticProvider for RevisionProvider {
    fn form_knowledge(&self, text: &str) -> Result<KnowledgeDraft, ProviderError> {
        use knowledge_garden::semantic::{FactDraft, SourceUpdateDraft, SourceUpdateRole};
        assert!(text.len() <= 32 * 1024);
        let evidence = |quote: &str| {
            let start = text.find(quote).unwrap();
            EvidenceDraft {
                quote: quote.into(),
                byte_start: start,
                byte_end: start + quote.len(),
                origin: "source".into(),
                qualifier: None,
                offset_basis: None,
                source_location: None,
            }
        };
        let mut draft = KnowledgeDraft::default();
        if text.contains("Complete replacement.") {
            draft.source_update = Some(SourceUpdateDraft {
                role: SourceUpdateRole::CompleteReplacement,
                evidence: evidence("Complete replacement."),
                certainty: 0.99,
                source_date: None,
                source_date_evidence: None,
                source_date_certainty: 0.0,
                source_revision: None,
                source_revision_evidence: None,
                source_revision_certainty: 0.0,
            });
        }
        let value = if text.contains("LongTextMarker count 2") {
            Some("2")
        } else if text.contains("LongTextMarker count 1") {
            Some("1")
        } else {
            None
        };
        if let Some(value) = value {
            let quote = format!("LongTextMarker count {value}");
            draft.entities.push(EntityDraft {
                kind: "topic".into(),
                label: "LongTextMarker".into(),
                evidence: evidence("LongTextMarker"),
            });
            draft.facts.push(FactDraft {
                subject: "LongTextMarker".into(),
                property: "count".into(),
                value: value.into(),
                evidence: evidence(&quote),
                record_key: None,
            });
        }
        Ok(draft)
    }
}

#[test]
fn complete_long_replacement_publishes_atomically_and_preserves_the_prior_original() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let input = temp.path().join("long.txt");
    let first = format!("{}\nLongTextMarker count 1", "x".repeat(2 * 1024 * 1024));
    let second = format!(
        "Complete replacement.\n{}\nLongTextMarker count 2",
        "y".repeat(2 * 1024 * 1024)
    );
    fs::write(&input, &first).unwrap();
    let mut app =
        Application::open_with_semantic_provider(&root, Arc::new(RevisionProvider)).unwrap();
    let source = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    finish(&mut app);
    let old = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(
        old.info.semantic_state, "complete",
        "{:?}",
        old.info.semantic_error
    );
    let page_id = old.knowledge_pages[0].page_id.clone();
    fs::write(&input, &second).unwrap();
    app.import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    for _ in 0..4 {
        app.resume_due_semantic_jobs().unwrap();
    }
    assert!(app
        .open_knowledge_page(&page_id)
        .unwrap()
        .markdown
        .contains("- **count:** 1"));
    assert_eq!(
        fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        first.as_bytes()
    );
    finish(&mut app);
    let current = app.open_source(&source.info.source_id).unwrap();
    assert!(
        current.info.pending_version_id.is_none(),
        "{:?}",
        current.info.semantic_error
    );
    assert!(app
        .open_knowledge_page(&page_id)
        .unwrap()
        .markdown
        .contains("- **count:** 2"));
    assert_eq!(
        fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        second.as_bytes()
    );
    let old_path = app
        .original_version_path(&source.info.source_id, &old.info.sha256, "original.txt")
        .unwrap();
    assert_eq!(fs::read(old_path).unwrap(), first.as_bytes());
    drop(app);
    fs::remove_dir_all(root.join(".derived")).unwrap();
    let mut app = Application::open(&root).unwrap();
    assert!(app
        .search_pages(PageSearchRequest {
            query: "LongTextMarker".into(),
            ..Default::default()
        })
        .unwrap()
        .pages
        .iter()
        .any(|page| page.page_id == page_id));
}

#[test]
fn withdrawing_a_long_replacement_reprocesses_the_remaining_supported_version() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let incoming = temp.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    let first = format!("{}\nLongTextMarker count 1", "x".repeat(2 * 1024 * 1024));
    let second = format!(
        "Complete replacement.\n{}\nLongTextMarker count 2",
        "y".repeat(2 * 1024 * 1024)
    );
    let stable = incoming.join("stable.txt");
    let changing = incoming.join("changing.txt");
    fs::write(&stable, &first).unwrap();
    fs::write(&changing, &first).unwrap();
    let mut app =
        Application::open_with_semantic_provider(&root, Arc::new(RevisionProvider)).unwrap();
    app.watch_directory(&incoming).unwrap();
    app.reconcile_watched_directories(1000).unwrap();
    finish(&mut app);
    let source_id = app.list_sources(0).unwrap().sources[0].source_id.clone();
    let page_id = app.open_source(&source_id).unwrap().knowledge_pages[0]
        .page_id
        .clone();
    fs::write(&changing, &second).unwrap();
    app.reconcile_watched_directories(1000).unwrap();
    finish(&mut app);
    assert!(app
        .open_knowledge_page(&page_id)
        .unwrap()
        .markdown
        .contains("- **count:** 2"));
    fs::remove_file(&changing).unwrap();
    app.reconcile_watched_directories(1000).unwrap();
    app.reconcile_watched_directories(1000).unwrap();
    finish(&mut app);
    let source = app.open_source(&source_id).unwrap();
    assert_eq!(
        source.info.semantic_state, "complete",
        "{:?}",
        source.info.semantic_error
    );
    assert!(app
        .open_knowledge_page(&page_id)
        .unwrap()
        .markdown
        .contains("- **count:** 1"));
    assert_eq!(
        fs::read(app.original_path(&source_id).unwrap()).unwrap(),
        first.as_bytes()
    );
}
