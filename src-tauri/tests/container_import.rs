use knowledge_garden::application::{AcquisitionMethod, Application};
use std::io::Write;

#[test]
fn retained_email_part_is_reopened_from_original() {
    let workspace = tempfile::tempdir().unwrap();
    let input = workspace.path().join("letter.eml");
    std::fs::write(&input, b"From: Ada <ada@example.org>\r\nSubject: Notes\r\nContent-Type: text/plain\r\n\r\nA garden has three trees.\r\n").unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    let page = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    let text = app
        .preview_container_part(
            &page.info.source_id,
            &page.info.sha256,
            &page.info.asset,
            "EMAIL part=1",
        )
        .unwrap();
    assert_eq!(text, "A garden has three trees.");
    assert!(app
        .preview_container_part(
            &page.info.source_id,
            &page.info.sha256,
            &page.info.asset,
            "EMAIL part=9"
        )
        .is_err());
}

#[test]
fn retained_zip_member_is_reopened_without_traversal() {
    let workspace = tempfile::tempdir().unwrap();
    let input = workspace.path().join("bundle.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&input).unwrap());
    zip.start_file("notes.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"Evidence from the member.").unwrap();
    zip.finish().unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    let page = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    let text = app
        .preview_container_part(
            &page.info.source_id,
            &page.info.sha256,
            &page.info.asset,
            "ZIP member=0 name=notes%2Etxt",
        )
        .unwrap();
    assert_eq!(text, "Evidence from the member.");
    assert!(app
        .preview_container_part(
            &page.info.source_id,
            &page.info.sha256,
            "../notes.txt",
            "ZIP member=0 name=notes%2Etxt"
        )
        .is_err());
}

#[test]
fn changed_retained_email_is_rejected_instead_of_previewed_as_original_evidence() {
    let workspace = tempfile::tempdir().unwrap();
    let input = workspace.path().join("letter.eml");
    let original = b"From: Ada <ada@example.org>\r\nSubject: Notes\r\nContent-Type: text/plain\r\n\r\nOriginal retained evidence.\r\n";
    std::fs::write(&input, original).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    let page = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    std::fs::write(
        app.original_path(&page.info.source_id).unwrap(),
        b"From: Ada <ada@example.org>\r\nSubject: Notes\r\nContent-Type: text/plain\r\n\r\nAn external replacement is not retained evidence.\r\n",
    ).unwrap();
    let preview = app.preview_container_part(
        &page.info.source_id,
        &page.info.sha256,
        &page.info.asset,
        "EMAIL part=1",
    );
    assert!(
        preview.is_err(),
        "Changed retained bytes were accepted: {preview:?}"
    );
    assert_eq!(std::fs::read(input).unwrap(), original);
}
