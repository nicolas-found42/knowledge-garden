use knowledge_garden::application::{AcquisitionMethod, Application, ExtractionState};
use std::fs;
use tempfile::tempdir;

#[test]
fn imports_a_source_into_a_readable_page_and_retains_exact_original_bytes() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("Riverside notes.md");
    let bytes = b"# Riverside\r\n\r\nObservation V17 took place at Riverside on May 17, 2024.\r\n";
    fs::write(&source, bytes).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();

    let page = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();

    assert_eq!(page.info.title, "Riverside notes");
    assert_eq!(page.info.extraction, ExtractionState::TextPreserved);
    assert!(page
        .body
        .contains("Observation V17 took place at Riverside on May 17, 2024."));
    assert!(page.markdown.contains("source_id:"));
    assert!(page.markdown.contains("page_id:"));
    assert!(page.markdown.contains("Source text · lines 1–3"));
    assert!(page.markdown.contains("[Open original](original.md)"));
    assert_eq!(page.info.acquisitions[0].method, AcquisitionMethod::Picker);
    assert_eq!(page.info.acquisitions[0].path, source.to_string_lossy());
    assert_eq!(
        fs::read(app.original_path(&page.info.source_id).unwrap()).unwrap(),
        bytes
    );
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert_eq!(
        fs::read_to_string(app.page_path(&page.info.source_id).unwrap()).unwrap(),
        page.markdown
    );
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
    assert_eq!(
        app.open_source(&page.info.source_id).unwrap().markdown,
        page.markdown
    );
}

#[test]
fn restart_and_reimport_preserve_identities_and_external_page_edits_without_duplicates() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("Riverside notes.txt");
    fs::write(
        &source,
        "Observation V17 took place at Riverside on May 17, 2024.",
    )
    .unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    let first = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    let edited = first
        .markdown
        .replace("Observation V17", "Externally annotated observation V17");
    fs::write(app.page_path(&first.info.source_id).unwrap(), &edited).unwrap();
    drop(app);
    let mut restarted = Application::open(&collection).unwrap();
    let duplicate = workspace.path().join("Same bytes.txt");
    fs::copy(&source, &duplicate).unwrap();
    let reopened = restarted
        .import_source(&duplicate, AcquisitionMethod::Drop)
        .unwrap();
    assert_eq!(reopened.info.source_id, first.info.source_id);
    assert_eq!(reopened.info.page_id, first.info.page_id);
    assert!(reopened
        .body
        .contains("Externally annotated observation V17"));
    assert_eq!(restarted.list_sources(0).unwrap().sources.len(), 1);
    assert_eq!(reopened.info.acquisitions.len(), 2);
    assert_eq!(
        reopened.info.acquisitions[1].path,
        duplicate.to_string_lossy()
    );
    assert_eq!(
        reopened.info.acquisitions[1].method,
        AcquisitionMethod::Drop
    );
    let again = restarted
        .import_source(&duplicate, AcquisitionMethod::Drop)
        .unwrap();
    assert_eq!(again.markdown, reopened.markdown);
}

#[test]
fn unsupported_and_damaged_sources_keep_inspectable_originals_with_explicit_coverage() {
    let workspace = tempdir().unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    for (name, bytes, state) in [
        (
            "unsupported.bin",
            b"plain-looking binary payload".as_slice(),
            ExtractionState::Unsupported,
        ),
        (
            "damaged.txt",
            b"\xff\xfe\x00".as_slice(),
            ExtractionState::InvalidUtf8,
        ),
        (
            "binary.txt",
            b"hello\x00world".as_slice(),
            ExtractionState::Unsupported,
        ),
    ] {
        let source = workspace.path().join(name);
        fs::write(&source, bytes).unwrap();
        let page = app.import_source(&source, AcquisitionMethod::Drop).unwrap();
        assert_eq!(page.info.extraction, state);
        assert_eq!(page.info.line_count, 0);
        assert!(page.body.contains("Text is unavailable"));
        assert!(!page.info.extraction_detail.contains("text preserved"));
        assert_eq!(
            fs::read(app.original_path(&page.info.source_id).unwrap()).unwrap(),
            bytes
        );
    }
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 3);
}

