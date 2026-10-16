//! PDF behavior through the agreed public Application import/read boundary.
//! Expected content and pages were frozen by an independent fixture author
//! before native extraction was run; fixture rendering is not an OCR result.
use knowledge_garden::application::{AcquisitionMethod, Application, ExtractionState};
use std::fs;
use tempfile::tempdir;

#[test]
fn text_pdf_import_retains_original_and_readable_page_evidence() {
    let workspace = tempdir().unwrap();
    let input = workspace.path().join("text-report.pdf");
    let original = include_bytes!("fixtures/pdf/source/text-report.pdf");
    fs::write(&input, original).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();

    let imported = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();

    assert_eq!(imported.info.format, "pdf");
    assert_eq!(
        fs::read(app.original_path(&imported.info.source_id).unwrap()).unwrap(),
        original
    );
    assert!(
        matches!(
            imported.info.extraction,
            ExtractionState::StructuredText | ExtractionState::PartialText
        ),
        "an intact text PDF must produce a readable located projection: {}",
        imported.info.extraction_detail
    );
    let page = app.open_source(&imported.info.source_id).unwrap();
    assert!(
        page.body.contains("PDF page 1"),
        "page-one support must be navigable"
    );
    assert!(page
        .body
        .contains("V17 recorded 15 visits at Riverside on May 17, 2024."));
    assert!(page.body.contains("PDF page 2"));
    assert!(page
        .body
        .contains("V17 lasted 10 minutes, excluding setup time."));
    assert!(page
        .body
        .contains("The duration is provisional pending calibration."));
    assert!(
        page.info.extraction_coverage.is_some(),
        "explicit PDF page coverage is retained"
    );
}

#[test]
fn scanned_pdf_recovers_located_text_and_qualifications() {
    let workspace = tempdir().unwrap();
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdf/source/scanned-report.pdf");
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    let imported = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    let page = app.open_source(&imported.info.source_id).unwrap();
    for expected in [
        "V17 recorded 15 visits at Riverside on May 17, 2024.",
        "V17 lasted 10 minutes, excluding setup time.",
        "The duration is provisional pending calibration.",
        "channel=ocr",
        "OCR transcription",
        "PDF page 2",
    ] {
        assert!(
            page.body.contains(expected),
            "missing {expected}: {}",
            page.body
        );
    }
    assert_eq!(
        fs::read(app.original_path(&imported.info.source_id).unwrap()).unwrap(),
        fs::read(input).unwrap()
    );
}

