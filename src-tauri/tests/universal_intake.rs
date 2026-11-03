use knowledge_garden::application::{AcquisitionMethod, Application, ExtractionState};
use std::fs;
use tempfile::tempdir;

#[test]
fn empty_supported_text_has_a_distinct_state_and_preserves_its_original() {
    let temp = tempdir().unwrap();
    let mut app = Application::open(temp.path().join("collection")).unwrap();
    for (name, bytes) in [("zero.txt", ""), ("whitespace.md", " \n\t")] {
        let input = temp.path().join(name);
        fs::write(&input, bytes).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        assert_eq!(
            serde_json::to_value(source.info.extraction).unwrap(),
            "empty"
        );
        assert!(source
            .info
            .extraction_detail
            .contains("no extractable text"));
        assert_eq!(
            fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
            bytes.as_bytes()
        );
        assert!(app.claim_due_semantic_jobs(1).unwrap().is_empty());
        let gaps = app.inspect_gaps(&source.info.source_id).unwrap();
        assert_eq!(gaps.gaps.len(), 1);
        assert_eq!(gaps.gaps[0].category, "empty_source");
    }
}

#[test]
fn unknown_and_empty_files_are_retained_with_honest_states() {
    let temp = tempdir().unwrap();
    let mut app = Application::open(temp.path().join("collection")).unwrap();
    for (name, bytes) in [
        ("unknown.weird-extension", &b"\x00\xffbinary"[..]),
        ("empty.txt", &b" \n\t"[..]),
    ] {
        let input = temp.path().join(name);
        fs::write(&input, bytes).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        assert_eq!(
            source.info.extraction,
            if name == "empty.txt" {
                ExtractionState::Empty
            } else {
                ExtractionState::Unsupported
            }
        );
        assert!(source.info.extraction_detail.contains("retained"));
        let original = app
            .original_asset_path(&source.info.source_id, &source.info.asset)
            .unwrap();
        assert_eq!(fs::read(original).unwrap(), bytes);
        let detail = app.open_source(&source.info.source_id).unwrap();
        let summary = app
            .list_sources(0)
            .unwrap()
            .sources
            .into_iter()
            .find(|s| s.source_id == source.info.source_id)
            .unwrap();
        assert_eq!(summary.extraction, detail.info.extraction);
    }
}

#[test]
fn individual_code_and_structured_text_files_are_read_without_execution() {
    let temp = tempdir().unwrap();
    let mut app = Application::open(temp.path().join("collection")).unwrap();
    for (name, contents) in [
        ("measurements.csv", "site,count\nRiverside,12\n"),
        ("record.json", "{\"location\":\"Riverside\"}"),
        ("script.py", "print('source text only')\n"),
    ] {
        let input = temp.path().join(name);
        fs::write(&input, contents).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        assert_eq!(source.info.extraction, ExtractionState::TextPreserved);
        assert!(source.body.contains(contents.trim()));
        assert_eq!(
            fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
            contents.as_bytes()
        );
    }
}

struct EmptyProjection;
impl knowledge_garden::extraction::SourceExtractor for EmptyProjection {
    fn extract(
        &self,
        _: &std::path::Path,
        _: &str,
        _: &str,
    ) -> Result<knowledge_garden::office::OfficeProjection, String> {
        use knowledge_garden::office::*;
        Ok(OfficeProjection {
            markdown: String::new(),
            semantic_text: String::new(),
            line_count: 0,
            coverage: vec![CoveragePart {
                scope: CoverageScope::MainDocument,
                status: CoverageStatus::Complete,
                source_location: None,
                detail: "Examined document.".into(),
            }],
            partial: false,
            detail: "Examined document.".into(),
        })
    }
}
struct NoInference;
impl knowledge_garden::semantic::SemanticProvider for NoInference {
    fn form_knowledge(
        &self,
        _: &str,
    ) -> Result<knowledge_garden::semantic::KnowledgeDraft, knowledge_garden::semantic::ProviderError>
    {
        panic!("empty extraction must not invoke inference")
    }
}

#[test]
fn empty_structured_projection_does_not_claim_complete_coverage() {
    use knowledge_garden::office::CoverageStatus;
    use std::sync::Arc;
    let temp = tempdir().unwrap();
    let input = temp.path().join("empty.docx");
    fs::write(&input, b"fixture original").unwrap();
    let mut app = Application::open_with_providers(
        temp.path().join("collection"),
        Arc::new(NoInference),
        Arc::new(EmptyProjection),
    )
    .unwrap();
    let source = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(source.info.extraction, ExtractionState::PartialText);
    assert!(source
        .info
        .extraction_detail
        .contains("No extractable text"));
    assert!(source
        .info
        .extraction_coverage
        .unwrap()
        .iter()
        .all(|part| part.status != CoverageStatus::Complete));
    assert_eq!(
        fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        b"fixture original"
    );
}
