#![cfg(target_os = "macos")]
use knowledge_garden::application::{AcquisitionMethod, Application, ExtractionState};
use knowledge_garden::semantic::{EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft};
use std::{fs, path::Path};

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/photos")
        .join(name)
}

#[test]
fn native_photo_formats_keep_originals_and_located_ocr_without_inventing_dates() {
    let collection = tempfile::tempdir().unwrap();
    let mut app = Application::open(collection.path()).unwrap();
    for name in [
        "river-no-date.png",
        "river-no-date.heic",
        "river-conflicting-caption.jpg",
    ] {
        let input = fixture(name);
        let page = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        assert_eq!(
            page.info.extraction,
            ExtractionState::StructuredText,
            "{name}: {}",
            page.info.extraction_detail
        );
        assert_eq!(
            fs::read(app.original_path(&page.info.source_id).unwrap()).unwrap(),
            fs::read(input).unwrap()
        );
        assert!(page.body.contains("RIVER SURVEY"));
        assert!(page.body.contains("Sample R17"));
        assert!(page.body.contains("12 visits"));
        let coverage = serde_json::to_value(&page.info.extraction_coverage).unwrap();
        assert!(coverage
            .as_array()
            .unwrap()
            .iter()
            .any(|part| part["scope"] == "image_text" && part["status"] == "complete"));
        assert!(page.body.contains("OCR"));
        assert!(
            page.body.contains("normalized"),
            "the OCR region must remain locatable"
        );
        if name.contains("conflicting") {
            assert!(page.body.contains("2024:05:18 08:30:00"));
            assert!(page
                .body
                .contains("Captured May 17, 2024 by Maya at Riverside."));
            assert!(page.body.contains("unauthenticated"));
            assert!(page.body.contains("supplied caption"));
        } else {
            assert!(!page.body.contains("2024:05:18"));
            assert!(!page.body.contains("May 17, 2024"));
        }
    }
}

#[test]
fn photo_caption_cannot_be_published_as_an_authenticated_capture_date() {
    let collection = tempfile::tempdir().unwrap();
    let mut app = Application::open(collection.path()).unwrap();
    let page = app
        .import_source(
            &fixture("river-conflicting-caption.jpg"),
            AcquisitionMethod::Picker,
        )
        .unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    let quote = "Captured May 17, 2024 by Maya at Riverside.";
    let start = job.source_text.find(quote).unwrap();
    let evidence = EvidenceDraft {
        quote: quote.into(),
        byte_start: start,
        byte_end: start + quote.len(),
        origin: "observed_pixels".into(),
        qualifier: None,
        offset_basis: None,
        source_location: None,
    };
    let draft = KnowledgeDraft {
        entities: vec![EntityDraft {
            kind: "document".into(),
            label: "Imported photo".into(),
            evidence: evidence.clone(),
        }],
        facts: vec![FactDraft {
            subject: "Imported photo".into(),
            property: "capture_date".into(),
            value: "May 17, 2024".into(),
            evidence,
            record_key: Some("photo-capture-date".into()),
        }],
        ..KnowledgeDraft::default()
    };
    app.finish_semantic_job(job, Ok(draft)).unwrap();
    let after = app.open_source(&page.info.source_id).unwrap();
    assert_eq!(after.info.semantic_state, "pending");
    assert!(
        after.info.knowledge_pages.is_empty(),
        "a broad accepted draft must not authenticate caption provenance"
    );
    assert_eq!(
        fs::read(app.original_path(&page.info.source_id).unwrap()).unwrap(),
        fs::read(fixture("river-conflicting-caption.jpg")).unwrap()
    );
}

#[test]
fn unreadable_photo_retains_its_exact_original_and_explicit_failure() {
    let workspace = tempfile::tempdir().unwrap();
    let input = workspace.path().join("broken.jpg");
    let bytes = b"not a JPEG photograph\xff\x00";
    fs::write(&input, bytes).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    let page = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(page.info.extraction, ExtractionState::InvalidContainer);
    assert!(page
        .info
        .extraction_coverage
        .as_ref()
        .is_some_and(|parts| parts
            .iter()
            .any(|part| part.status == knowledge_garden::office::CoverageStatus::Failed)));
    assert_eq!(
        fs::read(app.original_path(&page.info.source_id).unwrap()).unwrap(),
        bytes
    );
    assert!(page.info.knowledge_pages.is_empty());
}

