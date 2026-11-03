#![cfg(target_os = "macos")]
use knowledge_garden::mime_stream::{stream_part, PartEnd, PartEvent};
use std::io::Write;

#[test]
fn bounded_mime_stream_can_resume_after_interruption() {
    let mut input = tempfile::NamedTempFile::new().unwrap();
    input
        .write_all(b"Subject: Resume\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n")
        .unwrap();
    let mut expected = Vec::new();
    for _ in 0..230000 {
        expected.extend_from_slice(b"A garden has three trees and a pond.\r\n");
    }
    assert!(expected.len() > 8 * 1024 * 1024);
    input.write_all(&expected).unwrap();
    input.flush().unwrap();
    let mut actual = Vec::new();
    let interrupted = stream_part(input.path(), 0, 0, |event| {
        if let PartEvent::Data {
            locator,
            offset,
            bytes,
        } = event
        {
            assert_eq!(locator, "EMAIL part=1");
            assert_eq!(offset, 0);
            actual.extend(bytes);
            return Err("interrupt after checkpoint".into());
        }
        Ok(())
    });
    assert_eq!(interrupted.unwrap_err(), "interrupt after checkpoint");
    let offset = actual.len() as u64;
    assert!(offset > 0);
    let end = stream_part(input.path(), 0, offset, |event| {
        if let PartEvent::Data {
            locator,
            offset,
            bytes,
        } = event
        {
            assert_eq!(locator, "EMAIL part=1");
            assert_eq!(offset, actual.len() as u64);
            actual.extend(bytes);
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(
        end,
        PartEnd::Complete {
            locator: "EMAIL part=1".into(),
            length: expected.len() as u64
        }
    );
    assert_eq!(actual, expected);
}
