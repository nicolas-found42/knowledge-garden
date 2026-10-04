use knowledge_garden::application::Application;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

const PAGE_A: &[u8] = b"<!doctype html><html><head><title>Riverside observation U5</title></head><body><h1>Riverside observation U5</h1><p>Observation U5 took place on May 17, 2024. Observer: Maya. Location: Riverside. Visits: 4.</p><p>Reference: <a href='/b'>destination report</a>.</p><p>Another reference to <a href='/b'>the same report</a>; its contents have not been supplied in this page.</p><script>NEVER INDEX SCRIPT</script></body></html>";
const PAGE_B: &[u8] = b"<!doctype html><html><head><title>Explicit destination evidence</title></head><body><h1>Explicit destination evidence</h1><p>DESTINATION-UNSEEN-91823</p><p>Observation U6. Visits: 8.</p></body></html>";
const TEXT_ASSET: &[u8] =
    b"Observation U7 took place on June 5, 2024. Observer: Noor. Location: Estuary. Visits: 6.\n";

struct Fixture {
    base_url: String,
    requests: Arc<Mutex<Vec<String>>>,
    temporary_available: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Fixture {
    fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let temporary_available = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let worker = {
            let requests = Arc::clone(&requests);
            let temporary_available = Arc::clone(&temporary_available);
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let (mut stream, _) = match listener.accept() {
                        Ok(connection) => connection,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                            continue;
                        }
                        Err(_) => break,
                    };
                    stream.set_nonblocking(false).unwrap();
                    let mut request = Vec::with_capacity(4096);
                    let mut chunk = [0; 1024];
                    while request.len() < 4096
                        && !request.windows(4).any(|part| part == b"\r\n\r\n")
                    {
                        let size = stream.read(&mut chunk).unwrap_or(0);
                        if size == 0 {
                            break;
                        }
                        request.extend_from_slice(&chunk[..size]);
                    }
                    if request.is_empty() {
                        continue;
                    }
                    let line = String::from_utf8_lossy(&request);
                    let Some(target) = line
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                    else {
                        continue;
                    };
                    let target = target.to_owned();
                    let path = reqwest::Url::parse(&target)
                        .map(|url| url.path().to_owned())
                        .unwrap_or_else(|_| target.clone());
                    requests.lock().unwrap().push(path.clone());
                    if path == "/large.bin" {
                        let size = 36 * 1024 * 1024;
                        let headers = format!("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n");
                        if stream.write_all(headers.as_bytes()).is_ok() {
                            let pattern = b"\0SYNTHETIC-LARGE-ASSET\xff".repeat(4096);
                            let mut remaining = size;
                            while remaining > 0 {
                                let chunk_size = remaining.min(pattern.len());
                                if stream.write_all(&pattern[..chunk_size]).is_err() {
                                    break;
                                }
                                remaining -= chunk_size;
                            }
                        }
                        continue;
                    }
                    let (status, content_type, disposition, body, location) = match path.as_str() {
                        "/a" => (200, "text/html; charset=utf-8", None, PAGE_A, None),
                        "/b" => (200, "text/html; charset=utf-8", None, PAGE_B, None),
                        "/redirect" => (302, "text/plain", None, b"".as_slice(), Some("/a")),
                        "/asset.txt" => (
                            200,
                            "text/plain; charset=utf-8",
                            Some("attachment; filename=field-u7.txt"),
                            TEXT_ASSET,
                            None,
                        ),
                        "/invalid.html" => (
                            200,
                            "text/html; charset=utf-8",
                            None,
                            b"<html><body>invalid \xff html</body></html>".as_slice(),
                            None,
                        ),
                        "/denied" => (
                            403,
                            "text/plain",
                            None,
                            b"Access restricted; no source document supplied.".as_slice(),
                            None,
                        ),
                        "/temporary" if !temporary_available.load(Ordering::Relaxed) => (
                            503,
                            "text/plain",
                            None,
                            b"Temporarily unavailable".as_slice(),
                            None,
                        ),
                        "/temporary" => (200, "text/plain; charset=utf-8", None, TEXT_ASSET, None),
                        "/unknown.bin" => (
                            200,
                            "application/octet-stream",
                            None,
                            b"\0UNKNOWN-CONTAINER\xff\0".as_slice(),
                            None,
                        ),
                        _ => (
                            404,
                            "text/plain",
                            None,
                            b"No fixture source".as_slice(),
                            None,
                        ),
                    };
                    let reason = match status {
                        200 => "OK",
                        302 => "Found",
                        403 => "Forbidden",
                        404 => "Not Found",
                        _ => "Service Unavailable",
                    };
                    let mut headers = format!(
                        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
                        body.len()
                    );
                    if let Some(disposition) = disposition {
                        headers.push_str(&format!("Content-Disposition: {disposition}\r\n"));
                    }
                    if let Some(location) = location {
                        headers.push_str(&format!("Location: {location}\r\n"));
                    }
                    headers.push_str("\r\n");
                    let _ = stream.write_all(headers.as_bytes());
                    let _ = stream.write_all(body);
                }
            })
        };
        Self {
            base_url,
            requests,
            temporary_available,
            stop,
            worker: Some(worker),
        }
    }

    fn requested_paths(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}