#[test]
fn photo_channel_records_are_durable_searchable_and_reopen_with_region_context() {
    use knowledge_garden::application::PageSearchRequest;
    let collection = tempfile::tempdir().unwrap();
    let mut app = Application::open(collection.path()).unwrap();
    let page = app
        .import_source(
            fixture("river-conflicting-caption.jpg"),
            AcquisitionMethod::Picker,
        )
        .unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    let mut draft = KnowledgeDraft::default();
    for (needle, origin, property, qualifier) in [
        (
            "Decoded image dimensions:",
            "observed_pixels",
            "image_pixels",
            "decoded dimensions only; identity unknown",
        ),
        (
            "2024:05:18 08:30:00",
            "file_metadata",
            "image_metadata",
            "unauthenticated file metadata",
        ),
        (
            "Captured May 17, 2024 by Maya",
            "supplied_caption",
            "image_caption",
            "unverified supplied caption",
        ),
        (
            "RIVER SURVEY",
            "ocr",
            "image_text",
            "OCR transcription; not verified event",
        ),
        (
            "Classifier prediction:",
            "generated_interpretation",
            "image_interpretation",
            "uncertain classifier interpretation",
        ),
    ] {
        let line = job
            .source_text
            .lines()
            .find(|line| line.contains(needle))
            .unwrap();
        let (_, quote) = line.split_once("] ").unwrap();
        let start = job.source_text.find(quote).unwrap();
        let evidence = EvidenceDraft {
            quote: quote.into(),
            byte_start: start,
            byte_end: start + quote.len(),
            origin: origin.into(),
            qualifier: Some(qualifier.into()),
            offset_basis: None,
            source_location: None,
        };
        if draft.entities.is_empty() {
            draft.entities.push(EntityDraft {
                kind: "document".into(),
                label: "Imported photo".into(),
                evidence: evidence.clone(),
            });
        }
        draft.facts.push(FactDraft {
            subject: "Imported photo".into(),
            property: property.into(),
            value: quote.into(),
            evidence,
            record_key: Some(property.into()),
        });
    }
    app.finish_semantic_job(job, Ok(draft)).unwrap();
    let info = app.open_source(&page.info.source_id).unwrap().info;
    assert_eq!(info.semantic_state, "complete");
    let canonical = info.knowledge_pages[0].page_id.clone();
    let markdown = app.open_knowledge_page(&canonical).unwrap().markdown;
    for origin in [
        "observed_pixels",
        "file_metadata",
        "supplied_caption",
        "ocr",
        "generated_interpretation",
    ] {
        assert!(
            markdown.contains(origin),
            "missing durable channel {origin}"
        );
    }
    assert!(markdown.contains("extracted_image_projection"));
    assert!(markdown.contains("normalized bottom-left"));
    assert!(markdown.contains("unauthenticated"));
    let original = fs::read(app.original_path(&page.info.source_id).unwrap()).unwrap();
    let preview = app.preview_original(&page.info.source_id).unwrap();
    assert!(preview.starts_with("data:image/jpeg;base64,/9j/"));
    assert_eq!(
        fs::read(app.original_path(&page.info.source_id).unwrap()).unwrap(),
        original
    );
    drop(app);
    let reopened = Application::open(collection.path()).unwrap();
    let results = reopened
        .search_pages(PageSearchRequest {
            query: "RIVER SURVEY".into(),
            ..PageSearchRequest::default()
        })
        .unwrap();
    let result = results
        .pages
        .iter()
        .find(|result| result.page_id == canonical)
        .unwrap();
    let location = result.match_location.as_ref().unwrap();
    assert_eq!(location.offset_basis, "extracted_image_projection");
    assert!(location
        .source_location
        .as_deref()
        .unwrap()
        .contains("OCR region"));
    assert_eq!(
        reopened.open_knowledge_page(&canonical).unwrap().markdown,
        markdown
    );
}

