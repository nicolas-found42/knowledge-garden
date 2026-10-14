//! Transport tests for the shared Ollama Cloud adapter. They talk to a local fake
//! server only; no test performs a live provider call or reads a real credential.
use base64::Engine;
use knowledge_garden::ollama::{
    CredentialSource, OllamaCloudClient, OllamaImage, OllamaThink, OLLAMA_CLOUD_ENDPOINT,
    OLLAMA_CLOUD_MODEL,
};
use serde_json::{json, Value};
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

const SECRET: &str = "synthetic-ollama-secret-6f1c";

#[derive(Clone)]
enum Reply {
    Json(u16, String),
    Hang(Duration),
}

struct Recorded {
    request_line: String,
    headers: Vec<(String, String)>,
    body: Value,
}

struct FakeOllama {
    base_url: String,
    requests: Arc<Mutex<Vec<Recorded>>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

fn ok_body(content: &str) -> String {
    json!({"model": OLLAMA_CLOUD_MODEL, "message": {"role": "assistant", "content": content}, "done": true})
        .to_string()
}

impl FakeOllama {
    /// Replies are served in order; the last reply repeats once the script is exhausted.
    fn start(script: Vec<Reply>) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}/api", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let worker = {
            let requests = Arc::clone(&requests);
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                let mut served = 0usize;
                while !stop.load(Ordering::Relaxed) {
                    let mut stream = match listener.accept() {
                        Ok((stream, _)) => stream,
                        Err(_) => {
                            thread::sleep(Duration::from_millis(5));
                            continue;
                        }
                    };
                    stream.set_nonblocking(false).unwrap();
                    let mut raw = Vec::new();
                    let mut chunk = [0u8; 4096];
                    let header_end = loop {
                        let size = stream.read(&mut chunk).unwrap_or(0);
                        if size == 0 {
                            break None;
                        }
                        raw.extend_from_slice(&chunk[..size]);
                        if let Some(at) = raw.windows(4).position(|part| part == b"\r\n\r\n") {
                            break Some(at + 4);
                        }
                    };
                    let Some(header_end) = header_end else {
                        continue;
                    };
                    let head = String::from_utf8_lossy(&raw[..header_end]).to_string();
                    let mut lines = head.lines();
                    let request_line = lines.next().unwrap_or_default().to_owned();
                    let headers: Vec<(String, String)> = lines
                        .filter_map(|line| line.split_once(':'))
                        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
                        .collect();
                    let length = headers
                        .iter()
                        .find(|(k, _)| k == "content-length")
                        .and_then(|(_, v)| v.parse::<usize>().ok())
                        .unwrap_or(0);
                    while raw.len() < header_end + length {
                        let size = stream.read(&mut chunk).unwrap_or(0);
                        if size == 0 {
                            break;
                        }
                        raw.extend_from_slice(&chunk[..size]);
                    }
                    let body = serde_json::from_slice(&raw[header_end..]).unwrap_or(Value::Null);
                    requests.lock().unwrap().push(Recorded {
                        request_line,
                        headers,
                        body,
                    });
                    let reply = script[served.min(script.len() - 1)].clone();
                    served += 1;
                    match reply {
                        Reply::Json(status, body) => {
                            let response = format!("HTTP/1.1 {status} Scripted\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                            let _ = stream.write_all(response.as_bytes());
                        }
                        Reply::Hang(duration) => thread::sleep(duration),
                    }
                }
            })
        };
        Self {
            base_url,
            requests,
            stop,
            worker: Some(worker),
        }
    }

    fn client(&self) -> OllamaCloudClient {
        OllamaCloudClient::with_endpoint(&self.base_url, CredentialSource::fixed(SECRET))
            .with_retry_backoff(Duration::from_millis(1))
    }

    fn request_count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

impl Drop for FakeOllama {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[test]
fn production_client_targets_the_fixed_endpoint_and_exact_model() {
    assert_eq!(OLLAMA_CLOUD_ENDPOINT, "https://ollama.com/api");
    assert_eq!(OLLAMA_CLOUD_MODEL, "deepseek-v4.1-flash");
}

#[test]
fn chat_sends_one_non_streaming_request_with_system_user_and_base64_images() {
    let server = FakeOllama::start(vec![Reply::Json(200, ok_body("red square"))]);
    let jpeg = vec![0xff, 0xd8, 0xff, 0x00, 0x10];
    let reply = server
        .client()
        .chat(
            "Describe only what is visible.",
            "What is in this image?",
            &[OllamaImage {
                mime_type: "image/jpeg".into(),
                bytes: jpeg.clone(),
            }],
        )
        .unwrap();
    assert_eq!(reply.content, "red square");
    assert_eq!(reply.model, "deepseek-v4.1-flash");

    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert!(request.request_line.starts_with("POST /api/chat "));
    let authorization = request
        .headers
        .iter()
        .find(|(k, _)| k == "authorization")
        .map(|(_, v)| v.as_str());
    assert_eq!(authorization, Some(format!("Bearer {SECRET}").as_str()));
    assert_eq!(
        request.body,
        json!({
            "model": "deepseek-v4.1-flash",
            "stream": false,
            "think": false,
            "messages": [
                {"role": "system", "content": "Describe only what is visible."},
                {"role": "user", "content": "What is in this image?",
                 "images": [base64::engine::general_purpose::STANDARD.encode(&jpeg)]}
            ]
        })
    );
}

#[test]
fn text_only_chat_omits_images_and_think_is_configurable() {
    let server = FakeOllama::start(vec![Reply::Json(200, ok_body("unknown"))]);
    server
        .client()
        .with_think(OllamaThink::High)
        .chat("system", "user", &[])
        .unwrap();
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests[0].body["think"], json!("high"));
    assert!(requests[0].body["messages"][1].get("images").is_none());
}
