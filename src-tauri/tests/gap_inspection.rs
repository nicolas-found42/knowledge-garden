use knowledge_garden::{
    application::{AcquisitionMethod, Application, PageSearchRequest},
    semantic::{
        EntityDraft, EvidenceDraft, KnowledgeDraft, ProviderError, RelationshipDraft,
        SemanticProvider,
    },
};
use std::{
    fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tempfile::tempdir;
struct Recovering(Arc<AtomicBool>);
impl SemanticProvider for Recovering {
    fn form_knowledge(&self, text: &str) -> Result<KnowledgeDraft, ProviderError> {
        if !self.0.load(Ordering::SeqCst) {
            return Err(ProviderError::recoverable(
                "Network temporarily unavailable".into(),
            ));
        }
        let evidence = |quote: &str| {
            let start = text.find(quote).unwrap();
            EvidenceDraft {
                quote: quote.into(),
                byte_start: start,
                byte_end: start + quote.len(),
                origin: "fixture".into(),
                qualifier: None,
                offset_basis: None,
                source_location: None,
            }
        };
        Ok(KnowledgeDraft {
            entities: vec![
                EntityDraft {
                    kind: "person".into(),
                    label: "Maya".into(),
                    evidence: evidence("Maya"),
                },
                EntityDraft {
                    kind: "place".into(),
                    label: "Riverside".into(),
                    evidence: evidence("Riverside"),
                },
            ],
            relationships: vec![RelationshipDraft {
                from: "Maya".into(),
                to: "Riverside".into(),
                kind: "visited".into(),
                qualifier: Some("Unconfirmed location".into()),
                evidence: evidence(text),
            }],
            ..KnowledgeDraft::default()
        })
    }
}
fn categories(app: &Application, id: &str) -> Vec<String> {
    app.inspect_gaps(id)
        .unwrap()
        .gaps
        .into_iter()
        .map(|g| g.category)
        .collect()
}
#[test]
fn recovery_removes_only_the_resolved_gap_while_qualified_and_missing_support_stay_readable() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("collection");
    let source = temp.path().join("report.txt");
    fs::write(&source, "Maya visited Riverside; location unconfirmed.").unwrap();
    let ready = Arc::new(AtomicBool::new(false));
    let mut app =
        Application::open_with_semantic_provider(&root, Arc::new(Recovering(ready.clone())))
            .unwrap();
    let pending = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    assert!(categories(&app, &pending.info.source_id)
        .iter()
        .any(|s| s == "update_pending"));
    let unsupported = temp.path().join("unknown.bin");
    fs::write(&unsupported, [0, 255, 0]).unwrap();
    let other = app
        .import_source(&unsupported, AcquisitionMethod::Picker)
        .unwrap();
    assert!(categories(&app, &other.info.source_id)
        .iter()
        .any(|s| s == "extraction_incomplete"));
    ready.store(true, Ordering::SeqCst);
    app.resume_due_semantic_jobs_at(9_000_000_000_000).unwrap();
    assert!(!categories(&app, &pending.info.source_id)
        .iter()
        .any(|s| s == "update_pending"));
    assert!(categories(&app, &other.info.source_id)
        .iter()
        .any(|s| s == "extraction_incomplete"));
    let complete = app.open_source(&pending.info.source_id).unwrap();
    let page = &complete.info.knowledge_pages[0];
    let gaps = app.inspect_gaps(&page.page_id).unwrap();
    let qualified = gaps
        .gaps
        .iter()
        .find(|g| g.category == "insufficient_support")
        .unwrap();
    assert!(qualified.detail.contains("Unconfirmed location"));
    assert!(qualified.original_available);
    assert!(qualified.evidence.is_some());
    fs::remove_file(app.original_path(&complete.info.source_id).unwrap()).unwrap();
    app.rebuild_index().unwrap();
    assert!(categories(&app, &page.page_id)
        .iter()
        .any(|s| s == "source_unavailable"));
    assert!(app
        .open_knowledge_page(&page.page_id)
        .unwrap()
        .markdown
        .contains("Maya"));
    let results = app
        .search_pages(PageSearchRequest {
            processing_status: Some("source_unavailable".into()),
            ..PageSearchRequest::default()
        })
        .unwrap();
    assert!(results
        .pages
        .iter()
        .any(|p| p.source_id == complete.info.source_id));
}
#[test]
fn duplicate_page_identity_is_explained_from_source_without_selecting_an_owner_file() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("collection");
    let source = temp.path().join("report.txt");
    fs::write(&source, "Maya visited Riverside; location unconfirmed.").unwrap();
    let mut app = Application::open_with_semantic_provider(
        &root,
        Arc::new(Recovering(Arc::new(AtomicBool::new(true)))),
    )
    .unwrap();
    let source = app
        .import_source(source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs_at(9_000_000_000_000).unwrap();
    let source = app.open_source(&source.info.source_id).unwrap();
    let path = root.join(&source.info.knowledge_pages[0].path);
    let duplicate = root.join("pages/duplicate-owner-save.md");
    fs::copy(&path, &duplicate).unwrap();
    assert!(categories(&app, &source.info.source_id)
        .iter()
        .any(|s| s == "identity_ambiguous"));
    assert_eq!(fs::read(&path).unwrap(), fs::read(&duplicate).unwrap());
    assert!(app.original_path(&source.info.source_id).is_ok());
}
#[test]
fn persisted_partial_failed_and_ambiguous_update_states_keep_separate_scope_and_filters() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("collection");
    let source = temp.path().join("report.txt");
    fs::write(&source, "Maya visited Riverside; location unconfirmed.").unwrap();
    let mut app = Application::open(&root).unwrap();
    let page = app
        .import_source(source, AcquisitionMethod::Picker)
        .unwrap();
    let path = root
        .join("sources")
        .join(page.info.source_id.trim_start_matches("source-"))
        .join("index.md");
    let bytes = fs::read_to_string(&path).unwrap();
    let mut pieces = bytes.splitn(3, "---");
    pieces.next();
    let yaml = pieces.next().unwrap();
    let body = pieces.next().unwrap();
    let mut header: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml).unwrap();
    for (status, error, category) in [
        ("failed", "Extraction failed", "update_failed"),
        (
            "uncertain",
            "Event identity is ambiguous",
            "identity_ambiguous",
        ),
        ("pending", "Processing queued", "update_pending"),
    ] {
        header["current_version_id"] = page.info.sha256.clone().into();
        header["update_status"] = status.into();
        header["semantic_error"] = error.into();
        header["extraction"] = "partial_text".into();
        header["extraction_detail"] = "Pages 3–4 unavailable; pages 1–2 preserved".into();
        fs::write(
            &path,
            format!(
                "---\n{}---{}",
                serde_yaml_ng::to_string(&header).unwrap(),
                body
            ),
        )
        .unwrap();
        app.rebuild_index().unwrap();
        let query: knowledge_garden::application::boolean::BooleanQueryRequest = serde_json::from_value(serde_json::json!({"expression":{"op":"constraint","constraint":{"processing_status":category}},"returned":{},"continuation":null,"work_budget":50,"page_size":50})).unwrap();
        assert!(app
            .query_pages(query)
            .unwrap()
            .results
            .iter()
            .any(|p| p.source_id == page.info.source_id));
        let gaps = app.inspect_gaps(&page.info.source_id).unwrap();
        assert!(gaps.gaps.iter().any(|g| g.category == category));
        assert!(gaps
            .gaps
            .iter()
            .any(|g| g.category == "extraction_incomplete"));
        assert!(app
            .search_pages(PageSearchRequest {
                processing_status: Some(category.into()),
                ..PageSearchRequest::default()
            })
            .unwrap()
            .pages
            .iter()
            .any(|p| p.source_id == page.info.source_id));
    }
}

