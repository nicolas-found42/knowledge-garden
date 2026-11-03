use knowledge_garden::text_chunks::read_chunk;
use std::fs;

#[test]
fn reconstructs_large_utf8_file_from_resumable_byte_offsets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.txt");
    let original = format!(
        "{}é🙂\n{}終",
        "a".repeat(2 * 1024 * 1024 - 1),
        "z".repeat(70000)
    );
    fs::write(&path, original.as_bytes()).unwrap();
    let mut offset = 0;
    let mut reconstructed = String::new();
    loop {
        // Reopening the file each time models a process restart between chunks.
        let Some(chunk) = read_chunk(&path, offset, 65536).unwrap() else {
            break;
        };
        assert_eq!(chunk.start, offset);
        assert_eq!(
            &original.as_bytes()[chunk.start as usize..chunk.end as usize],
            chunk.text.as_bytes()
        );
        assert!(chunk.text.len() <= 65536);
        offset = chunk.end;
        reconstructed.push_str(&chunk.text);
        assert_eq!(chunk.is_last, offset == original.len() as u64);
    }
    assert_eq!(reconstructed, original);
}

#[test]
fn rejects_invalid_utf8_even_across_a_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.txt");
    fs::write(&path, b"abc\xf0\x9f").unwrap();
    let first = read_chunk(&path, 0, 4).unwrap().unwrap();
    assert_eq!(first.end, 3);
    assert_eq!(
        read_chunk(&path, first.end, 4).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
}

#[test]
fn searches_across_read_boundaries_and_overlapping_prefixes() {
    use knowledge_garden::text_chunks::contains_bytes;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.txt");
    let text = format!("{}abababac🙂", "x".repeat(65534));
    fs::write(&path, text).unwrap();
    assert!(contains_bytes(&path, "ababac🙂".as_bytes()).unwrap());
    assert!(!contains_bytes(&path, "ababac🙃".as_bytes()).unwrap());
}
