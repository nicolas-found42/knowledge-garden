//! Exercise the actual Rust HTTP adapter against authored protocol responses.
use knowledge_garden::providers::{OpenRouterSystemOneTransport, SystemOneTransport};
use knowledge_garden::semantic::ProviderAvailability;
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

fn response(status: &str, headers: &str, body: &str) -> OpenRouterSystemOneTransport {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/api/v1/systemone", listener.local_addr().unwrap());
    let response = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}", body.len());
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut bytes = [0; 4096];
        loop {
            let count = stream.read(&mut bytes).unwrap();
            if count == 0 {
                break;
            }
            request.extend_from_slice(&bytes[..count]);
            if let Some(start) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&request[..start]);
                let length = header
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if request.len() >= start + 4 + length {
                    break;
                }
            }
        }
        assert!(request.starts_with(b"POST /api/v1/systemone "));
        stream.write_all(response.as_bytes()).unwrap();
    });
    OpenRouterSystemOneTransport::with_endpoint(endpoint)
}

#[test]
fn actual_http_quota_response_remains_distinct_and_recoverable() {
    let transport = response(
        "429 Too Many Requests",
        "Retry-After: 120\r\n",
        r#"{"error":{"code":429,"message":"Authored quota fixture"}}"#,
    );
    let error = transport
        .complete(
            "synthetic-test-credential",
            &json!({"model":"jev-1.13","state":"synthetic","questions":{}}),
        )
        .unwrap_err();
    assert!(error.retryable);
    assert_eq!(error.availability, Some(ProviderAvailability::Quota));
    assert_eq!(error.retry_after_ms, Some(120_000));
}

#[test]
fn real_http_service_outages_do_not_become_exhausted_semantic_failures() {
    for (status, cause) in [
        ("401 Unauthorized", ProviderAvailability::Authentication),
        ("403 Forbidden", ProviderAvailability::CapabilityUnavailable),
        ("402 Payment Required", ProviderAvailability::Quota),
        ("404 Not Found", ProviderAvailability::ModelUnavailable),
        ("408 Request Timeout", ProviderAvailability::Network),
        (
            "500 Internal Server Error",
            ProviderAvailability::ModelUnavailable,
        ),
        ("502 Bad Gateway", ProviderAvailability::ModelUnavailable),
        (
            "504 Gateway Timeout",
            ProviderAvailability::ModelUnavailable,
        ),
        (
            "503 Service Unavailable",
            ProviderAvailability::ModelUnavailable,
        ),
    ] {
        let transport = response(status, "", r#"{"error":{"message":"fixture only"}}"#);
        let error = transport
            .complete("synthetic-test-credential", &json!({"model":"jev-1.13"}))
            .unwrap_err();
        assert!(error.retryable, "{status} must remain queued");
        assert_eq!(
            error.availability,
            Some(cause),
            "{status} must disclose its cause"
        );
    }
}

#[test]
fn successful_http_envelope_with_provider_error_is_not_a_malformed_knowledge_draft() {
    let transport = response(
        "200 OK",
        "",
        r#"{"error":{"code":503,"message":"fixture only","metadata":{"error_type":"provider_overloaded"}}}"#,
    );
    let error = transport
        .complete("synthetic-test-credential", &json!({"model":"jev-1.13"}))
        .unwrap_err();
    assert!(error.retryable);
    assert_eq!(
        error.availability,
        Some(ProviderAvailability::ModelUnavailable)
    );
}

#[test]
fn service_retry_deadline_survives_restart_and_disposable_cache_loss() {
    use knowledge_garden::{
        application::{AcquisitionMethod, Application},
        semantic::{KnowledgeDraft, ProviderError, SemanticProvider},
    };
    use std::sync::Arc;
    struct HttpOutage(OpenRouterSystemOneTransport);
    impl SemanticProvider for HttpOutage {
        fn form_knowledge(&self, _: &str) -> Result<KnowledgeDraft, ProviderError> {
            self.0
                .complete("synthetic-test-credential", &json!({"model":"jev-1.13"}))
                .map(|_| unreachable!("outage fixture cannot form knowledge"))
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let input = temp.path().join("offline.txt");
    std::fs::write(&input, "The offline orchard has a rain gauge.").unwrap();
    let provider = Arc::new(HttpOutage(response(
        "503 Service Unavailable",
        "Retry-After: 120\r\n",
        r#"{"error":{"code":503}}"#,
    )));
    let mut app = Application::open_with_semantic_provider(&root, provider).unwrap();
    let source = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    app.resume_due_semantic_jobs().unwrap();
    let pending = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(
        pending.info.provider_availability,
        Some(ProviderAvailability::ModelUnavailable)
    );
    assert_eq!(pending.info.versions_seen[0].retryable_provider_failures, 0);
    let deadline: u128 = pending
        .info
        .semantic_retry_at
        .as_ref()
        .unwrap()
        .parse()
        .unwrap();
    assert!(deadline >= before + 120_000);
    for status in [
        "500 Internal Server Error",
        "504 Gateway Timeout",
        "500 Internal Server Error",
        "504 Gateway Timeout",
    ] {
        let job = app
            .claim_due_semantic_jobs_at(1, u128::MAX)
            .unwrap()
            .remove(0);
        let error = response(
            status,
            "Retry-After: 120\r\n",
            r#"{"error":{"message":"fixture"}}"#,
        )
        .complete("synthetic-test-credential", &json!({"model":"jev-1.13"}))
        .unwrap_err();
        app.finish_semantic_job(job, Err(error)).unwrap();
        let pending = app.open_source(&source.info.source_id).unwrap();
        assert_eq!(pending.info.semantic_state, "pending");
        assert_eq!(pending.info.versions_seen[0].retryable_provider_failures, 0);
        assert!(
            pending
                .info
                .semantic_retry_at
                .unwrap()
                .parse::<u128>()
                .unwrap()
                >= before + 120_000
        );
    }
    let pending = app.open_source(&source.info.source_id).unwrap();
    let deadline: u128 = pending.info.semantic_retry_at.unwrap().parse().unwrap();
    drop(app);
    std::fs::remove_dir_all(root.join(".derived")).unwrap();
    let mut app = Application::open(&root).unwrap();
    assert!(app
        .claim_due_semantic_jobs_at(1, deadline - 1)
        .unwrap()
        .is_empty());
    let jobs = app.claim_due_semantic_jobs_at(1, deadline).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].source_id, source.info.source_id);
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
}

#[test]
fn null_error_field_in_a_successful_response_does_not_mask_data() {
    let transport = response("200 OK", "", r#"{"error":null,"data":{"result":true}}"#);
    let body = transport
        .complete("synthetic-test-credential", &json!({"model":"jev-1.13"}))
        .unwrap();
    assert_eq!(body["data"]["result"], true);
}
