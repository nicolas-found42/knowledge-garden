use knowledge_garden::{
    application::{AcquisitionMethod, Application, PageSearchRequest},
    semantic::{EntityDraft, EvidenceDraft, KnowledgeDraft, ProviderError, SemanticProvider},
};
use std::{fs, sync::Arc};

struct MarkerProvider;
impl SemanticProvider for MarkerProvider {
    fn form_knowledge(&self, text: &str) -> Result<KnowledgeDraft, ProviderError> {
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
