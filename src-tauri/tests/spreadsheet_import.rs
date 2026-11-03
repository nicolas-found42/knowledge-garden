use knowledge_garden::{
    extraction::{LocalExtractor, SourceExtractor},
    office::{CoverageScope, CoverageStatus},
};
use std::path::Path;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/spreadsheets")
        .join(name)
}

#[test]
fn workbook_projects_located_cells_and_discloses_formula_and_other_gaps() {
    let projection = LocalExtractor
        .extract(&fixture("cell-evidence.xlsx"), "xlsx", "Observations")
        .unwrap();
    assert!(projection
        .semantic_text
        .contains("[XLSX sheet \"Field notes\" cell B1; channel=cell] River Delta"));
    assert!(projection
        .semantic_text
        .contains("[XLSX sheet \"Field notes\" cell B2; channel=cell] 42"));
    assert!(projection
        .semantic_text
        .contains("[XLSX sheet \"Field notes\" cell C3; channel=cell] TRUE"));
    assert!(projection
        .semantic_text
        .contains("[XLSX sheet \"Second sheet\" cell A7; channel=cell] Independent observation"));
    assert!(projection.partial);
    assert!(projection
        .coverage
        .iter()
        .any(|part| part.scope == CoverageScope::SpreadsheetCells
            && part.status == CoverageStatus::Partial
            && part.detail.contains("formula")));
}

#[test]
fn empty_and_invalid_workbooks_are_not_complete() {
    let empty = LocalExtractor
        .extract(&fixture("empty.xlsx"), "xlsx", "Empty")
        .unwrap();
    assert!(empty.partial);
    assert!(empty.semantic_text.is_empty());
    let dir = tempfile::tempdir().unwrap();
    let corrupt = dir.path().join("corrupt.xlsx");
    std::fs::write(&corrupt, b"not a workbook").unwrap();
    assert!(LocalExtractor.extract(&corrupt, "xlsx", "Corrupt").is_err());
    let encrypted = dir.path().join("encrypted.xlsx");
    std::fs::write(&encrypted, [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]).unwrap();
    assert!(LocalExtractor
        .extract(&encrypted, "xlsx", "Encrypted")
        .is_err());
}

#[test]
fn unsafe_and_oversized_packages_are_rejected() {
    let unsafe_error = LocalExtractor
        .extract(&fixture("unsafe.xlsx"), "xlsx", "Unsafe")
        .unwrap_err();
    assert!(unsafe_error.contains("unsafe"));
    let oversized_error = LocalExtractor
        .extract(&fixture("oversized.xlsx"), "xlsx", "Oversized")
        .unwrap_err();
    assert!(oversized_error.contains("8 MiB"));
}

#[test]
fn encrypted_workbook_requires_structured_header_and_package() {
    use knowledge_garden::office::{detect_xlsx_container, OfficeContainer};
    use std::io::{Seek, SeekFrom, Write};
    let dir = tempfile::tempdir().unwrap();
    let real = fixture("encrypted-agile.xlsx");
    assert_eq!(detect_xlsx_container(&real), OfficeContainer::Encrypted);
    let forged = dir.path().join("forged.xlsx");
    let mut compound = cfb::create(&forged).unwrap();
    compound.create_stream("/EncryptionInfo").unwrap();
    compound.create_stream("/EncryptedPackage").unwrap();
    drop(compound);
    assert_eq!(detect_xlsx_container(&forged), OfficeContainer::Other);
    let invalid_header = dir.path().join("invalid-header.xlsx");
    std::fs::copy(&real, &invalid_header).unwrap();
    let mut compound = cfb::open_rw(&invalid_header).unwrap();
    compound
        .open_stream("/EncryptionInfo")
        .unwrap()
        .write_all(&[0, 0, 0, 0])
        .unwrap();
    drop(compound);
    assert_eq!(
        detect_xlsx_container(&invalid_header),
        OfficeContainer::Other
    );
    let invalid_payload = dir.path().join("invalid-payload.xlsx");
    std::fs::copy(&real, &invalid_payload).unwrap();
    let mut compound = cfb::open_rw(&invalid_payload).unwrap();
    let mut stream = compound.open_stream("/EncryptedPackage").unwrap();
    stream.seek(SeekFrom::Start(0)).unwrap();
    stream.write_all(&u64::MAX.to_le_bytes()).unwrap();
    drop(stream);
    drop(compound);
    assert_eq!(
        detect_xlsx_container(&invalid_payload),
        OfficeContainer::Other
    );
}

#[test]
fn encrypted_workbook_status_survives_import_and_reopen() {
    use knowledge_garden::application::{AcquisitionMethod, Application, ExtractionState};
    let dir = tempfile::tempdir().unwrap();
    let path = fixture("encrypted-agile.xlsx");
    let collection = dir.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    let imported = app.import_source(&path, AcquisitionMethod::Picker).unwrap();
    assert_eq!(imported.info.extraction, ExtractionState::Encrypted);
    assert!(imported.info.extraction_detail.contains("encrypted"));
    let id = imported.info.source_id;
    drop(app);
    let app = Application::open(&collection).unwrap();
    assert_eq!(
        app.open_source(&id).unwrap().info.extraction,
        ExtractionState::Encrypted
    );
}

#[test]
fn untrusted_workbook_text_cannot_break_locator_or_insert_markdown() {
    let projection = LocalExtractor
        .extract(&fixture("adversarial.xlsx"), "xlsx", "Untrusted")
        .unwrap();
    assert!(projection
        .semantic_text
        .contains(r#"[XLSX sheet "Bad\u005d injected" cell B1; channel=cell]"#));
    assert!(projection
        .markdown
        .contains("&lt;img src=x onerror=alert\\(1\\)&gt;"));
    assert!(projection
        .markdown
        .contains(r#"\[click\]\(https://example.test\)"#));
    assert!(!projection.markdown.contains("<img"));
}

#[test]
fn imported_workbook_keeps_original_and_exposes_cell_in_projection() {
    use knowledge_garden::application::{AcquisitionMethod, Application, ExtractionState};
    let dir = tempfile::tempdir().unwrap();
    let mut app = Application::open(dir.path().join("collection")).unwrap();
    let input = fixture("cell-evidence.xlsx");
    let imported = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(imported.info.extraction, ExtractionState::PartialText);
    assert_eq!(
        std::fs::read(app.original_path(&imported.info.source_id).unwrap()).unwrap(),
        std::fs::read(input).unwrap()
    );
    let page = app.open_source(&imported.info.source_id).unwrap();
    assert!(page.body.contains("Sheet Field notes"));
    assert!(page.body.contains("**B1**: River Delta"));
}
