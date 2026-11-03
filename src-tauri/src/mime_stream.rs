//! Sandbox boundary for version-bound MIME part projection.
use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, PartialEq, Eq)]
pub enum PartEvent {
    Warning {
        offset: i64,
        code: i32,
    },
    Data {
        locator: String,
        offset: u64,
        bytes: Vec<u8>,
    },
}
#[derive(Debug, PartialEq, Eq)]
pub enum PartEnd {
    Complete { locator: String, length: u64 },
    Skipped { locator: String, reason: String },
    Done,
}

fn paths() -> Result<(PathBuf, PathBuf), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut directory = executable.parent().ok_or("Missing executable directory")?;
    if directory.file_name().is_some_and(|name| name == "deps") {
        directory = directory.parent().ok_or("Missing executable directory")?;
    }
    let helper = directory
        .join("kg-mime-unit")
        .canonicalize()
        .map_err(|e| format!("GMime helper unavailable: {e}"))?;
    let runtime = [
        directory.join("../Resources/container-runtime"),
        directory.join("../../resources/container-runtime"),
    ]
    .into_iter()
    .find_map(|p| p.canonicalize().ok())
    .ok_or("GMime runtime unavailable")?;
    Ok((helper, runtime))
}

/// Emits bounded decoded bytes. The caller must checkpoint source version, generation,
/// locator and offset before accepting the next record, and publish only after Complete.
pub fn stream_part(
    path: &Path,
    part: usize,
    offset: u64,
    mut emit: impl FnMut(PartEvent) -> Result<(), String>,
) -> Result<PartEnd, String> {
    #[cfg(not(target_os = "macos"))]
    return Err("GMime sandbox is unavailable on this platform.".into());
    #[cfg(target_os = "macos")]
    {
        let original = path.canonicalize().map_err(|e| e.to_string())?;
        let (helper, runtime) = paths()?;
        let profile = "(version 1) (deny default) (allow process-exec (literal (param \"EXECUTABLE\"))) (allow file-read* (literal \"/\") (literal (param \"INPUT\")) (literal (param \"EXECUTABLE\")) (subpath (param \"RUNTIME\")) (subpath \"/usr/lib\") (subpath \"/System/Library\")) (allow sysctl-read) (allow mach-lookup)";
        let mut child = Command::new("/usr/bin/sandbox-exec")
            .arg("-D")
            .arg(format!("INPUT={}", original.display()))
            .arg("-D")
            .arg(format!("EXECUTABLE={}", helper.display()))
            .arg("-D")
            .arg(format!("RUNTIME={}", runtime.display()))
            .arg("-p")
            .arg(profile)
            .arg(&helper)
            .arg(&original)
            .arg(part.to_string())
            .arg(offset.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("GMime helper could not start: {e}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or("GMime helper output unavailable")?;
        let (tx, rx) = mpsc::sync_channel::<Result<Vec<u8>, String>>(4);
        let reader = thread::spawn(move || {
            let mut source = BufReader::new(stdout);
            loop {
                let mut line = Vec::new();
                match source.by_ref().take(100_001).read_until(b'\n', &mut line) {
                    Ok(0) => break,
                    Ok(_) if line.last() == Some(&b'\n') && line.len() <= 100_000 => {
                        if tx.send(Ok(line)).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {
                        let _ = tx.send(Err("GMime helper emitted an oversized record.".into()));
                        break;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string()));
                        break;
                    }
                }
            }
        });
        let result = (|| {
            let mut expected = offset;
            let mut locator: Option<String> = None;
            let started = Instant::now();
            let mut last_memory = Instant::now() - Duration::from_secs(1);
            loop {
                if started.elapsed() > Duration::from_secs(15) {
                    return Err(
                        "GMime helper exceeded its time limit; staged work can resume.".into(),
                    );
                }
                if last_memory.elapsed() >= Duration::from_millis(200) {
                    let rss = Command::new("/bin/ps")
                        .args(["-o", "rss=", "-p", &child.id().to_string()])
                        .output()
                        .ok()
                        .and_then(|o| String::from_utf8(o.stdout).ok())
                        .and_then(|s| s.trim().parse::<u64>().ok());
                    if rss.is_some_and(|kib| kib > 512 * 1024) {
                        return Err("GMime helper exceeded its sampled memory limit.".into());
                    }
                    if rss.is_none() && child.try_wait().map_err(|e| e.to_string())?.is_none() {
                        return Err("GMime helper memory could not be monitored.".into());
                    }
                    last_memory = Instant::now();
                }
                let line = match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(line) => line?,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => return Err("GMime helper ended without a complete part.".into()),
                };
                let line = std::str::from_utf8(&line)
                    .map_err(|_| "GMime helper output was not UTF-8")?
                    .trim_end();
                let fields: Vec<_> = line.splitn(5, ' ').collect();
                match fields.as_slice() {
                    ["WARN", at, code] => emit(PartEvent::Warning {
                        offset: at.parse().map_err(|_| "Invalid warning offset")?,
                        code: code.parse().map_err(|_| "Invalid warning code")?,
                    })?,
                    ["DATA", part_path, at, len, encoded] => {
                        if !valid_path(part_path)
                            || locator.as_deref().is_some_and(|v| v != *part_path)
                        {
                            return Err("GMime part locator changed.".into());
                        }
                        let at: u64 = at.parse().map_err(|_| "Invalid decoded offset")?;
                        let len: usize = len.parse().map_err(|_| "Invalid decoded length")?;
                        let bytes = STANDARD
                            .decode(encoded)
                            .map_err(|_| "Invalid decoded data")?;
                        if at != expected || len != bytes.len() || len == 0 || len > 65536 {
                            return Err("GMime part record is inconsistent.".into());
                        }
                        expected = expected
                            .checked_add(len as u64)
                            .ok_or("Decoded offset overflow")?;
                        locator = Some((*part_path).to_owned());
                        emit(PartEvent::Data {
                            locator: format!("EMAIL part={part_path}"),
                            offset: at,
                            bytes,
                        })?;
                    }
                    ["END", part_path, length] => {
                        if !valid_path(part_path)
                            || locator.as_deref().is_some_and(|v| v != *part_path)
                            || length.parse::<u64>().ok() != Some(expected)
                        {
                            return Err("GMime part end is inconsistent.".into());
                        }
                        return Ok(PartEnd::Complete {
                            locator: format!("EMAIL part={part_path}"),
                            length: expected,
                        });
                    }
                    ["SKIP", part_path, reason] if valid_path(part_path) => {
                        return Ok(PartEnd::Skipped {
                            locator: format!("EMAIL part={part_path}"),
                            reason: (*reason).to_owned(),
                        })
                    }
                    ["DONE"] => return Ok(PartEnd::Done),
                    _ => return Err("GMime helper emitted an invalid record.".into()),
                }
            }
        })();
        drop(rx);
        let result = match result {
            Ok(value) => {
                let deadline = Instant::now() + Duration::from_secs(2);
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) if status.success() => break Ok(value),
                        Ok(Some(_)) => {
                            break Err("GMime helper failed after its final record.".into())
                        }
                        Ok(None) if Instant::now() < deadline => {
                            thread::sleep(Duration::from_millis(10))
                        }
                        _ => {
                            let _ = child.kill();
                            let _ = child.wait();
                            break Err("GMime helper did not exit after its final record.".into());
                        }
                    }
                }
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(error)
            }
        };
        let _ = reader.join();
        result
    }
}
fn valid_path(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}
