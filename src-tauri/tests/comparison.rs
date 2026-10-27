use knowledge_garden::{
    application::{
        comparison::{ComparisonRequest, ComparisonStatus},
        AcquisitionMethod, Application,
    },
    semantic::{
        EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft, ProviderError, SemanticProvider,
        SourceUpdateDraft, SourceUpdateRole,
    },
};
use std::{fs, path::Path, sync::Arc};
struct Reports;
impl SemanticProvider for Reports {
    fn form_knowledge(&self, text: &str) -> Result<KnowledgeDraft, ProviderError> {
        let evidence = |quote: &str| {
            let start = text.find(quote).unwrap();
            EvidenceDraft {
                quote: quote.into(),
                byte_start: start,
                byte_end: start + quote.len(),
                origin: "qualified report".into(),
                qualifier: Some("Source qualification retained".into()),
                offset_basis: None,
                source_location: None,
            }
        };
        let event = if text.contains("V17") {
            "Observation V17"
        } else {
            "Observation V18"
        };
        let count = if text.contains("14 visits") {
            "14 visits"
        } else if text.contains("12 visits") {
            "12 visits"
        } else {
            "15 visits"
        };
        let revision = if text.contains("Revision 2") { 2 } else { 1 };
        Ok(KnowledgeDraft {
            entities: vec![EntityDraft {
                kind: "event".into(),
                label: event.into(),
                evidence: evidence(event),
            }],
            facts: vec![FactDraft {
                subject: event.into(),
                property: "visit_count".into(),
                value: count.into(),
                evidence: evidence(count),
                record_key: None,
            }],
            source_update: Some(SourceUpdateDraft {
                role: SourceUpdateRole::CompleteReplacement,
                evidence: evidence(event),
                certainty: 1.0,
                source_date: None,
                source_date_evidence: None,
                source_date_certainty: 0.0,
                source_revision: Some(revision),
                source_revision_evidence: Some(evidence(if revision == 1 {
                    "Revision 1"
                } else {
                    "Revision 2"
                })),
                source_revision_certainty: 1.0,
            }),
            ..Default::default()
        })
    }
}
fn request(id: &str, signature: Option<String>) -> ComparisonRequest {
    ComparisonRequest {
        source_id: id.into(),
        line_start: 1,
        expected_signature: signature,
    }
}
fn durable_files(root: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(std::path::PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().unwrap() == ".derived" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out)
            } else {
                out.push((
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                ))
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}
#[test]
fn qualified_current_reports_invalidate_replacements_and_missing_originals_without_saving_a_view() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let a = temp.path().join("first.txt");
    let b = temp.path().join("second.txt");
    fs::write(
        &a,
        "Revision 1. Observation V17: 12 visits, two unverified.",
    )
    .unwrap();
    fs::write(
        &b,
        "Revision 1. Observation V18: 15 visits, provisional count.",
    )
    .unwrap();
    let mut app = Application::open_with_semantic_provider(&root, Arc::new(Reports)).unwrap();
    let first = app.import_source(&a, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let second = app.import_source(&b, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let before = durable_files(&root);
    let left = app
        .comparison_side(request(&first.info.source_id, None))
        .unwrap();
    let right = app
        .comparison_side(request(&second.info.source_id, None))
        .unwrap();
    assert_eq!(left.status, ComparisonStatus::Fresh);
    assert_eq!(right.status, ComparisonStatus::Fresh);
    assert!(left
        .passage
        .as_ref()
        .unwrap()
        .text
        .contains("two unverified"));
    assert!(right
        .passage
        .as_ref()
        .unwrap()
        .text
        .contains("provisional count"));
    assert_eq!(
        fs::read_to_string(
            app.comparison_original_path(request(&first.info.source_id, left.signature.clone()))
                .unwrap()
        )
        .unwrap(),
        fs::read_to_string(&a).unwrap()
    );
    assert_eq!(durable_files(&root), before);
    fs::write(
        &a,
        "Revision 2. Observation V17: 14 visits, corrected total.",
    )
    .unwrap();
    app.import_source(&a, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let stale = app
        .comparison_side(request(&first.info.source_id, left.signature.clone()))
        .unwrap();
    assert_eq!(stale.status, ComparisonStatus::Stale);
    assert!(stale.passage.is_none());
    assert!(app
        .comparison_original_path(request(&first.info.source_id, left.signature))
        .is_err());
    let refreshed = app
        .comparison_side(request(&first.info.source_id, None))
        .unwrap();
    assert!(refreshed.passage.unwrap().text.contains("14 visits"));
    assert_eq!(
        app.comparison_side(request(&second.info.source_id, right.signature.clone()))
            .unwrap()
            .status,
        ComparisonStatus::Fresh
    );
    fs::remove_file(app.original_path(&second.info.source_id).unwrap()).unwrap();
    let missing = app
        .comparison_side(request(&second.info.source_id, right.signature))
        .unwrap();
    assert_eq!(missing.status, ComparisonStatus::Unavailable);
    assert!(missing.passage.is_none());
    assert!(missing.asset.is_none());
}
#[test]
fn owner_knowledge_changes_invalidate_passages_even_when_original_bytes_do_not_change() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("report.txt");
    fs::write(
        &input,
        "Revision 1. Observation V17: 12 visits, two unverified.",
    )
    .unwrap();
    let mut app =
        Application::open_with_semantic_provider(temp.path().join("collection"), Arc::new(Reports))
            .unwrap();
    let source = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let page = app.open_source(&source.info.source_id).unwrap();
    let original = app
        .comparison_side(request(&source.info.source_id, None))
        .unwrap();
    let knowledge = temp
        .path()
        .join("collection")
        .join(&page.knowledge_pages[0].path);
    let mut content = fs::read_to_string(&knowledge).unwrap();
    content.push_str("\nOwner qualification: the count requires a new interpretation.\n");
    fs::write(knowledge, content).unwrap();
    let stale = app
        .comparison_side(request(&source.info.source_id, original.signature))
        .unwrap();
    assert_eq!(stale.status, ComparisonStatus::Stale);
    assert!(stale.passage.is_none());
}
#[test]
fn unavailable_passages_are_not_fabricated_and_utf8_excerpts_are_bounded() {
    let temp = tempfile::tempdir().unwrap();
    let mut app = Application::open(temp.path().join("collection")).unwrap();
    let binary = temp.path().join("unknown.bin");
    fs::write(&binary, [0, 255, 0, 3]).unwrap();
    let source = app
        .import_source(binary, AcquisitionMethod::Picker)
        .unwrap();
    let missing = app
        .comparison_side(request(&source.info.source_id, None))
        .unwrap();
    assert_eq!(missing.status, ComparisonStatus::Unavailable);
    assert!(missing.passage.is_none());
    assert!(app
        .comparison_original_path(request(&source.info.source_id, None))
        .unwrap()
        .is_file());
    let input = temp.path().join("long.txt");
    fs::write(&input, "🪴".repeat(3000)).unwrap();
    let source = app.import_source(input, AcquisitionMethod::Picker).unwrap();
    let side = app
        .comparison_side(request(&source.info.source_id, None))
        .unwrap();
    let passage = side.passage.unwrap();
    assert!(passage.text.len() <= 8192);
    assert!(passage.truncated);
    assert_eq!(passage.text.len() % 4, 0);
    let mut absent = request(&source.info.source_id, None);
    absent.line_start = 9000;
    let missing = app.comparison_side(absent).unwrap();
    assert_eq!(missing.status, ComparisonStatus::Unavailable);
    assert!(missing.passage.is_none());
}