#[test]
fn frozen_pdf_corpus_preserves_pages_layout_gaps_and_inspectable_failures() {
    use knowledge_garden::application::PageSearchRequest;
    use knowledge_garden::office::CoverageStatus;
    let workspace = tempdir().unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    let labels: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/pdf/frozen-labels.json")).unwrap();
    for (name, label) in labels.as_object().unwrap() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pdf/source")
            .join(name);
        let imported = app.import_source(&path, AcquisitionMethod::Picker).unwrap();
        assert_eq!(
            fs::read(app.original_path(&imported.info.source_id).unwrap()).unwrap(),
            fs::read(&path).unwrap(),
            "{name}"
        );
        let page = app.open_source(&imported.info.source_id).unwrap();
        if name == "encrypted.pdf" || name == "damaged.pdf" {
            assert_eq!(
                page.info.extraction,
                ExtractionState::InvalidContainer,
                "{name}"
            );
            assert!(page.info.knowledge_pages.is_empty());
            assert!(page
                .info
                .extraction_coverage
                .as_ref()
                .unwrap()
                .iter()
                .all(|part| part.status == CoverageStatus::Failed));
            continue;
        }
        for fact in label["facts"].as_array().unwrap() {
            let section = format!("## PDF page {}\n", fact["page"]);
            let body = page
                .body
                .split(&section)
                .nth(1)
                .unwrap()
                .split("\n## PDF page")
                .next()
                .unwrap();
            assert!(
                body.contains(fact["text"].as_str().unwrap()),
                "{name}: missing fact {fact}"
            );
            if let Some(qualification) = fact["qualification"].as_str() {
                assert!(
                    body.contains(qualification),
                    "{name}: missing qualification"
                );
            }
        }
        if let Some(forbidden) = label["forbidden_associations"].as_array() {
            for text in forbidden {
                assert!(!page.body.contains(text.as_str().unwrap()), "{name}");
            }
        }
        if name == "columns.pdf" {
            assert!(page.body.contains("Estimated until review."));
            assert!(page.body.contains("Confirmed final count."));
            assert!(page.body.contains("x=45.000") && page.body.contains("x=325.000"));
        }
        if name == "rotated-report.pdf" {
            assert!(page.body.contains("rotation=90; column=1; channel=ocr"));
        }
        if name == "mixed.pdf" {
            assert!(page.body.contains("channel=pdf_text") && page.body.contains("channel=ocr"));
        }
        if name == "low-quality.pdf" {
            assert_eq!(page.info.extraction, ExtractionState::PartialText);
            assert!(page
                .body
                .contains("visible content remains unresolved at PDF points"));
            assert!(
                !page.body.contains("duration not verified"),
                "unreadable text must not be fabricated"
            );
            assert!(page
                .info
                .extraction_coverage
                .as_ref()
                .unwrap()
                .iter()
                .any(|part| part.status == CoverageStatus::Partial));
        }
        let results = app
            .search_pages(PageSearchRequest {
                query: if name == "homework.pdf" {
                    "friction"
                } else {
                    "V17"
                }
                .into(),
                ..PageSearchRequest::default()
            })
            .unwrap();
        assert!(
            results
                .pages
                .iter()
                .any(|result| result.page_id == page.info.page_id),
            "{name}: missing searchable page"
        );
        assert!(page
            .body
            .contains("original opens at the beginning as a disclosed fallback"));
    }
}

fn pdf_draft(text: &str) -> knowledge_garden::semantic::KnowledgeDraft {
    use knowledge_garden::semantic::{EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft};
    let mut draft = KnowledgeDraft::default();
    for (index, line) in text.lines().enumerate() {
        let (locator, quote) = line.split_once("] ").unwrap();
        let start = text.find(quote).unwrap();
        let channel = locator.rsplit_once("channel=").unwrap().1;
        let evidence = EvidenceDraft {
            quote: quote.into(),
            byte_start: start,
            byte_end: start + quote.len(),
            origin: channel.into(),
            qualifier: if channel == "ocr" {
                Some("OCR transcription; unverified".into())
            } else {
                None
            },
            offset_basis: None,
            source_location: None,
        };
        if draft.entities.is_empty() {
            draft.entities.push(EntityDraft {
                kind: "document".into(),
                label: "PDF report".into(),
                evidence: evidence.clone(),
            });
        }
        draft.facts.push(FactDraft {
            subject: "PDF report".into(),
            property: "document_statement".into(),
            value: quote.into(),
            evidence,
            record_key: Some(if quote.contains("recorded 15 visits") {
                "visit-count".into()
            } else if quote.contains("lasted 10 minutes") {
                "duration".into()
            } else {
                format!("region-{index}")
            }),
        });
    }
    draft
}

