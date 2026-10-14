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
}
