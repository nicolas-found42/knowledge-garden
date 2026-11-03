use knowledge_garden::visual_inference::{LocalVisualModel, VisualError};
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

#[test]
fn local_api_is_bounded_and_returns_unverified_interpretation() {
    let _test_guard = TEST_LOCK.lock().unwrap();
    for endpoint in [
        "https://127.0.0.1:11434",
        "http://example.com",
        "http://localhost:11434",
        "http://127.0.0.1:11434/path",
        "http://user@127.0.0.1",
    ] {
        assert!(matches!(
            LocalVisualModel::new(endpoint),
            Err(VisualError::InvalidEndpoint)
        ));
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        for i in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 4096];
            loop {
                let n = stream.read(&mut chunk).unwrap();
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(end) = bytes.windows(4).position(|x| x == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|v| v.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let request = String::from_utf8(bytes).unwrap();
            let body = if i != 1 {
                assert!(request.starts_with("GET /api/tags "));
                format!(
                    r#"{{"models":[{{"name":"qwen2.5vl:7b","digest":"{}"}}]}}"#,
                    "a".repeat(64)
                )
            } else {
                assert!(request.starts_with("POST /api/generate "));
                let payload: serde_json::Value =
                    serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
                assert_eq!(payload["images"][0], "AQID");
                assert_eq!(payload["options"]["num_predict"], 256);
                r#"{"model":"qwen2.5vl:7b","response":"Fruit on a table.","done":true,"done_reason":"stop"}"#.into()
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        }
    });
    let model = LocalVisualModel::new(&endpoint).unwrap();
    assert!(matches!(
        model.interpret(&[]),
        Err(VisualError::InvalidImage)
    ));
    assert!(matches!(
        model.interpret(&vec![0; 8 * 1024 * 1024 + 1]),
        Err(VisualError::InvalidImage)
    ));
    let result = model.interpret(&[1, 2, 3]).unwrap();
    assert_eq!(result.description, "Fruit on a table.");
    assert_eq!(result.observed_revision, "a".repeat(64));
    server.join().unwrap();

    for changed_revision in [true, false] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            for i in 0..if changed_revision { 3 } else { 2 } {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 8192];
                let _ = stream.read(&mut request).unwrap();
                let body = if i == 1 {
                    if changed_revision {
                        r#"{"model":"qwen2.5vl:7b","response":"Fruit","done":true,"done_reason":"stop"}"#.to_owned()
                    } else {
                        r#"{"model":"qwen2.5vl:7b","response":"Fruit","done":true,"done_reason":"length"}"#.to_owned()
                    }
                } else {
                    let digest = if i == 2 { "b" } else { "a" }.repeat(64);
                    format!(r#"{{"models":[{{"name":"qwen2.5vl:7b","digest":"{digest}"}}]}}"#)
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        assert!(matches!(
            LocalVisualModel::new(&endpoint).unwrap().interpret(&[1]),
            Err(VisualError::InvalidResponse)
        ));
        server.join().unwrap();
    }
}

#[test]
#[ignore = "requires a running local Ollama with qwen2.5vl:7b"]
fn real_local_model_describes_repository_fruit_image() {
    let _test_guard = TEST_LOCK.lock().unwrap();
    let image = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/research/expanded-exploration/01-source-interpretation/experiments/media-lab/fruits.jpg")).unwrap();
    let result = LocalVisualModel::new("http://127.0.0.1:11434")
        .unwrap()
        .interpret(&image)
        .unwrap();
    println!("{}: {}", result.observed_revision, result.description);
    let description = result.description.to_lowercase();
    assert!(
        description.contains("fruit")
            || description.contains("orange")
            || description.contains("apple")
    );
}

#[cfg(unix)]
#[test]
#[ignore = "requires installed Ollama and qwen2.5vl:7b"]
fn managed_local_model_starts_and_stops() {
    let _test_guard = TEST_LOCK.lock().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let image = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/research/expanded-exploration/01-source-interpretation/experiments/media-lab/fruits.jpg")).unwrap();
    let model = knowledge_garden::visual_inference::ManagedVisualModel::new(
        "/usr/local/bin/ollama",
        workspace.path().join("lease"),
    );
    let result = model.interpret(&image).unwrap();
    assert!(result.description.to_lowercase().contains("fruit"));
}

#[cfg(unix)]
#[test]
fn managed_timeout_kills_server_and_child_and_lease_excludes_other_clients() {
    let _test_guard = TEST_LOCK.lock().unwrap();
    use knowledge_garden::visual_inference::ManagedVisualModel;
    use std::{
        fs,
        io::{BufRead, BufReader},
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
        time::Duration,
    };
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("server");
    let pid_file = dir.path().join("child-pid");
    let code = format!(
        r##"#!/usr/bin/python3
import http.server, json, os, subprocess, time
p = subprocess.Popen(['/bin/sleep', '60'])
open({pid:?}, 'w').write(str(p.pid))
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        b = json.dumps({{'models':[{{'name':'qwen2.5vl:7b','digest':'a'*64}}]}}).encode()
        self.send_response(200); self.send_header('Content-Length', str(len(b))); self.end_headers(); self.wfile.write(b)
    def do_POST(self):
        time.sleep(30)
http.server.HTTPServer(('127.0.0.1', int(os.environ['OLLAMA_HOST'].split(':')[-1])), Handler).serve_forever()
"##,
        pid = pid_file.to_str().unwrap()
    );
    fs::write(&script, code).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    let lease_path = dir.path().join("lease");
    let mut holder = Command::new("/usr/bin/python3")
        .arg("-c")
        .arg("import fcntl,sys,time; f=open(sys.argv[1], 'w'); fcntl.flock(f, fcntl.LOCK_EX); print('ready', flush=True); time.sleep(60)")
        .arg(&lease_path).stdout(Stdio::piped()).spawn().unwrap();
    let mut ready = String::new();
    BufReader::new(holder.stdout.take().unwrap())
        .read_line(&mut ready)
        .unwrap();
    assert_eq!(ready.trim(), "ready");
    let model = ManagedVisualModel::new(&script, &lease_path)
        .with_timeout(Duration::from_millis(200))
        .unwrap();
    assert!(matches!(model.interpret(&[1]), Err(VisualError::Busy)));
    holder.kill().unwrap();
    holder.wait().unwrap();
    assert!(matches!(
        model.interpret(&[1]),
        Err(VisualError::Unavailable)
    ));
    let pid: libc::pid_t = fs::read_to_string(pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
}

#[cfg(unix)]
#[test]
#[ignore = "requires installed Ollama and qwen2.5vl:7b"]
fn managed_real_timeout_ends_process_group() {
    let _test_guard = TEST_LOCK.lock().unwrap();
    use std::{fs, os::unix::fs::PermissionsExt, time::Duration};
    let dir = tempfile::tempdir().unwrap();
    let wrapper = dir.path().join("ollama-wrapper");
    let pid_file = dir.path().join("server-pid");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nps -o pgid= -p $$ | tr -d ' ' > '{}'\nexec /usr/local/bin/ollama \"$@\"\n",
            pid_file.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    let image = fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/research/expanded-exploration/01-source-interpretation/experiments/media-lab/fruits.jpg")).unwrap();
    let model = knowledge_garden::visual_inference::ManagedVisualModel::new(
        &wrapper,
        dir.path().join("lease"),
    )
    .with_timeout(Duration::from_secs(1))
    .unwrap();
    assert!(matches!(
        model.interpret(&image),
        Err(VisualError::Unavailable)
    ));
    let pid: libc::pid_t = fs::read_to_string(pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(unsafe { libc::kill(-pid, 0) }, -1);
}

#[cfg(unix)]
#[test]
#[ignore = "subprocess used by the crash test"]
fn managed_crash_child() {
    let Ok(dir) = std::env::var("KG_VISUAL_CRASH_TEST") else {
        return;
    };
    let image = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/research/expanded-exploration/01-source-interpretation/experiments/media-lab/fruits.jpg")).unwrap();
    let model = knowledge_garden::visual_inference::ManagedVisualModel::new(
        format!("{dir}/wrapper"),
        format!("{dir}/lease"),
    );
    let _ = model.interpret(&image);
}

#[cfg(unix)]
#[test]
#[ignore = "requires installed Ollama and qwen2.5vl:7b"]
fn managed_server_dies_when_application_crashes() {
    let _test_guard = TEST_LOCK.lock().unwrap();
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let dir = tempfile::tempdir().unwrap();
    let wrapper = dir.path().join("wrapper");
    let group_file = dir.path().join("group");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nps -o pgid= -p $$ | tr -d ' ' > '{}'\nexec /usr/local/bin/ollama \"$@\"\n",
            group_file.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    let mut app = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "managed_crash_child"])
        .env("KG_VISUAL_CRASH_TEST", dir.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(45);
    let group: libc::pid_t = loop {
        if let Ok(group) = fs::read_to_string(&group_file) {
            if let Ok(group) = group.trim().parse() {
                break group;
            }
        }
        assert!(Instant::now() < deadline, "server did not start");
        std::thread::sleep(Duration::from_millis(50));
    };
    loop {
        let output = Command::new("/bin/ps")
            .args(["-Ao", "pgid,command"])
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&output.stdout);
        if text.lines().any(|line| {
            line.trim_start().starts_with(&format!("{group} ")) && line.contains("llama-server")
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "model runner did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
    app.kill().unwrap();
    app.wait().unwrap();
    use fs2::FileExt;
    let lease = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(dir.path().join("lease"))
        .unwrap();
    if lease.try_lock_exclusive().is_ok() {
        assert_eq!(
            unsafe { libc::kill(-group, 0) },
            -1,
            "lease released before the server group exited"
        );
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if unsafe { libc::kill(-group, 0) } == -1 {
            break;
        }
        if Instant::now() >= deadline {
            unsafe {
                libc::kill(-group, libc::SIGKILL);
            }
            panic!("orphaned server group after application crash");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires installed Ollama and qwen2.5vl:7b"]
fn photo_projection_contains_qualified_real_qwen_interpretation() {
    let _test_guard = TEST_LOCK.lock().unwrap();
    use knowledge_garden::application::{AcquisitionMethod, Application};
    let dir = tempfile::tempdir().unwrap();
    let mut app = Application::open(dir.path()).unwrap();
    let path = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/research/expanded-exploration/01-source-interpretation/experiments/media-lab/fruits.jpg"));
    let page = app.import_source(path, AcquisitionMethod::Picker).unwrap();
    app.resume_due_visual_jobs().unwrap();
    let page = app.open_source(&page.info.source_id).unwrap();
    assert!(page.body.contains("may hallucinate"));
    let jobs = app.claim_due_semantic_jobs(1).unwrap();
    assert_eq!(jobs.len(), 1);
    assert!(jobs[0].source_text.contains("Qwen interpretation:"));
    assert!(jobs[0].source_text.contains("5ced39dfa4ba"));
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires local Qwen and configured Jev access"]
fn real_photo_visual_claim_becomes_located_knowledge() {
    use knowledge_garden::{
        application::{AcquisitionMethod, Application},
        providers::JevSemanticProvider,
    };
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let mut app = Application::open_with_semantic_provider(
        dir.path(),
        Arc::new(JevSemanticProvider::from_environment_and_keychain()),
    )
    .unwrap();
    let path = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/research/expanded-exploration/01-source-interpretation/experiments/media-lab/fruits.jpg"));
    let source = app.import_source(path, AcquisitionMethod::Picker).unwrap();
    app.resume_due_visual_jobs().unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&source.info.source_id).unwrap();
    assert!(
        !source.knowledge_pages.is_empty(),
        "{:?}",
        source.info.semantic_error
    );
    let pages = source
        .knowledge_pages
        .iter()
        .map(|summary| app.open_knowledge_page(&summary.page_id).unwrap().markdown)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(pages.to_lowercase().contains("orange"), "{pages}");
    assert!(pages.contains("interpretation"), "{pages}");
    assert!(pages.contains("PHOTO whole image"), "{pages}");
}