#[test]
fn pdf_knowledge_and_fine_search_spans_keep_projection_offsets_and_page_context() {
    use knowledge_garden::application::PageSearchRequest;
    let workspace = tempdir().unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdf/source/mixed.pdf");
    let imported = app.import_source(input, AcquisitionMethod::Picker).unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    let draft = pdf_draft(&job.source_text);
    app.finish_semantic_job(job, Ok(draft)).unwrap();
    let source = app.open_source(&imported.info.source_id).unwrap();
    assert_eq!(
        source.info.semantic_state, "complete",
        "{:?}",
        source.info.semantic_error
    );
    let document = source
        .knowledge_pages
        .iter()
        .find(|p| p.kind == "document")
        .unwrap();
    let page_id = document.page_id.clone();
    let markdown = app.open_knowledge_page(&page_id).unwrap().markdown;
    assert!(markdown.contains("extracted_pdf_projection"));
    assert!(markdown.contains("offsets are not original PDF byte offsets"));
    assert!(markdown.contains("PDF page 1 region 2"));
    assert!(markdown.contains("OCR transcription; unverified"));
    drop(app);
    let mut app = Application::open(&collection).unwrap();
    let results = app
        .search_pages(PageSearchRequest {
            query: "excluding setup time".into(),
            ..PageSearchRequest::default()
        })
        .unwrap();
    let result = results.pages.iter().find(|p| p.page_id == page_id).unwrap();
    let location = result.match_location.as_ref().unwrap();
    assert_eq!(location.offset_basis, "extracted_pdf_projection");
    assert!(location
        .source_location
        .as_ref()
        .unwrap()
        .contains("PDF page 1 region 2"));
    assert!(markdown.contains("fallback"));
    assert_eq!(
        app.open_knowledge_page(&page_id).unwrap().markdown,
        markdown
    );
    assert_eq!(
        app.open_source(&imported.info.source_id)
            .unwrap()
            .knowledge_pages
            .len(),
        1,
        "fine spans must not become separate visible nodes"
    );
}

struct InterruptedOcr(std::sync::atomic::AtomicBool);

impl knowledge_garden::extraction::SourceExtractor for InterruptedOcr {
    fn extract(
        &self,
        path: &std::path::Path,
        format: &str,
        title: &str,
    ) -> Result<knowledge_garden::office::OfficeProjection, String> {
        use knowledge_garden::extraction::LocalExtractor;
        let mut result = LocalExtractor.extract(path, format, title)?;
        // An authored external capability response: the selectable pass succeeds,
        // the raster pass is temporarily unavailable for this exact candidate.
        if fs::read(path).unwrap() == include_bytes!("fixtures/pdf/source/mixed.pdf")
            && !self.0.load(std::sync::atomic::Ordering::SeqCst)
        {
            result.semantic_text = result
                .semantic_text
                .lines()
                .filter(|line| !line.contains("channel=ocr"))
                .collect::<Vec<_>>()
                .join("\n");
            result.partial = true;
            result.detail = "PDF page 1 raster region unavailable until OCR resumes.".into();
            result.markdown = result.semantic_text.clone();
            for part in &mut result.coverage {
                part.status = knowledge_garden::office::CoverageStatus::Partial;
                part.detail = result.detail.clone();
            }
        }
        Ok(result)
    }
}

struct RecordedPdfProvider;
impl knowledge_garden::semantic::SemanticProvider for RecordedPdfProvider {
    fn form_knowledge(
        &self,
        text: &str,
    ) -> Result<knowledge_garden::semantic::KnowledgeDraft, knowledge_garden::semantic::ProviderError>
    {
        // Deterministic replacement judgment at the external provider boundary;
        // this test measures recovery integrity, not live semantic accuracy.
        use knowledge_garden::semantic::{SourceUpdateDraft, SourceUpdateRole};
        let mut draft = pdf_draft(text);
        draft.source_update = Some(SourceUpdateDraft {
            role: SourceUpdateRole::CompleteReplacement,
            evidence: draft.facts[0].evidence.clone(),
            certainty: 0.99,
            source_date: None,
            source_date_evidence: None,
            source_date_certainty: 0.0,
            source_revision: None,
            source_revision_evidence: None,
            source_revision_certainty: 0.0,
        });
        Ok(draft)
    }
}