#[test]
fn explicitly_supplied_urls_retain_origins_without_crawling_and_can_be_retried() {
    let fixture = Fixture::start();
    let collection = tempfile::tempdir().unwrap();
    let mut app = Application::open(collection.path()).unwrap();

    let first = app
        .import_url(&format!("{}/a", fixture.base_url))
        .unwrap_or_else(|error| panic!("{error}; requests: {:?}", fixture.requested_paths()));
    assert_eq!(first.info.title, "Riverside observation U5");
    assert_eq!(first.info.format, "html");
    assert_eq!(
        first.info.extraction,
        knowledge_garden::application::ExtractionState::StructuredText
    );
    assert_eq!(
        first.info.acquisitions[0].requested_url.as_deref(),
        Some(format!("{}/a", fixture.base_url).as_str())
    );
    assert_eq!(
        first.info.acquisitions[0].final_url.as_deref(),
        Some(format!("{}/a", fixture.base_url).as_str())
    );
    assert_eq!(
        std::fs::read(app.original_path(&first.info.source_id).unwrap()).unwrap(),
        PAGE_A
    );
    assert!(!first.body.contains("DESTINATION-UNSEEN-91823"));
    assert!(first.body.contains(&format!("{}/b", fixture.base_url)));
    assert_eq!(
        first
            .body
            .matches(&format!("{}/b", fixture.base_url))
            .count(),
        2
    );
    assert!(!first.body.contains("NEVER INDEX SCRIPT"));
    assert_eq!(app.search_pages(Default::default()).unwrap().pages.len(), 1);
    assert!(app
        .search_pages(knowledge_garden::application::PageSearchRequest {
            query: "DESTINATION-UNSEEN-91823".into(),
            ..Default::default()
        })
        .unwrap()
        .pages
        .is_empty());
    assert_eq!(fixture.requested_paths(), vec!["/a"]);

    let destination = app.import_url(&format!("{}/b", fixture.base_url)).unwrap();
    assert!(destination.body.contains("DESTINATION-UNSEEN-91823"));
    let found = app
        .search_pages(knowledge_garden::application::PageSearchRequest {
            query: "DESTINATION-UNSEEN-91823".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        found
            .pages
            .iter()
            .map(|page| page.page_id.as_str())
            .collect::<Vec<_>>(),
        vec![destination.info.page_id.as_str()]
    );
    let by_title = app
        .search_pages(knowledge_garden::application::PageSearchRequest {
            query: "Riverside observation U5".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(by_title
        .pages
        .iter()
        .any(|page| page.page_id == first.info.page_id));
    assert_eq!(fixture.requested_paths(), vec!["/a", "/b"]);

    let duplicate = app.import_url(&format!("{}/a", fixture.base_url)).unwrap();
    assert_eq!(duplicate.info.source_id, first.info.source_id);
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 2);
    assert_eq!(duplicate.info.acquisitions.len(), 1);

    let before_redirect = fixture.requested_paths();
    let redirected = app
        .import_url(&format!("{}/redirect", fixture.base_url))
        .unwrap();
    assert_ne!(redirected.info.source_id, first.info.source_id);
    assert!(redirected
        .info
        .acquisitions
        .iter()
        .any(|acquisition| acquisition.requested_url.as_deref()
            == Some(format!("{}/redirect", fixture.base_url).as_str())
            && acquisition.final_url.as_deref()
                == Some(format!("{}/a", fixture.base_url).as_str())));
    assert!(fixture
        .requested_paths()
        .ends_with(&["/redirect".into(), "/a".into()]));
    assert_eq!(
        fixture
            .requested_paths()
            .iter()
            .filter(|path| path.as_str() == "/b")
            .count(),
        1
    );
    assert_eq!(fixture.requested_paths().len(), before_redirect.len() + 2);

    let text = app
        .import_url(&format!("{}/asset.txt", fixture.base_url))
        .unwrap();
    assert_eq!(text.info.original_name, "field-u7.txt");
    assert_eq!(
        std::fs::read(app.original_path(&text.info.source_id).unwrap()).unwrap(),
        TEXT_ASSET
    );
    assert!(text.body.contains("Observation U7"));

    let unknown = app
        .import_url(&format!("{}/unknown.bin", fixture.base_url))
        .unwrap();
    assert_eq!(
        unknown.info.extraction,
        knowledge_garden::application::ExtractionState::Unsupported
    );
    assert_eq!(
        std::fs::read(app.original_path(&unknown.info.source_id).unwrap()).unwrap(),
        b"\0UNKNOWN-CONTAINER\xff\0"
    );
    assert!(unknown.body.contains("Text is unavailable"));

    let large = app
        .import_url(&format!("{}/large.bin", fixture.base_url))
        .unwrap();
    assert_eq!(large.info.bytes, 36 * 1024 * 1024);
    assert_eq!(
        large.info.extraction,
        knowledge_garden::application::ExtractionState::Unsupported
    );
    let retained = std::fs::read(app.original_path(&large.info.source_id).unwrap()).unwrap();
    assert_eq!(retained.len(), 36 * 1024 * 1024);
    assert_eq!(
        format!("{:x}", Sha256::digest(&retained)),
        large.info.sha256
    );

    let invalid_html = app
        .import_url(&format!("{}/invalid.html", fixture.base_url))
        .unwrap();
    assert_eq!(
        invalid_html.info.extraction,
        knowledge_garden::application::ExtractionState::InvalidUtf8
    );
    assert_eq!(
        std::fs::read(app.original_path(&invalid_html.info.source_id).unwrap()).unwrap(),
        b"<html><body>invalid \xff html</body></html>"
    );

    let before_failures = app.list_sources(0).unwrap().sources.len();
    assert!(app
        .import_url(&format!("{}/denied", fixture.base_url))
        .unwrap_err()
        .to_string()
        .contains("HTTP 403"));
    assert!(app
        .import_url(&format!("{}/temporary", fixture.base_url))
        .unwrap_err()
        .to_string()
        .contains("HTTP 503"));
    assert_eq!(app.list_sources(0).unwrap().sources.len(), before_failures);
    fixture.temporary_available.store(true, Ordering::Relaxed);
    let resumed = app
        .import_url(&format!("{}/temporary", fixture.base_url))
        .unwrap();
    assert!(resumed.body.contains("Observation U7"));
    assert_ne!(resumed.info.source_id, text.info.source_id);
    assert_eq!(
        app.list_sources(0).unwrap().sources.len(),
        before_failures + 1
    );
    assert!(resumed
        .info
        .acquisitions
        .iter()
        .any(|acquisition| acquisition.requested_url.as_deref()
            == Some(format!("{}/temporary", fixture.base_url).as_str())));

    let jobs = app.claim_due_semantic_jobs(20).unwrap();
    let u5 = jobs
        .iter()
        .find(|job| job.source_id == first.info.source_id)
        .unwrap();
    assert!(u5
        .source_text
        .contains("Observation U5 took place on May 17, 2024."));
    assert!(!u5.source_text.contains("<script>"));
    assert!(!u5.source_text.contains("NEVER INDEX SCRIPT"));
    assert!(!u5.source_text.contains("DESTINATION-UNSEEN-91823"));
}
