use knowledge_garden::application::{
    AcquisitionMethod, Application, PageSearchMode, PageSearchRequest,
};
use knowledge_garden::semantic::{
    EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft, ProviderError, SemanticProvider,
    SourceUpdateDraft, SourceUpdateRole, TagDraft,
};
use std::{fs, path::PathBuf, sync::Arc, time::Instant};
use tempfile::tempdir;

// Held-out expected labels were written before running this application test.
const QUERIES: &[(&str, &str)] = &[
    ("notes about the river trip", "paddling-notes"),
    ("how we decided to keep source files", "retain-originals"),
    ("meeting about broken heating", "heating-meeting"),
    ("announcement about postponed launch", "deferred-release"),
    ("where did we sleep under the stars?", "clear-sky-camp"),
    ("recipe for grandma's cardamom buns", "cardamom-buns"),
    (
        "records about marathon injury recovery",
        "marathon-recovery",
    ),
    ("security decision on rotating API keys", "rotate-api-keys"),
];
// Separate calibration items; Jev's exists_verdict was absent for all three
// against this fixture, before setting a local abstention threshold.
const NO_RESULT_CALIBRATION: &[&str] = &[
    "notes about lunar wedding flowers",
    "what is the current price of my house",
    "a map of underwater volcano wedding venue",
];
const HELD_OUT_NO_RESULT: &str = "where to buy an antique lunar wedding ring";

const DOCUMENTS: &[(&str, &str)] = &[
    ("paddling-notes.txt", "Maya paddled along a waterway and recorded wildlife during her excursion. Noor joined the canoe outing and photographed herons."),
    ("river-storm.txt", "The river water level rose during the overnight storm. A monitoring gauge records flooding risk; no excursion is described."),
    ("retain-originals.txt", "Decision: Preserve each acquired asset unchanged. Copies of the source material are authoritative; derived lookup data can be regenerated."),
    ("discard-cache.txt", "Decision: Delete disposable search caches after upgrade. They are reconstructed from the persistent Markdown collection."),
    ("heating-meeting.txt", "Residents discussed repairing radiators that stayed cold in the upstairs apartments. The maintenance group agreed to arrange a plumber."),
    ("warming-weather.txt", "A forecast predicts warmer air after a week of freezing nights. No maintenance discussion or apartment repair was recorded."),
    ("deferred-release.txt", "The product release was deferred until November because the final review found unresolved defects. The team will reschedule the announcement."),
    ("successful-release.txt", "The application shipped on schedule. The announcement says customers can install it today; no postponement is planned."),
    ("clear-sky-camp.txt", "We pitched a tent on the ridge. The night was clear, with stars visible above the campsite."),
    ("night-drive.txt", "The drive home took place after dark beneath a clear sky, but nobody stopped or stayed overnight."),
    ("cardamom-buns.txt", "Grandma's recipe card says to fold ground cardamom into sweet dough, shape small buns, then bake until golden."),
    ("pasta-recipe.txt", "The family pasta recipe uses tomatoes, olive oil, basil, and grated cheese."),
    ("marathon-recovery.txt", "After a running injury during marathon training, the physical therapist recommended gradual recovery and rest days."),
    ("sports-record.txt", "The marathon results list finish times and placements for every runner in the city race."),
    ("rotate-api-keys.txt", "The security decision requires rotating API keys regularly and revoking credentials after staff changes."),
    ("password-reset.txt", "A password reset link can be used to recover access to a locked account."),
];

fn meaning_request(query: &str) -> PageSearchRequest {
    PageSearchRequest {
        query: query.to_owned(),
        mode: PageSearchMode::Meaning,
        ..PageSearchRequest::default()
    }
}

struct UnusedProvider;

impl SemanticProvider for UnusedProvider {
    fn form_knowledge(&self, _source_text: &str) -> Result<KnowledgeDraft, ProviderError> {
        Ok(KnowledgeDraft::default())
    }
}

