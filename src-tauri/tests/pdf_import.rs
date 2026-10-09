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