#[test]
fn incomplete_pdf_replacement_preserves_facts_then_recovers_after_restart_and_interruption() {
    use knowledge_garden::semantic::SemanticProvider;
    use std::sync::{atomic::Ordering, Arc};
    let workspace = tempdir().unwrap();
    let collection = workspace.path().join("collection");
    let input = workspace.path().join("report.pdf");
    let baseline = include_bytes!("fixtures/pdf/source/text-report.pdf");
    let replacement = include_bytes!("fixtures/pdf/source/mixed.pdf");
    fs::write(&input, baseline).unwrap();
    let extractor = Arc::new(InterruptedOcr(std::sync::atomic::AtomicBool::new(false)));
    let provider = Arc::new(RecordedPdfProvider);
    let mut app =
        Application::open_with_providers(&collection, provider.clone(), extractor.clone()).unwrap();
    let imported = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&imported.info.source_id).unwrap();
    assert_eq!(
        source.info.semantic_state, "complete",
        "{:?}",
        source.info.semantic_error
    );
    let page_id = source.knowledge_pages[0].page_id.clone();
    let before = app.open_knowledge_page(&page_id).unwrap().markdown;
    assert!(before.contains("10 minutes"));
    fs::write(&input, replacement).unwrap();
    let pending = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(pending.info.update_status.as_deref(), Some("pending"));
    assert!(
        app.claim_due_semantic_jobs(1).unwrap().is_empty(),
        "missing PDF regions cannot authorize omission-based updates"
    );
    assert_eq!(app.open_knowledge_page(&page_id).unwrap().markdown, before);
    assert_eq!(
        fs::read(app.original_path(&imported.info.source_id).unwrap()).unwrap(),
        baseline
    );
    let candidate = app
        .original_version_path(
            &imported.info.source_id,
            pending.info.pending_version_id.as_deref().unwrap(),
            "original.pdf",
        )
        .unwrap();
    assert_eq!(fs::read(candidate).unwrap(), replacement);
    drop(app);
    extractor.0.store(true, Ordering::SeqCst);
    let mut app =
        Application::open_with_providers(&collection, provider.clone(), extractor.clone()).unwrap();
    let future = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        + 60_000;
    let job = app.claim_due_semantic_jobs_at(1, future).unwrap().remove(0);
    assert!(job.source_text.contains("10 minutes, excluding setup time"));
    // A real process interruption after extraction and before publication.
    drop(app);
    let mut app =
        Application::open_with_providers(&collection, provider, extractor.clone()).unwrap();
    assert!(app
        .claim_due_semantic_jobs_at(1, future + 60_000)
        .unwrap()
        .is_empty());
    assert_eq!(app.open_knowledge_page(&page_id).unwrap().markdown, before);
    let recovered_job = app
        .claim_due_semantic_jobs_at(1, future + 120_000)
        .unwrap()
        .remove(0);
    let draft = RecordedPdfProvider
        .form_knowledge(&recovered_job.source_text)
        .unwrap();
    extractor.0.store(false, Ordering::SeqCst);
    app.finish_semantic_job(recovered_job, Ok(draft)).unwrap();
    assert_eq!(
        app.open_knowledge_page(&page_id).unwrap().markdown,
        before,
        "a capability regression at publication must not delete prior facts"
    );
    assert_eq!(
        fs::read(app.original_path(&imported.info.source_id).unwrap()).unwrap(),
        baseline
    );
    extractor.0.store(true, Ordering::SeqCst);
    let final_job = app
        .claim_due_semantic_jobs_at(1, future + 240_000)
        .unwrap()
        .remove(0);
    let draft = RecordedPdfProvider
        .form_knowledge(&final_job.source_text)
        .unwrap();
    app.finish_semantic_job(final_job, Ok(draft)).unwrap();
    let current = app.open_source(&imported.info.source_id).unwrap();
    assert_eq!(
        current.info.update_status, None,
        "{:?}",
        current.info.semantic_error
    );
    assert!(app
        .open_knowledge_page(&page_id)
        .unwrap()
        .markdown
        .contains("10 minutes, excluding setup time"));
    assert_eq!(
        fs::read(app.original_path(&imported.info.source_id).unwrap()).unwrap(),
        replacement
    );
    assert_ne!(
        current.info.current_version_id,
        source.info.current_version_id
    );
}