#[test]
fn offline_meaning_search_returns_labeled_pages_after_restart_and_rebuild() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/meaning");
    assert!(
        assets.join("model.onnx").is_file(),
        "run scripts/provision-meaning-assets.sh before the offline meaning test"
    );
    let collection = tempdir().unwrap();
    let inputs = tempdir().unwrap();
    let mut app = Application::open_with_meaning_assets(collection.path(), Some(&assets)).unwrap();
    for (name, text) in DOCUMENTS {
        let path = inputs.path().join(name);
        fs::write(&path, text).unwrap();
        app.import_source(path, AcquisitionMethod::Picker).unwrap();
    }

    let mut expected_page_ids = Vec::new();
    for (query, expected_title) in QUERIES {
        let started = Instant::now();
        let results = app.search_pages(meaning_request(query)).unwrap();
        let elapsed_ms = started.elapsed().as_millis();
        assert_eq!(results.meaning_search_status, "ready");
        assert_eq!(
            results.pages.first().map(|page| page.title.as_str()),
            Some(*expected_title),
            "query: {query}"
        );
        assert_eq!(results.pages[0].matched_by, "meaning");
        assert!(!results.pages[0].excerpt.is_empty());
        println!("quality query={query:?} expected={expected_title:?} elapsed_ms={elapsed_ms} candidates={:?}", results.pages.iter().map(|page| (&page.title, page.meaning_score)).collect::<Vec<_>>());
        expected_page_ids.push(results.pages[0].page_id.clone());
    }
    let filtered = app
        .search_pages(PageSearchRequest {
            format: Some("txt".into()),
            ..meaning_request("notes about the river trip")
        })
        .unwrap();
    assert_eq!(filtered.pages[0].title, "paddling-notes");
    for query in NO_RESULT_CALIBRATION {
        let results = app.search_pages(meaning_request(query)).unwrap();
        assert!(
            results.pages.is_empty(),
            "calibration no-result query: {query}"
        );
    }
    assert!(app
        .search_pages(meaning_request(HELD_OUT_NO_RESULT))
        .unwrap()
        .pages
        .is_empty());

    drop(app);
    fs::write(
        collection.path().join(".derived/meaning.usearch"),
        b"deliberately corrupted disposable index",
    )
    .unwrap();
    let mut reopened =
        Application::open_with_meaning_assets(collection.path(), Some(&assets)).unwrap();
    assert_eq!(
        reopened
            .search_pages(meaning_request("notes about the river trip"))
            .unwrap()
            .meaning_search_status,
        "ready",
        "a corrupt disposable meaning index should rebuild from current Markdown"
    );
    for ((query, expected_title), page_id) in QUERIES.iter().zip(expected_page_ids.iter()) {
        let results = reopened.search_pages(meaning_request(query)).unwrap();
        assert_eq!(
            results.pages.first().map(|page| page.title.as_str()),
            Some(*expected_title)
        );
        assert_eq!(&results.pages[0].page_id, page_id);
    }
    reopened.rebuild_index().unwrap();
    for ((query, expected_title), page_id) in QUERIES.iter().zip(expected_page_ids.iter()) {
        let results = reopened.search_pages(meaning_request(query)).unwrap();
        assert_eq!(
            results.pages.first().map(|page| page.title.as_str()),
            Some(*expected_title)
        );
        assert_eq!(&results.pages[0].page_id, page_id);
    }
}