#[test]
fn pending_replacement_gap_opens_the_affected_original_version() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("collection");
    let path = temp.path().join("report.txt");
    fs::write(&path, "Maya visited Riverside; location unconfirmed.").unwrap();
    let ready = Arc::new(AtomicBool::new(true));
    let mut app =
        Application::open_with_semantic_provider(&root, Arc::new(Recovering(ready.clone())))
            .unwrap();
    let first = app.import_source(&path, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs_at(9_000_000_000_000).unwrap();
    ready.store(false, Ordering::SeqCst);
    fs::write(
        &path,
        "Maya may have visited Riverside; changed replacement report.",
    )
    .unwrap();
    let replacement = app.import_source(&path, AcquisitionMethod::Picker).unwrap();
    assert_eq!(replacement.info.source_id, first.info.source_id);
    let explanation = app
        .inspect_gaps(&first.info.source_id)
        .unwrap()
        .gaps
        .into_iter()
        .find(|g| g.category == "update_pending")
        .unwrap();
    let (version, asset) = explanation
        .original_version
        .expect("pending explanation must name its pending original");
    assert_eq!(
        Some(version.as_str()),
        replacement.info.pending_version_id.as_deref()
    );
    let retained = app
        .original_version_path(&first.info.source_id, &version, &asset)
        .unwrap();
    assert_eq!(
        fs::read_to_string(retained).unwrap(),
        "Maya may have visited Riverside; changed replacement report."
    );
}