#[test]
fn pdf_evidence_cannot_be_retargeted_or_launder_ocr_as_selectable_text() {
    for defect in ["page", "offsets", "channel", "qualification"] {
        let workspace = tempdir().unwrap();
        let mut app = Application::open(workspace.path().join("collection")).unwrap();
        let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pdf/source/scanned-report.pdf");
        let imported = app.import_source(input, AcquisitionMethod::Picker).unwrap();
        let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
        let mut draft = pdf_draft(&job.source_text);
        let evidence = &mut draft.facts[0].evidence;
        match defect {
            "page" => evidence.source_location = Some("PDF page 99 region 1".into()),
            "offsets" => evidence.offset_basis = Some("preserved_text".into()),
            "channel" => evidence.origin = "pdf_text".into(),
            "qualification" => evidence.qualifier = None,
            _ => unreachable!(),
        }
        app.finish_semantic_job(job, Ok(draft)).unwrap();
        let after = app.open_source(&imported.info.source_id).unwrap();
        assert_eq!(after.info.semantic_state, "pending", "{defect}");
        assert!(
            after.knowledge_pages.is_empty(),
            "{defect}: invalid evidence must not publish"
        );
        assert!(after
            .info
            .semantic_error
            .as_deref()
            .unwrap()
            .contains("PDF"));
    }
}

struct RecordedPdfTransport;
impl knowledge_garden::providers::SystemOneTransport for RecordedPdfTransport {
    fn complete(
        &self,
        _: &str,
        request: &serde_json::Value,
    ) -> Result<serde_json::Value, knowledge_garden::semantic::ProviderError> {
        use serde_json::json;
        let mut answers = serde_json::Map::new();
        for (key, question) in request["questions"].as_object().unwrap() {
            let answer = match question["type"].as_str().unwrap() {
                "noul" => json!({"type": "noul", "noul": 0.99}),
                "choice" => {
                    let criteria = question["criteria"].as_object().unwrap();
                    let choice = match key.as_str() {
                        "source_update_role" => "unknown",
                        "source_order_date" | "source_order_revision" => "none",
                        "source_update_evidence" => criteria
                            .keys()
                            .find(|key| key.starts_with("span_"))
                            .unwrap(),
                        _ => panic!("Unexpected PDF question: {key}"),
                    };
                    assert!(criteria.contains_key(choice));
                    json!({"type": "choice", "choice": choice, "probabilities": {choice: 0.99}})
                }
                kind => panic!("Unexpected answer type: {kind}"),
            };
            answers.insert(key.clone(), answer);
        }
        Ok(json!({"model": "recorded-pdf-judgments", "answers": answers}))
    }
}