#[test]
fn injected_wrong_ocr_remains_a_qualified_transcription_and_cannot_become_a_count() {
    use knowledge_garden::{
        extraction::SourceExtractor,
        office::{CoveragePart, CoverageScope, CoverageStatus, OfficeProjection},
        semantic::{ProviderError, SemanticProvider},
    };
    use std::sync::Arc;
    struct RecordedProvider;
    impl SemanticProvider for RecordedProvider {
        fn form_knowledge(&self, _: &str) -> Result<KnowledgeDraft, ProviderError> {
            Err(ProviderError::recoverable("This integrity scenario supplies its recorded draft through the public completion boundary.".into()))
        }
    }
    struct WrongOcr;
    impl SourceExtractor for WrongOcr {
        fn extract(&self, _: &Path, _: &str, _: &str) -> Result<OfficeProjection, String> {
            let text = "[PHOTO OCR region 1; normalized bottom-left x=0.1,y=0.2,width=0.3,height=0.1; channel=ocr] 72 visits (OCR transcription; confidence 0.1200; uncertain; not a verified event or identity)";
            Ok(OfficeProjection { markdown: format!("# Recorded OCR-error fixture\n\n{text}"), semantic_text: text.into(), line_count: 1, coverage: vec![CoveragePart { scope: CoverageScope::ImageText, status: CoverageStatus::Partial, source_location: Some("normalized region 1".into()), detail: "Injected inaccurate OCR: independent pixels say 12 visits, this deliberately supplied response says 72 visits.".into() }], partial: true, detail: "Injected OCR-error integrity fixture; not a real native OCR accuracy result.".into() })
        }
    }
    let collection = tempfile::tempdir().unwrap();
    let mut app = Application::open_with_providers(
        collection.path(),
        Arc::new(RecordedProvider),
        Arc::new(WrongOcr),
    )
    .unwrap();
    let page = app
        .import_source(fixture("river-no-date.png"), AcquisitionMethod::Picker)
        .unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    let quote = job.source_text.split_once("] ").unwrap().1.to_owned();
    let start = job.source_text.find(&quote).unwrap();
    let evidence = EvidenceDraft {
        quote: quote.clone(),
        byte_start: start,
        byte_end: start + quote.len(),
        origin: "ocr".into(),
        qualifier: Some(
            "uncertain OCR transcription; confidence 0.12; not a verified count".into(),
        ),
        offset_basis: None,
        source_location: None,
    };
    let draft = KnowledgeDraft {
        entities: vec![EntityDraft {
            kind: "document".into(),
            label: "Imported photo".into(),
            evidence: evidence.clone(),
        }],
        facts: vec![FactDraft {
            subject: "Imported photo".into(),
            property: "image_text".into(),
            value: quote.clone(),
            evidence: evidence.clone(),
            record_key: Some("ocr-region-1".into()),
        }],
        ..KnowledgeDraft::default()
    };
    app.finish_semantic_job(job, Ok(draft)).unwrap();
    let canonical = app
        .open_source(&page.info.source_id)
        .unwrap()
        .info
        .knowledge_pages[0]
        .page_id
        .clone();
    let markdown = app.open_knowledge_page(&canonical).unwrap().markdown;
    assert!(markdown.contains("uncertain OCR transcription"));
    assert!(markdown.contains("0.1200"));
    assert!(markdown.contains("not a verified"));
    assert!(!markdown.contains("visit_count:"));
    assert_eq!(
        fs::read(app.original_path(&page.info.source_id).unwrap()).unwrap(),
        fs::read(fixture("river-no-date.png")).unwrap()
    );

    // A separately imported source receives a deliberately overbroad semantic draft.
    let second = app
        .import_source(fixture("river-no-date.heic"), AcquisitionMethod::Picker)
        .unwrap();
    let second_job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    let forged = KnowledgeDraft {
        entities: vec![EntityDraft {
            kind: "document".into(),
            label: "Imported photo".into(),
            evidence: evidence.clone(),
        }],
        facts: vec![FactDraft {
            subject: "Imported photo".into(),
            property: "visit_count".into(),
            value: "72".into(),
            evidence,
            record_key: None,
        }],
        ..KnowledgeDraft::default()
    };
    app.finish_semantic_job(second_job, Ok(forged)).unwrap();
    let second = app.open_source(&second.info.source_id).unwrap();
    assert_eq!(second.info.semantic_state, "pending");
    assert!(second.info.knowledge_pages.is_empty());
    assert!(second
        .info
        .semantic_error
        .unwrap()
        .contains("cannot be promoted"));
}