#[test]
fn different_relationship_support_keeps_quote_qualification_and_original_together() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("collection");
    let mut app = Application::open_with_semantic_provider(
        &root,
        Arc::new(Recovering(Arc::new(AtomicBool::new(true)))),
    )
    .unwrap();
    let a = temp.path().join("a.txt");
    fs::write(&a, "Maya visited Riverside; location unconfirmed.").unwrap();
    let source_a = app.import_source(&a, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs_at(9_000_000_000_000).unwrap();
    let source_a = app.open_source(&source_a.info.source_id).unwrap();
    let b = temp.path().join("b.txt");
    let text_b = "Maya possibly visited Riverside; a second qualified witness.";
    fs::write(&b, text_b).unwrap();
    let source_b = app.import_source(&b, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs_at(9_000_000_000_000).unwrap();
    let source_b = app.open_source(&source_b.info.source_id).unwrap();
    let page = &source_a.info.knowledge_pages[0];
    let path = root.join(&page.path);
    let bytes = fs::read_to_string(&path).unwrap();
    let mut chunks = bytes.splitn(3, "---");
    chunks.next();
    let yaml = chunks.next().unwrap();
    let body = chunks.next().unwrap();
    let mut header: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml).unwrap();
    let mut support = header["relationships"][0]["supports"][0].clone();
    support["source_id"] = source_b.info.source_id.clone().into();
    support["source_version_id"] = source_b.info.current_version_id.clone().unwrap().into();
    support["qualifier"] = "Second witness, tentative".into();
    support["evidence"]["quote"] = text_b.into();
    header["relationships"][0]["supports"]
        .as_sequence_mut()
        .unwrap()
        .push(support);
    fs::write(
        &path,
        format!(
            "---\n{}---{}",
            serde_yaml_ng::to_string(&header).unwrap(),
            body
        ),
    )
    .unwrap();
    let item = app
        .inspect_gaps(&page.page_id)
        .unwrap()
        .gaps
        .into_iter()
        .find(|g| g.category == "insufficient_support")
        .unwrap();
    assert_eq!(
        item.source_id.as_deref(),
        Some(source_b.info.source_id.as_str())
    );
    assert_eq!(item.evidence.as_deref(), Some(text_b));
    assert!(item.detail.contains("Second witness, tentative"));
}