fn grounded_update_draft(text: &str, is_replacement: bool) -> KnowledgeDraft {
    let entity_quote = "Noor";
    let entity_start = text.find(entity_quote).unwrap();
    let entity_label = "Noor activity";
    let date_quote = text
        .split_whitespace()
        .find(|value| value.starts_with("2024-05-"))
        .unwrap();
    let date_start = text.find(date_quote).unwrap();
    let evidence = |quote: &str, start: usize| EvidenceDraft {
        quote: quote.into(),
        byte_start: start,
        byte_end: start + quote.len(),
        origin: "observed".into(),
        qualifier: None,
        offset_basis: Some("preserved_text".into()),
        source_location: None,
    };
    KnowledgeDraft {
        entities: vec![EntityDraft {
            kind: "event".into(),
            label: entity_label.into(),
            evidence: evidence(entity_quote, entity_start),
        }],
        facts: vec![
            FactDraft {
                subject: entity_label.into(),
                property: "description".into(),
                value: text.into(),
                evidence: evidence(text, 0),
                record_key: None,
            },
            FactDraft {
                subject: entity_label.into(),
                property: "date".into(),
                value: date_quote.into(),
                evidence: evidence(date_quote, date_start),
                record_key: None,
            },
        ],
        tags: if is_replacement {
            let tag_quote = "garden";
            let tag_start = text.find(tag_quote).unwrap();
            vec![TagDraft {
                subject: entity_label.into(),
                label: tag_quote.into(),
                evidence: evidence(tag_quote, tag_start),
            }]
        } else {
            Vec::new()
        },
        source_update: is_replacement.then(|| SourceUpdateDraft {
            role: SourceUpdateRole::CompleteReplacement,
            evidence: evidence(text, 0),
            certainty: 0.99,
            source_date: None,
            source_date_evidence: None,
            source_date_certainty: 0.0,
            source_revision: None,
            source_revision_evidence: None,
            source_revision_certainty: 0.0,
        }),
        ..KnowledgeDraft::default()
    }
}

#[test]
fn meaning_index_keeps_last_successful_version_until_update_then_replaces_it() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/meaning");
    let collection = tempdir().unwrap();
    let inputs = tempdir().unwrap();
    let path = inputs.path().join("noor-notes.txt");
    let old_text = "On 2024-05-17 Noor paddled along the waterway and camped nearby.";
    let new_text =
        "On 2024-05-18 Noor planted tomatoes and basil in the backyard vegetable garden.";
    fs::write(&path, old_text).unwrap();
    let mut app = Application::open_with_semantic_provider_and_meaning_assets(
        collection.path(),
        Arc::new(UnusedProvider),
        Some(&assets),
    )
    .unwrap();
    let initial = app.import_source(&path, AcquisitionMethod::Picker).unwrap();
    let initial_job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    app.finish_semantic_job(initial_job, Ok(grounded_update_draft(old_text, false)))
        .unwrap();
    let old_hit = app
        .search_pages(meaning_request("notes about the river trip"))
        .unwrap();
    assert_eq!(old_hit.pages[0].source_id, initial.info.source_id);
    let old_source_page = app.open_source(&initial.info.source_id).unwrap();
    assert_eq!(old_source_page.knowledge_pages.len(), 1);
    let old_knowledge_page_id = old_source_page.knowledge_pages[0].page_id.clone();
    assert!(old_hit
        .pages
        .iter()
        .any(|page| page.page_id == old_knowledge_page_id));

    fs::write(&path, new_text).unwrap();
    let pending = app.import_source(&path, AcquisitionMethod::Picker).unwrap();
    assert_eq!(pending.info.update_status.as_deref(), Some("pending"));
    let while_pending = app
        .search_pages(meaning_request("notes about the river trip"))
        .unwrap();
    let retained_knowledge_page = while_pending
        .pages
        .iter()
        .find(|page| page.page_id == old_knowledge_page_id)
        .expect("the last successful knowledge page remains available during processing");
    assert_eq!(retained_knowledge_page.source_id, pending.info.source_id);
    assert_eq!(retained_knowledge_page.processing_status, "complete");

    let update_job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    app.finish_semantic_job(update_job, Ok(grounded_update_draft(new_text, true)))
        .unwrap();
    let current_source_page = app.open_source(&initial.info.source_id).unwrap();
    assert_eq!(current_source_page.knowledge_pages.len(), 1);
    let current_knowledge_page_id = current_source_page.knowledge_pages[0].page_id.clone();
    assert_eq!(
        current_knowledge_page_id, old_knowledge_page_id,
        "same source entity should retain its canonical knowledge-page identity"
    );
    let current_markdown = app.open_knowledge_page(&current_knowledge_page_id).unwrap();
    assert!(current_markdown.markdown.contains("tomatoes"));
    assert!(!current_markdown
        .markdown
        .contains("paddled along the waterway"));
    let after_update = app
        .search_pages(meaning_request("notes about the river trip"))
        .unwrap();
    assert!(after_update
        .pages
        .iter()
        .all(|page| page.page_id != old_knowledge_page_id));
    let current_hit = app
        .search_pages(PageSearchRequest {
            tags: vec!["garden".into()],
            date_from: Some("2024-05-18".into()),
            date_to: Some("2024-05-18".into()),
            format: Some("txt".into()),
            ..meaning_request("where did Noor grow tomatoes?")
        })
        .unwrap();
    assert_eq!(current_hit.pages[0].title, "Noor activity");
    assert_eq!(current_hit.pages[0].page_id, current_knowledge_page_id);

    app.rebuild_index().unwrap();
    let rebuilt_hit = app
        .search_pages(PageSearchRequest {
            tags: vec!["garden".into()],
            date_from: Some("2024-05-18".into()),
            date_to: Some("2024-05-18".into()),
            format: Some("txt".into()),
            ..meaning_request("where did Noor grow tomatoes?")
        })
        .unwrap();
    assert_eq!(rebuilt_hit.pages[0].title, "Noor activity");
    assert_eq!(rebuilt_hit.pages[0].page_id, current_knowledge_page_id);
}