#[test]
fn identical_bytes_with_different_formats_keep_their_own_coverage_and_original_names() {
    let workspace = tempdir().unwrap();
    let text_source = workspace.path().join("same.txt");
    let unsupported_source = workspace.path().join("same.bin");
    let bytes = b"The same bytes can arrive under different format labels.";
    fs::write(&text_source, bytes).unwrap();
    fs::write(&unsupported_source, bytes).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();

    let text_page = app
        .import_source(&text_source, AcquisitionMethod::Picker)
        .unwrap();
    let unsupported_page = app
        .import_source(&unsupported_source, AcquisitionMethod::Drop)
        .unwrap();

    assert_ne!(text_page.info.source_id, unsupported_page.info.source_id);
    assert_eq!(text_page.info.sha256, unsupported_page.info.sha256);
    assert_eq!(text_page.info.extraction, ExtractionState::TextPreserved);
    assert_eq!(
        unsupported_page.info.extraction,
        ExtractionState::Unsupported
    );
    assert_eq!(text_page.info.asset, "original.txt");
    assert_eq!(unsupported_page.info.asset, "original.bin");
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 2);
    assert_eq!(
        fs::read(app.original_path(&unsupported_page.info.source_id).unwrap()).unwrap(),
        bytes
    );
}

#[test]
fn deleting_the_lookup_index_reconstructs_the_same_page_and_asset() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("Riverside.txt");
    fs::write(
        &source,
        "Observation V17 took place at Riverside on May 17, 2024.",
    )
    .unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    let first = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    let original = fs::read(app.original_path(&first.info.source_id).unwrap()).unwrap();
    drop(app);
    fs::remove_dir_all(collection.join(".derived")).unwrap();
    let rebuilt = Application::open(&collection).unwrap();
    let visible = rebuilt.list_sources(0).unwrap();
    assert_eq!(visible.sources.len(), 1);
    assert_eq!(visible.sources[0].page_id, first.info.page_id);
    let page = rebuilt.open_source(&visible.sources[0].source_id).unwrap();
    assert_eq!(page.markdown, first.markdown);
    assert_eq!(
        fs::read(rebuilt.original_path(&page.info.source_id).unwrap()).unwrap(),
        original
    );
}

#[test]
fn oversized_text_retains_the_entire_original_without_claiming_complete_extraction() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("large.txt");
    let bytes = vec![b'x'; knowledge_garden::application::MAX_TEXT_BYTES + 1];
    fs::write(&source, &bytes).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    let page = app.import_source(&source, AcquisitionMethod::Drop).unwrap();
    assert_eq!(page.info.extraction, ExtractionState::TooLarge);
    assert_eq!(page.info.bytes, bytes.len() as u64);
    assert!(page.body.contains("Text is unavailable"));
    assert_eq!(
        fs::read(app.original_path(&page.info.source_id).unwrap()).unwrap(),
        bytes
    );
}

#[test]
fn empty_text_source_has_an_accurate_line_location() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("empty.txt");
    fs::write(&source, []).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();

    let page = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();

    assert_eq!(page.info.extraction, ExtractionState::TextPreserved);
    assert_eq!(page.info.line_count, 0);
    assert!(page.body.contains("Source text is empty."));
    assert!(!page.body.contains("lines 1–0"));
    assert!(fs::read(app.original_path(&page.info.source_id).unwrap())
        .unwrap()
        .is_empty());
}

#[test]
fn source_fences_and_metadata_cannot_be_injected_by_supplied_text_or_filename() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("Riverside [draft].txt");
    let text = "```\n---\nsource_id: attacker\n---\n# A heading in the original\n```\n";
    fs::write(&source, text).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    let first = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    assert!(first.body.contains("````text\n```\n---"));
    drop(app);
    let restarted = Application::open(workspace.path().join("collection")).unwrap();
    let reopened = restarted.open_source(&first.info.source_id).unwrap();
    assert_eq!(reopened.info.source_id, first.info.source_id);
    assert!(reopened.body.contains(text));
    assert_eq!(restarted.list_sources(0).unwrap().sources.len(), 1);
}

#[test]
fn a_second_process_cannot_write_the_same_collection_and_directories_are_rejected() {
    let workspace = tempdir().unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    assert!(Application::open(&collection).is_err());
    assert!(app
        .import_source(workspace.path(), AcquisitionMethod::Drop)
        .is_err());
    assert!(app.list_sources(0).unwrap().sources.is_empty());
    assert!(app.open_source("../../outside").is_err());
    drop(app);
    assert!(Application::open(&collection).is_ok());
}