struct PartialDocument;
impl knowledge_garden::extraction::SourceExtractor for PartialDocument {
    fn extract(
        &self,
        path: &std::path::Path,
        _format: &str,
        _title: &str,
    ) -> Result<knowledge_garden::office::OfficeProjection, String> {
        use knowledge_garden::office::*;
        let text = fs::read_to_string(path).unwrap();
        if text.contains("EXTRACTION_FAIL") {
            return Err("Candidate cannot be extracted".into());
        }
        let partial = text.contains("replacement") || text.contains("partial current");
        Ok(OfficeProjection {
            markdown: text.clone(),
            semantic_text: text.clone(),
            line_count: 1,
            coverage: vec![CoveragePart {
                scope: CoverageScope::MainDocument,
                status: if partial {
                    CoverageStatus::Partial
                } else {
                    CoverageStatus::Complete
                },
                source_location: Some("document paragraphs 1–2".into()),
                detail: if partial {
                    "Replacement page 3 missing"
                } else {
                    "Complete first document"
                }
                .into(),
            }],
            partial,
            detail: text,
        })
    }
}
struct DocumentProvider;
impl SemanticProvider for DocumentProvider {
    fn form_knowledge(&self, text: &str) -> Result<KnowledgeDraft, ProviderError> {
        let evidence = EvidenceDraft {
            quote: text.into(),
            byte_start: 0,
            byte_end: text.len(),
            origin: "document_body".into(),
            qualifier: None,
            offset_basis: None,
            source_location: None,
        };
        Ok(KnowledgeDraft {
            entities: vec![EntityDraft {
                kind: "document".into(),
                label: "Office source document".into(),
                evidence: evidence.clone(),
            }],
            facts: vec![knowledge_garden::semantic::FactDraft {
                subject: "Office source document".into(),
                property: "document_statement".into(),
                value: text.into(),
                record_key: Some("whole-input".into()),
                evidence,
            }],
            ..KnowledgeDraft::default()
        })
    }
}
#[test]
fn partial_replacement_extraction_gaps_point_to_candidate_original_not_published_document() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("collection");
    let path = temp.path().join("document.docx");
    fs::write(&path, "First complete document.").unwrap();
    let mut app = Application::open_with_providers(
        &root,
        Arc::new(DocumentProvider),
        Arc::new(PartialDocument),
    )
    .unwrap();
    let first = app.import_source(&path, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    assert!(app
        .open_source(&first.info.source_id)
        .unwrap()
        .info
        .current_version_id
        .is_some());
    fs::write(&path, "Partial replacement document.").unwrap();
    let candidate = app.import_source(&path, AcquisitionMethod::Picker).unwrap();
    let explanations = app.inspect_gaps(&first.info.source_id).unwrap();
    let extraction: Vec<_> = explanations
        .gaps
        .into_iter()
        .filter(|g| g.category == "extraction_incomplete")
        .collect();
    assert!(!extraction.is_empty());
    for explanation in extraction {
        let (version, asset) = explanation
            .original_version
            .expect("partial extraction belongs to candidate version");
        assert_eq!(
            Some(version.as_str()),
            candidate.info.pending_version_id.as_deref()
        );
        assert_eq!(
            fs::read_to_string(
                app.original_version_path(&first.info.source_id, &version, &asset)
                    .unwrap()
            )
            .unwrap(),
            "Partial replacement document."
        );
    }
}

#[test]
fn failed_candidate_extraction_does_not_rebind_existing_partial_coverage() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("collection");
    let path = temp.path().join("document.docx");
    fs::write(&path, "A partial current document.").unwrap();
    let mut app = Application::open_with_providers(
        &root,
        Arc::new(DocumentProvider),
        Arc::new(PartialDocument),
    )
    .unwrap();
    let first = app.import_source(&path, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let first = app.open_source(&first.info.source_id).unwrap();
    assert!(first.info.current_version_id.is_some());
    fs::write(&path, "EXTRACTION_FAIL changed candidate.").unwrap();
    app.import_source(&path, AcquisitionMethod::Picker).unwrap();
    let gaps = app.inspect_gaps(&first.info.source_id).unwrap();
    for gap in gaps
        .gaps
        .iter()
        .filter(|g| g.category == "extraction_incomplete")
    {
        let (version, asset) = gap
            .original_version
            .as_ref()
            .expect("known partial current coverage version");
        assert_eq!(
            Some(version.as_str()),
            first.info.current_version_id.as_deref()
        );
        assert_eq!(
            fs::read_to_string(
                app.original_version_path(&first.info.source_id, version, asset)
                    .unwrap()
            )
            .unwrap(),
            "A partial current document."
        );
    }
}