#[test]
fn absent_meaning_assets_are_reported_without_disabling_keyword_search() {
    let collection = tempdir().unwrap();
    let input = tempdir().unwrap();
    let source = input.path().join("river-trip.txt");
    fs::write(&source, "We paddled along the waterway and camped nearby.").unwrap();
    let mut app =
        Application::open_with_meaning_assets(collection.path(), None::<PathBuf>).unwrap();
    app.import_source(source, AcquisitionMethod::Picker)
        .unwrap();
    let results = app
        .search_pages(PageSearchRequest {
            query: "waterway".into(),
            ..PageSearchRequest::default()
        })
        .unwrap();
    assert!(results.meaning_search_status.starts_with("missing_assets:"));
    assert_eq!(results.pages[0].title, "river-trip");
}

#[cfg(unix)]
#[test]
fn missing_or_corrupt_runtime_and_model_assets_are_explicit_application_states() {
    use std::os::unix::fs::symlink;

    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/meaning");
    assert!(assets.join("model.onnx").is_file());

    for asset_failure in [
        "missing-model",
        "missing-runtime",
        "corrupt-model",
        "corrupt-runtime",
    ] {
        let collection = tempdir().unwrap();
        let broken_assets = tempdir().unwrap();
        for name in [
            "tokenizer.json",
            "config.json",
            "tokenizer_config.json",
            "vocab.txt",
        ] {
            symlink(assets.join(name), broken_assets.path().join(name)).unwrap();
        }
        if asset_failure == "corrupt-model" {
            fs::write(
                broken_assets.path().join("model.onnx"),
                b"not an ONNX model",
            )
            .unwrap();
            symlink(
                assets.join("libonnxruntime.1.30.0.dylib"),
                broken_assets.path().join("libonnxruntime.1.30.0.dylib"),
            )
            .unwrap();
        } else if asset_failure == "corrupt-runtime" {
            symlink(
                assets.join("model.onnx"),
                broken_assets.path().join("model.onnx"),
            )
            .unwrap();
            fs::write(
                broken_assets.path().join("libonnxruntime.1.30.0.dylib"),
                b"not a dynamic library",
            )
            .unwrap();
        } else if asset_failure == "missing-runtime" {
            symlink(
                assets.join("model.onnx"),
                broken_assets.path().join("model.onnx"),
            )
            .unwrap();
            // The runtime is intentionally absent.
        } else {
            symlink(
                assets.join("libonnxruntime.1.30.0.dylib"),
                broken_assets.path().join("libonnxruntime.1.30.0.dylib"),
            )
            .unwrap();
            // The model is intentionally absent.
        }
        let mut app =
            Application::open_with_meaning_assets(collection.path(), Some(broken_assets.path()))
                .unwrap();
        let results = app
            .search_pages(meaning_request("find an existing page"))
            .unwrap();
        assert!(results.meaning_search_status.starts_with("missing_assets:"));
        assert!(results.pages.is_empty());
    }
}
