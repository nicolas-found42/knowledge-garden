use knowledge_garden::{
    application::{Application, UrlAcquisitionStatus},
    providers::{JevSemanticProvider, SystemOneTransport},
    semantic::ProviderError,
};
use serde_json::Value;
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
const PAGE_C: &[u8] = b"<!doctype html><html><head><title>Updated redirected source</title></head><body><h1>Updated redirected source</h1><p>Observation U8. See <a href='/b'>current destination</a>.</p></body></html>";
const TEXT_ASSET: &[u8] =
    b"Observation U7 took place on June 5, 2024. Observer: Noor. Location: Estuary. Visits: 6.\n";

struct Fixture {
    base_url: String,
    requests: Arc<Mutex<Vec<String>>>,
    temporary_available: Arc<AtomicBool>,
    redirect_to_localhost: Arc<AtomicBool>,
    redirect_to_alias: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Fixture {
    fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let updated_redirect = format!(
            "http://localhost:{}/c",
            listener.local_addr().unwrap().port()
        );
        let alias_redirect = format!(
            "http://localhost:{}/alias",
            listener.local_addr().unwrap().port()
        );
        let requests = Arc::new(Mutex::new(Vec::new()));
        let temporary_available = Arc::new(AtomicBool::new(false));
        let redirect_to_localhost = Arc::new(AtomicBool::new(false));
        let redirect_to_alias = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let worker = {
            let requests = Arc::clone(&requests);
            let temporary_available = Arc::clone(&temporary_available);
            let redirect_to_localhost = Arc::clone(&redirect_to_localhost);
            let redirect_to_alias = Arc::clone(&redirect_to_alias);
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
                        "/c" => (200, "text/html; charset=utf-8", None, PAGE_C, None),
                        "/alias" => (200, "text/html; charset=utf-8", None, PAGE_C, None),
                        "/redirect" if redirect_to_alias.load(Ordering::Relaxed) => (
                            302,
                            "text/plain",
                            None,
                            b"".as_slice(),
                            Some(alias_redirect.as_str()),
                        ),
                        "/redirect" if redirect_to_localhost.load(Ordering::Relaxed) => (
                            302,
                            "text/plain",
                            None,
                            b"".as_slice(),
                            Some(updated_redirect.as_str()),
                        ),
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
            redirect_to_localhost,
            redirect_to_alias,
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
fn eligible_url_retry_resumes_from_persisted_queue_after_restart() {
    let fixture = Fixture::start();
    let collection = tempfile::tempdir().unwrap();
    let url = format!("{}/temporary", fixture.base_url);
    {
        let mut app = Application::open(collection.path()).unwrap();
        assert!(app.import_url(&url).is_err());
        let record = app.list_url_acquisitions().unwrap().remove(0);
        assert_eq!(record.state, "pending");
    }
    fixture.temporary_available.store(true, Ordering::Relaxed);
    let mut reopened = Application::open(collection.path()).unwrap();
    reopened.resume_due_url_acquisitions_at(u64::MAX).unwrap();
    assert!(reopened.list_url_acquisitions().unwrap().is_empty());
    assert_eq!(reopened.list_sources(0).unwrap().sources.len(), 1);
    assert_eq!(
        fixture
            .requested_paths()
            .iter()
            .filter(|path| path.as_str() == "/temporary")
            .count(),
        2
    );
}

#[test]
fn url_retry_budget_is_bounded_after_eight_total_attempts() {
    let fixture = Fixture::start();
    let collection = tempfile::tempdir().unwrap();
    let url = format!("{}/temporary", fixture.base_url);
    let mut app = Application::open(collection.path()).unwrap();
    assert!(app.import_url(&url).is_err());
    for _ in 0..7 {
        app.resume_due_url_acquisitions_at(u64::MAX).unwrap();
    }
    let status = app.list_url_acquisitions().unwrap().remove(0);
    assert_eq!(status.attempts, 8);
    assert_eq!(status.state, "failed");
    assert!(status.retry_at.is_none());
    let request_count = fixture.requested_paths().len();
    app.resume_due_url_acquisitions_at(u64::MAX).unwrap();
    assert_eq!(fixture.requested_paths().len(), request_count);
}

#[test]
fn latest_redirect_final_url_resolves_relative_links_for_source_updates() {
    let fixture = Fixture::start();
    let port = fixture.base_url.rsplit(':').next().unwrap();
    let collection = tempfile::tempdir().unwrap();
    let mut app = Application::open_with_semantic_provider(
        collection.path(),
        Arc::new(JevSemanticProvider::with_transport(
            "recorded-test-key".into(),
            Arc::new(UrlReferenceTransport),
        )),
    )
    .unwrap();
    let requested = format!("{}/redirect", fixture.base_url);
    let initial = app.import_url(&requested).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    assert!(app
        .open_source(&initial.info.source_id)
        .unwrap()
        .info
        .current_version_id
        .is_some());

    fixture.redirect_to_localhost.store(true, Ordering::Relaxed);
    let updated = app.import_url(&requested).unwrap();
    assert_eq!(updated.info.source_id, initial.info.source_id);
    let pending_id = updated.info.pending_version_id.as_deref().unwrap();
    let pending_version = updated
        .info
        .versions_seen
        .iter()
        .find(|version| version.source_version_id == pending_id)
        .unwrap();
    assert_eq!(
        updated
            .info
            .acquisitions
            .last()
            .and_then(|acquisition| acquisition.final_url.as_deref()),
        Some(format!("http://localhost:{port}/c").as_str()),
        "{:?}",
        updated.info.acquisitions
    );
    assert_eq!(
        pending_version.received_at,
        updated.info.acquisitions.last().unwrap().received_at
    );
    assert!(updated.info.acquisitions.iter().any(|acquisition| {
        acquisition.requested_url.as_deref() == Some(requested.as_str())
            && acquisition.final_url.as_deref()
                == Some(format!("http://localhost:{port}/c").as_str())
    }));
    let jobs = app.claim_due_semantic_jobs(20).unwrap();
    let update_job = jobs
        .iter()
        .find(|job| job.source_id == updated.info.source_id)
        .expect("changed redirect must schedule semantic source work");
    assert!(update_job.source_text.contains("Observation U8"));
    assert!(
        update_job
            .source_text
            .contains(&format!("http://localhost:{port}/b")),
        "{}",
        update_job.source_text
    );
    assert!(!update_job
        .source_text
        .contains(&format!("{}/b", fixture.base_url)));
}

#[test]
fn repeated_redirect_contexts_survive_identical_bytes_and_restart() {
    let fixture = Fixture::start();
    let collection = tempfile::tempdir().unwrap();
    let requested = format!("{}/redirect", fixture.base_url);
    let mut app = Application::open(collection.path()).unwrap();
    let initial = app.import_url(&requested).unwrap();
    fixture.redirect_to_localhost.store(true, Ordering::Relaxed);
    app.import_url(&requested).unwrap();
    fixture.redirect_to_alias.store(true, Ordering::Relaxed);
    let latest = app.import_url(&requested).unwrap();
    assert_eq!(latest.info.source_id, initial.info.source_id);
    assert_eq!(
        latest.info.versions_seen.len(),
        2,
        "same bytes retain one version"
    );
    drop(app);
    let app = Application::open(collection.path()).unwrap();
    let page = app.open_source(&initial.info.source_id).unwrap();
    assert_eq!(page.info.acquisitions.len(), 3);
    assert!(page.info.acquisitions.iter().any(|record| record
        .final_url
        .as_deref()
        .is_some_and(|url| url.ends_with("/c"))));
    assert!(page.info.acquisitions.iter().any(|record| record
        .final_url
        .as_deref()
        .is_some_and(|url| url.ends_with("/alias"))));
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
    let denied = app.list_url_acquisitions().unwrap();
    assert_eq!(denied.len(), 1);
    assert_eq!(denied[0].state, "restricted");
    assert_eq!(denied[0].attempts, 1);
    assert!(denied[0].retry_at.is_none());
    let not_found_error = app
        .import_url(&format!("{}/missing", fixture.base_url))
        .unwrap_err()
        .to_string();
    assert!(not_found_error.contains("HTTP 404"));
    assert!(
        !not_found_error.contains("Automatic retries exhausted"),
        "a permanent first-attempt 404 must report that retry stopped, not that retries were exhausted: {not_found_error}"
    );
    let not_found = app
        .list_url_acquisitions()
        .unwrap()
        .into_iter()
        .find(|item| item.url.ends_with("/missing"))
        .unwrap();
    assert_eq!(not_found.state, "failed");
    assert!(not_found.retry_at.is_none());
    assert!(app
        .import_url(&format!("{}/temporary", fixture.base_url))
        .unwrap_err()
        .to_string()
        .contains("HTTP 503"));
    let pending = app.list_url_acquisitions().unwrap();
    assert!(pending.iter().any(|item| item.url.ends_with("/temporary")
        && item.state == "pending"
        && item.retry_at.is_some()));
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

    fixture.temporary_available.store(false, Ordering::Relaxed);
    assert!(app
        .import_url(&format!("{}/temporary", fixture.base_url))
        .unwrap_err()
        .to_string()
        .contains("HTTP 503"));
    let retained_previous = app.open_source(&resumed.info.source_id).unwrap();
    assert!(retained_previous.body.contains("Observation U7"));
    assert!(retained_previous
        .info
        .acquisitions
        .iter()
        .any(|acquisition| acquisition.requested_url.as_deref()
            == Some(format!("{}/temporary", fixture.base_url).as_str())
            && acquisition.http_status == Some(200)));
    assert_eq!(
        app.list_sources(0).unwrap().sources.len(),
        before_failures + 1
    );

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

#[test]
fn restart_on_eighth_processing_attempt_does_not_schedule_a_ninth_request() {
    let fixture = Fixture::start();
    fixture.temporary_available.store(true, Ordering::Relaxed);
    let collection = tempfile::tempdir().unwrap();
    let url = format!("{}/temporary", fixture.base_url);
    let mut app = Application::open(collection.path()).unwrap();
    let previous = app.import_url(&url).unwrap();
    assert!(previous.body.contains("Observation U7"));
    fixture.temporary_available.store(false, Ordering::Relaxed);
    let requests_before_interruption = fixture.requested_paths().len();

    // Recreate the durable public URL-status record at the process-crash boundary:
    // the eighth HTTP request was marked processing, but no response was committed.
    let digest = format!("{:x}", Sha256::digest(url.as_bytes()));
    let status_dir = collection.path().join("url-acquisitions");
    std::fs::create_dir_all(&status_dir).unwrap();
    std::fs::write(
        status_dir.join(format!("{digest}.json")),
        serde_json::to_vec(&UrlAcquisitionStatus {
            url: url.clone(),
            attempts: 8,
            state: "processing".into(),
            retry_at: None,
            last_error: "The application stopped while retrieving this URL.".into(),
            previous_source_available: true,
        })
        .unwrap(),
    )
    .unwrap();
    drop(app);

    let mut reopened = Application::open(collection.path()).unwrap();
    assert_eq!(reopened.list_url_acquisitions().unwrap()[0].attempts, 8);
    reopened.resume_due_url_acquisitions_at(u64::MAX).unwrap();
    let status = reopened.list_url_acquisitions().unwrap().remove(0);
    assert_eq!(
        status.attempts, 8,
        "restart recovery must not exceed the lifetime request budget"
    );
    assert_eq!(status.state, "failed");
    assert!(status.retry_at.is_none());
    assert_eq!(
        fixture.requested_paths().len(),
        requests_before_interruption
    );
    let retained = reopened.open_source(&previous.info.source_id).unwrap();
    assert!(retained.body.contains("Observation U7"));
    assert!(retained
        .info
        .acquisitions
        .iter()
        .any(|item| item.http_status == Some(200)));
}

struct UrlReferenceTransport;

impl SystemOneTransport for UrlReferenceTransport {
    fn complete(
        &self,
        _api_key: &str,
        request: &Value,
    ) -> std::result::Result<Value, ProviderError> {
        let questions = request["questions"].as_object().unwrap();
        let mut answers = serde_json::Map::new();
        for (key, question) in questions {
            let answer = match question["type"].as_str().unwrap_or_default() {
                "noul" => serde_json::json!({"type":"noul","noul":0.99}),
                "choice" => {
                    let criteria = question["criteria"].as_object().unwrap();
                    let choice = match key.as_str() {
                        "source_update_role" => "unknown",
                        "source_update_evidence" => "span_0",
                        "source_order_date" | "source_order_revision" => "none",
                        _ => criteria.keys().next().map(String::as_str).unwrap_or("none"),
                    };
                    serde_json::json!({
                        "type":"choice",
                        "choice":choice,
                        "probabilities":{choice:0.99}
                    })
                }
                _ => {
                    return Err(ProviderError::recoverable(
                        "Unexpected URL fixture question type.".into(),
                    ));
                }
            };
            answers.insert(key.clone(), answer);
        }
        Ok(serde_json::json!({
            "model":"typesafe/jev-1.13-recorded",
            "answers":answers
        }))
    }
}

#[test]
fn repeated_url_references_keep_each_html_occurrence_context_in_knowledge_evidence() {
    let fixture = Fixture::start();
    let collection = tempfile::tempdir().unwrap();
    let mut app = Application::open_with_semantic_provider(
        collection.path(),
        Arc::new(JevSemanticProvider::with_transport(
            "recorded-test-key".into(),
            Arc::new(UrlReferenceTransport),
        )),
    )
    .unwrap();
    let source = app.import_url(&format!("{}/a", fixture.base_url)).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(
        source.info.semantic_state, "complete",
        "{:?}",
        source.info.semantic_error
    );
    let references = source
        .knowledge_pages
        .iter()
        .map(|summary| app.open_knowledge_page(&summary.page_id).unwrap().markdown)
        .filter(|markdown| markdown.contains("property: acquired_content"))
        .collect::<Vec<_>>();
    assert_eq!(references.len(), 1, "{references:?}");
    let first_context = format!("Reference: destination report ({}/b).", fixture.base_url);
    let second_context = format!(
        "Another reference to the same report ({}/b); its contents have not been supplied in this page.",
        fixture.base_url
    );
    let projected_text_start = source.body.find("```text\n").unwrap() + "```text\n".len();
    let projected_text = &source.body[projected_text_start..];
    let frontmatter = references[0]
        .strip_prefix("---\n")
        .unwrap()
        .split_once("\n---\n")
        .unwrap()
        .0;
    let metadata: serde_yaml_ng::Value = serde_yaml_ng::from_str(frontmatter).unwrap();
    let facts = metadata["facts"].as_sequence().unwrap();
    for context in [&first_context, &second_context] {
        let start = projected_text.find(context).unwrap();
        assert!(
            facts.iter().any(|fact| {
                fact["evidence"]["quote"].as_str() == Some(context)
                    && fact["evidence"]["byte_start"].as_u64() == Some(start as u64)
                    && fact["evidence"]["byte_end"].as_u64() == Some((start + context.len()) as u64)
            }),
            "{context}\n{references:?}"
        );
    }
    assert_eq!(
        references[0]
            .matches("  property: acquired_content")
            .count(),
        2
    );
}

#[test]
fn office_hyperlink_occurrences_are_source_located_references() {
    let fixtures =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/office/source");
    for (name, expected_context, expected_location) in [
        (
            "field-visit.docx",
            "Methods reference: unacquired methods reference",
            "DOCX paragraph",
        ),
        (
            "visit-summary.pptx",
            "Linked method: https://example.invalid/linked-method",
            "PPTX slide",
        ),
    ] {
        let workspace = tempfile::tempdir().unwrap();
        let mut app = Application::open_with_semantic_provider(
            workspace.path().join("collection"),
            Arc::new(JevSemanticProvider::with_transport(
                "recorded-test-key".into(),
                Arc::new(UrlReferenceTransport),
            )),
        )
        .unwrap();
        let imported = app
            .import_source(
                fixtures.join(name),
                knowledge_garden::application::AcquisitionMethod::Picker,
            )
            .unwrap();
        app.resume_due_semantic_jobs().unwrap();
        let processed = app.open_source(&imported.info.source_id).unwrap();
        assert_eq!(processed.info.semantic_state, "complete", "{name}");
        let reference_pages = processed
            .knowledge_pages
            .iter()
            .map(|summary| app.open_knowledge_page(&summary.page_id).unwrap().markdown)
            .filter(|markdown| markdown.contains("property: acquired_content"))
            .collect::<Vec<_>>();
        assert_eq!(reference_pages.len(), 1, "{name}: {reference_pages:?}");
        assert!(
            reference_pages[0].contains(expected_context),
            "{name}: {}",
            reference_pages[0]
        );
        assert!(
            reference_pages[0].contains("offset_basis: extracted_office_projection"),
            "{name}"
        );
        assert!(
            reference_pages[0].contains(&format!("source_location: {expected_location}")),
            "{name}"
        );
        assert_eq!(processed.info.acquisitions.len(), 1, "{name}");
        assert_eq!(
            processed.info.acquisitions[0].method,
            knowledge_garden::application::AcquisitionMethod::Picker,
            "{name}"
        );
    }
}