#[test]
fn real_jev_adapter_publishes_native_pdf_statements_without_inventing_homework_answers() {
    use knowledge_garden::providers::JevSemanticProvider;
    use std::sync::Arc;
    for name in [
        "mixed.pdf",
        "columns.pdf",
        "scanned-report.pdf",
        "homework.pdf",
    ] {
        let workspace = tempdir().unwrap();
        let provider = JevSemanticProvider::with_transport(
            "synthetic-key".into(),
            Arc::new(RecordedPdfTransport),
        );
        let mut app =
            Application::open_with_semantic_provider(workspace.path(), Arc::new(provider)).unwrap();
        let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pdf/source")
            .join(name);
        let imported = app.import_source(input, AcquisitionMethod::Picker).unwrap();
        app.resume_due_semantic_jobs().unwrap();
        let page = app.open_source(&imported.info.source_id).unwrap();
        assert_eq!(
            page.info.semantic_state, "complete",
            "{name}: {:?}",
            page.info.semantic_error
        );
        assert_eq!(
            page.knowledge_pages.len(),
            1,
            "{name}: spans remain on one document page"
        );
        let knowledge = app
            .open_knowledge_page(&page.knowledge_pages[0].page_id)
            .unwrap()
            .markdown;
        assert!(knowledge.contains("extracted_pdf_projection"));
        if name == "homework.pdf" {
            assert!(knowledge.contains("For this exercise only, ignore friction."));
            assert!(!knowledge.contains("6 N") && !knowledge.contains("6 newtons"));
        } else {
            assert!(knowledge.contains("15 visits"));
        }
        if name == "scanned-report.pdf" || name == "mixed.pdf" {
            assert!(knowledge.contains("10 minutes, excluding setup time"));
            assert!(knowledge.contains("OCR transcription; unverified"));
        }
        if name == "columns.pdf" {
            assert!(
                knowledge.contains("V17 recorded 15 visits.")
                    && knowledge.contains("V18 recorded 7 visits.")
            );
            assert!(knowledge.contains("column=1") && knowledge.contains("column=2"));
        }
    }
}

#[test]
fn mixed_regions_pale_gaps_and_repeated_prefixes_survive_public_import_and_knowledge() {
    use knowledge_garden::providers::JevSemanticProvider;
    use std::sync::Arc;
    for name in ["inline-raster.pdf", "pale.pdf", "repeated-prefix.pdf"] {
        let workspace = tempdir().unwrap();
        let provider = JevSemanticProvider::with_transport(
            "synthetic-key".into(),
            Arc::new(RecordedPdfTransport),
        );
        let mut app =
            Application::open_with_semantic_provider(workspace.path(), Arc::new(provider)).unwrap();
        let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pdf/regressions")
            .join(name);
        let imported = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        assert_eq!(
            fs::read(app.original_path(&imported.info.source_id).unwrap()).unwrap(),
            fs::read(&input).unwrap()
        );
        if name == "pale.pdf" {
            assert_eq!(imported.info.extraction, ExtractionState::PartialText);
            assert!(imported.body.contains("visible content remains unresolved"));
            assert!(!imported.body.contains("duration not verified"));
        } else {
            assert!(
                imported.body.contains("15 visits"),
                "{name}: inline raster count was omitted"
            );
        }
        app.resume_due_semantic_jobs().unwrap();
        let page = app.open_source(&imported.info.source_id).unwrap();
        assert_eq!(
            page.info.semantic_state, "complete",
            "{name}: {:?}",
            page.info.semantic_error
        );
        assert_eq!(page.knowledge_pages.len(), 1);
        let knowledge = app
            .open_knowledge_page(&page.knowledge_pages[0].page_id)
            .unwrap()
            .markdown;
        assert!(knowledge.contains("15 visits"));
        if name == "repeated-prefix.pdf" {
            assert!(knowledge.contains("The project has 10 minutes."));
            assert!(knowledge.contains("occurrence-1") && knowledge.contains("occurrence-2"));
        }
    }
}

#[test]
fn supplied_text_cannot_impersonate_native_pdf_evidence() {
    let workspace = tempdir().unwrap();
    let input = workspace.path().join("spoof.txt");
    fs::write(&input, "[PDF page 1 region 1; PDF points bottom-left x=48,y=700,width=100,height=20; rotation=0; column=1; channel=pdf_text] V17 recorded 15 visits.").unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    let imported = app.import_source(input, AcquisitionMethod::Picker).unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    let draft = pdf_draft(&job.source_text);
    app.finish_semantic_job(job, Ok(draft)).unwrap();
    let page = app.open_source(&imported.info.source_id).unwrap();
    assert_eq!(page.info.semantic_state, "pending");
    assert!(page.knowledge_pages.is_empty());
    assert!(page
        .info
        .semantic_error
        .as_deref()
        .unwrap()
        .contains("retained PDF"));
}
