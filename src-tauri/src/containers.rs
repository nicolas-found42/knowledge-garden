//! Bounded read-only projections of email and ZIP containers.
use crate::office::{CoveragePart, CoverageScope, CoverageStatus, OfficeProjection};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use std::{fs::File, io::Read, path::Path};
const MAX_INPUT: u64 = 8 * 1024 * 1024;
const MAX_PART: u64 = 1024 * 1024;
const MAX_PARTS: usize = 256;
fn locator(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}
fn projection(title: &str, lines: Vec<String>, mut gaps: Vec<String>) -> OfficeProjection {
    let mut selected = Vec::new();
    let mut size = 0usize;
    for line in lines {
        if size.saturating_add(line.len() + 1) > 2 * 1024 * 1024 {
            gaps.push("The extracted projection exceeded the 2 MiB limit; remaining text was not projected.".into());
            break;
        }
        size += line.len() + 1;
        selected.push(line);
    }
    let text = selected.join("\n");
    let partial = !gaps.is_empty() || text.trim().is_empty();
    let detail = if partial {
        "Container projection is partial; inspect coverage gaps."
    } else {
        "Supported container text was projected."
    }
    .to_owned();
    let mut markdown = format!(
        "# {}\n\n[Open original](ORIGINAL_ASSET)\n\n## Extracted text\n\n{}\n",
        title.replace(['\n', '\r'], " "),
        text
    );
    if partial {
        markdown.push_str(&format!(
            "\n## Extraction coverage gaps\n\n{}\n",
            gaps.iter()
                .map(|g| format!("- {g}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    let coverage = vec![CoveragePart {
        scope: CoverageScope::MainDocument,
        status: if partial {
            CoverageStatus::Partial
        } else {
            CoverageStatus::Complete
        },
        source_location: Some("container".into()),
        detail: if gaps.is_empty() && text.trim().is_empty() {
            "No readable text was found.".into()
        } else if gaps.is_empty() {
            detail.clone()
        } else {
            gaps.join("; ")
        },
    }];
    OfficeProjection {
        markdown,
        line_count: text.lines().count(),
        semantic_text: text,
        coverage,
        partial,
        detail,
    }
}
pub fn extract(path: &Path, format: &str, title: &str) -> Result<OfficeProjection, String> {
    match format {
        "eml" => email(path, title),
        "zip" => archive(path, title),
        _ => Err("Unsupported container.".into()),
    }
}
fn email(path: &Path, title: &str) -> Result<OfficeProjection, String> {
    use mailparse::MailHeaderMap;
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_INPUT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_INPUT {
        return Ok(projection(
            title,
            Vec::new(),
            vec!["Email exceeds the 8 MiB parsing limit; the original remains available.".into()],
        ));
    }
    // Each recursive multipart declaration contains this token, including folded headers.
    // Count throughout the input conservatively, so content can only lower the limit.
    let multipart_headers = bytes
        .windows(10)
        .filter(|w| w.eq_ignore_ascii_case(b"multipart/"))
        .count();
    if multipart_headers > 32 {
        return Ok(projection(title, Vec::new(), vec!["Email exceeds the multipart declaration safety limit; the original remains available.".into()]));
    }
    let parsed =
        mailparse::parse_mail(&bytes).map_err(|e| format!("Email could not be parsed: {e}"))?;
    if parsed.headers.is_empty() {
        return Err("Email has no readable message headers.".into());
    }
    let mut lines = Vec::new();
    for key in ["From", "To", "Cc", "Date", "Subject"] {
        if let Some(value) = parsed.headers.get_first_value(key) {
            lines.push(format!(
                "[EMAIL header={key}] {}",
                value.replace(['\r', '\n'], " ")
            ));
        }
    }
    let mut gaps = Vec::new();
    let header_count = lines.len();
    let mut stack = vec![("1".to_owned(), &parsed)];
    let mut count = 0;
    while let Some((part_id, part)) = stack.pop() {
        count += 1;
        if count > MAX_PARTS {
            gaps.push("Message exceeds the 256 part limit.".into());
            break;
        }
        if !part.subparts.is_empty() {
            for (i, sub) in part.subparts.iter().enumerate().rev() {
                stack.push((format!("{part_id}.{}", i + 1), sub));
            }
            continue;
        }
        let kind = part.ctype.mimetype.to_ascii_lowercase();
        if kind == "text/plain"
            && part.get_content_disposition().disposition != mailparse::DispositionType::Attachment
        {
            match part.get_body() {
                Ok(body) if body.len() as u64 <= MAX_PART => {
                    for line in body.lines() {
                        if lines.len() >= 10_000 {
                            gaps.push("Text line limit reached.".into());
                            break;
                        }
                        lines.push(format!("[EMAIL part={part_id}] {line}"));
                    }
                }
                Ok(_) => gaps.push(format!(
                    "Message part {part_id} exceeds the 1 MiB text limit."
                )),
                Err(_) => gaps.push(format!("Message part {part_id} could not be decoded.")),
            }
        } else {
            gaps.push(format!(
                "Message part {part_id} ({kind}) was retained but not interpreted."
            ));
        }
    }
    if lines.len() == header_count {
        gaps.push("No readable message body was found.".into());
    }
    Ok(projection(title, lines, gaps))
}
fn archive(path: &Path, title: &str) -> Result<OfficeProjection, String> {
    let mut zip = zip::ZipArchive::new(File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("ZIP archive could not be read: {e}"))?;
    if zip.len() > MAX_PARTS {
        return Ok(projection(
            title,
            Vec::new(),
            vec![
                "ZIP archive exceeds the 256 member safety limit; the original remains available."
                    .into(),
            ],
        ));
    }
    let mut lines = Vec::new();
    let mut gaps = Vec::new();
    let mut expanded = 0u64;
    for i in 0..zip.len() {
        let mut member = match zip.by_index(i) {
            Ok(m) => m,
            Err(e) => {
                gaps.push(format!("ZIP member {i} could not be opened: {e}"));
                continue;
            }
        };
        if member.is_dir() {
            continue;
        }
        let name = match member.name() {
            Ok(name) => name.into_owned(),
            Err(_) => {
                gaps.push(format!("ZIP member {i} has an invalid name."));
                continue;
            }
        };
        if member.enclosed_name().is_none() {
            gaps.push(format!(
                "ZIP member {i} has an unsafe path and was skipped."
            ));
            continue;
        }
        if member.size() > MAX_PART || expanded.saturating_add(member.size()) > MAX_INPUT {
            gaps.push(format!("ZIP member {i} exceeds an expansion limit."));
            continue;
        }
        expanded += member.size();
        let ext = Path::new(&name)
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(
            ext.as_str(),
            "txt"
                | "md"
                | "csv"
                | "json"
                | "jsonl"
                | "xml"
                | "yaml"
                | "yml"
                | "toml"
                | "rs"
                | "py"
                | "js"
                | "ts"
        ) {
            gaps.push(format!(
                "ZIP member {i} ({}) was retained but not interpreted.",
                locator(&name)
            ));
            continue;
        }
        let mut data = Vec::new();
        if let Err(e) = member.by_ref().take(MAX_PART + 1).read_to_end(&mut data) {
            gaps.push(format!("ZIP member {i} could not be decoded: {e}"));
            continue;
        }
        if data.len() as u64 > MAX_PART {
            gaps.push(format!("ZIP member {i} exceeds the 1 MiB limit."));
            continue;
        }
        match std::str::from_utf8(&data) {
            Ok(text) if !text.contains('\0') => {
                for line in text.lines() {
                    if lines.len() >= 10_000 {
                        gaps.push("Text line limit reached.".into());
                        break;
                    }
                    lines.push(format!("[ZIP member={i} name={}] {line}", locator(&name)));
                }
            }
            _ => gaps.push(format!("ZIP member {i} is not readable UTF-8 text.")),
        }
    }
    Ok(projection(title, lines, gaps))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn mime_parts_have_locations_and_attachments_are_partial() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, "From: A <a@example.org>\r\nSubject: Greeting\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\nHello garden\r\n--x\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=secret.bin\r\n\r\nABC\r\n--x--\r\n").unwrap();
        let p = extract(f.path(), "eml", "Mail").unwrap();
        assert!(p.semantic_text.contains("[EMAIL part=1.1] Hello garden"));
        assert!(p.partial);
        assert!(p.coverage[0].detail.contains("part 1.2"));
    }
    #[test]
    fn zip_skips_traversal_and_nested_archive() {
        let f = tempfile::NamedTempFile::new().unwrap();
        let mut writer = zip::ZipWriter::new(File::create(f.path()).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("notes/a.txt", options).unwrap();
        writer.write_all(b"Garden evidence").unwrap();
        writer.start_file("../escape.txt", options).unwrap();
        writer.write_all(b"bad").unwrap();
        writer.start_file("nested.zip", options).unwrap();
        writer.write_all(b"PK").unwrap();
        writer.finish().unwrap();
        let p = extract(f.path(), "zip", "Archive").unwrap();
        assert!(p
            .semantic_text
            .contains("[ZIP member=0 name=notes%2Fa%2Etxt] Garden evidence"));
        assert!(!p.semantic_text.contains("bad"));
        assert!(p.partial);
        assert!(p.coverage[0].detail.contains("unsafe path"));
        assert!(p.coverage[0].detail.contains("not interpreted"));
    }
    #[test]
    fn corrupt_and_empty_are_not_complete() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, "not a zip").unwrap();
        assert!(extract(f.path(), "zip", "Broken").is_err());
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, "Subject: Empty\r\n\r\n").unwrap();
        let p = extract(f.path(), "eml", "Empty").unwrap();
        assert!(
            p.partial,
            "headers alone do not constitute message body extraction"
        );
    }
    #[test]
    fn oversized_container_is_partial_not_corrupt() {
        let f = tempfile::NamedTempFile::new().unwrap();
        f.as_file().set_len(MAX_INPUT + 1).unwrap();
        for format in ["eml"] {
            let result = extract(f.path(), format, "Large").unwrap();
            assert!(result.partial);
            assert!(result.coverage[0].detail.contains("8 MiB"));
        }
    }
    #[test]
    fn excessive_projected_text_reports_a_gap() {
        let lines = (0..3).map(|_| "a".repeat(1024 * 1024)).collect();
        let result = projection("Long", lines, Vec::new());
        assert!(result.partial);
        assert!(result.coverage[0].detail.contains("2 MiB"));
        assert!(result.semantic_text.len() <= 2 * 1024 * 1024);
    }
    #[test]
    fn large_zip_processes_bounded_members() {
        let f = tempfile::NamedTempFile::new().unwrap();
        let mut writer = zip::ZipWriter::new(File::create(f.path()).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer.start_file("large.bin", options).unwrap();
        let chunk = [0u8; 65536];
        for _ in 0..129 {
            writer.write_all(&chunk).unwrap();
        }
        writer.start_file("note.txt", options).unwrap();
        writer
            .write_all(b"A later member remains available.")
            .unwrap();
        writer.finish().unwrap();
        let result = extract(f.path(), "zip", "Large archive").unwrap();
        assert!(result.partial);
        assert!(result
            .semantic_text
            .contains("A later member remains available."));
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn sandbox_denies_unrelated_file_and_network() {
        use std::process::Command;
        let allowed = tempfile::NamedTempFile::new().unwrap();
        let secret = tempfile::NamedTempFile::new().unwrap();
        let helper = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("kg-container-helper");
        let run = |path: &Path, probe: &str| {
            Command::new("/usr/bin/sandbox-exec")
            .arg("-D").arg(format!("INPUT={}", allowed.path().canonicalize().unwrap().display()))
            .arg("-D").arg(format!("EXECUTABLE={}", helper.display()))
            .args(["-p", "(version 1) (deny default) (allow process-exec (literal (param \"EXECUTABLE\"))) (allow file-read* (literal \"/\") (literal (param \"INPUT\")) (literal (param \"EXECUTABLE\")) (subpath \"/usr/lib\") (subpath \"/System/Library\")) (allow sysctl-read) (allow mach-lookup)"])
            .arg(&helper).arg("--extract-container-helper").arg(path).arg("__probe").arg(probe).output().unwrap()
        };
        let read = run(secret.path(), "read");
        assert!(read.status.success());
        assert_eq!(read.stdout, b"true");
        let network = run(allowed.path(), "network");
        assert!(network.status.success());
        assert_eq!(network.stdout, b"Some(1)");
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn helper_is_sandboxed_and_supervised() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, "Subject: Probe\r\n\r\nVisible body\r\n").unwrap();
        let p = isolated_extract(f.path(), "eml", "Probe").unwrap();
        assert!(p.semantic_text.contains("Visible body"));
        let memory = isolated_extract(f.path(), "__probe", "memory").unwrap_err();
        assert!(memory.contains("memory limit"), "{memory}");
        let timeout = isolated_extract(f.path(), "__probe", "sleep").unwrap_err();
        assert!(timeout.contains("time or output limit"), "{timeout}");
    }
}

/// The desktop executable owns the helper protocol. The helper does not run the UI.
pub fn helper() {
    let args: Vec<_> = std::env::args().collect();
    #[cfg(debug_assertions)]
    if args.len() == 5 && args[3] == "__probe" {
        match args[4].as_str() {
            "read" => print!("{}", std::fs::read(&args[2]).is_err()),
            "network" => print!(
                "{:?}",
                std::net::TcpStream::connect("127.0.0.1:9")
                    .unwrap_err()
                    .raw_os_error()
            ),
            "sleep" => std::thread::sleep(std::time::Duration::from_secs(30)),
            "memory" => {
                let mut b = vec![0u8; 600 * 1024 * 1024];
                for byte in b.iter_mut().step_by(4096) {
                    *byte = 1;
                }
                std::hint::black_box(&b);
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
            _ => {}
        }
        return;
    }
    if args.len() == 5 && args[1] == "--extract-zip-unit" {
        let parsed = args[3]
            .parse::<usize>()
            .ok()
            .zip(args[4].parse::<u64>().ok());
        match parsed.and_then(|(index, offset)| zip_unit(Path::new(&args[2]), index, offset).ok()) {
            Some(()) => return,
            None => std::process::exit(1),
        }
    }
    let result = if args.len() == 5 {
        extract(Path::new(&args[2]), &args[3], &args[4])
    } else {
        Err("Invalid helper arguments.".to_owned())
    };
    if let Ok(data) = serde_json::to_vec(&result) {
        if data.len() <= 16 * 1024 * 1024 {
            let _ = std::io::Write::write_all(&mut std::io::stdout(), &data);
        }
    }
}

pub fn isolated_extract(
    path: &Path,
    format: &str,
    title: &str,
) -> Result<OfficeProjection, String> {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let output = tempfile::tempfile().map_err(|e| e.to_string())?;
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let parent = current.parent().ok_or("Cannot locate helper directory")?;
    let executable = if parent.file_name().is_some_and(|name| name == "deps") {
        parent
            .parent()
            .ok_or("Cannot locate helper directory")?
            .join("kg-container-helper")
    } else {
        parent.join("kg-container-helper")
    };
    #[cfg(target_os = "macos")]
    let absolute = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("/usr/bin/sandbox-exec");
        command
            .arg("-D")
            .arg(format!("INPUT={}", absolute.display()));
        command
            .arg("-D")
            .arg(format!("EXECUTABLE={}", executable.display()));
        command.args(["-p", "(version 1) (deny default) (allow process-exec (literal (param \"EXECUTABLE\"))) (allow file-read* (literal \"/\") (literal (param \"INPUT\")) (literal (param \"EXECUTABLE\")) (subpath \"/usr/lib\") (subpath \"/System/Library\")) (allow sysctl-read) (allow mach-lookup)"]);
        command.arg(executable);
        command
    };
    #[cfg(not(target_os = "macos"))]
    return Err("Container extraction requires the macOS sandbox helper.".into());
    #[cfg(target_os = "macos")]
    {
        let mut child = command
            .arg("--extract-container-helper")
            .arg(&absolute)
            .arg(format)
            .arg(title)
            .stdin(Stdio::null())
            .stdout(Stdio::from(output.try_clone().map_err(|e| e.to_string())?))
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Container helper could not start: {e}"))?;
        let start = Instant::now();
        let mut last_memory_check = Instant::now() - Duration::from_secs(1);
        loop {
            if last_memory_check.elapsed() >= Duration::from_millis(200) {
                let rss = Command::new("/bin/ps")
                    .args(["-o", "rss=", "-p", &child.id().to_string()])
                    .output()
                    .ok()
                    .and_then(|out| String::from_utf8(out.stdout).ok())
                    .and_then(|value| value.trim().parse::<u64>().ok());
                if rss.is_none() {
                    match child.try_wait().map_err(|e| e.to_string())? {
                        Some(status) if status.success() => break,
                        Some(_) => return Err("Container helper failed.".into()),
                        None => {
                            let _ = child.kill();
                            let _ = child.wait();
                            return Err("Container helper memory could not be monitored.".into());
                        }
                    }
                }
                if rss.is_some_and(|kib| kib > 512 * 1024) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Container helper exceeded its memory limit.".into());
                }
                last_memory_check = Instant::now();
            }
            if output.metadata().map_err(|e| e.to_string())?.len() > 16 * 1024 * 1024
                || start.elapsed() > Duration::from_secs(15)
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Container helper exceeded its time or output limit.".into());
            }
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(status) if status.success() => break,
                Some(_) => return Err("Container helper failed.".into()),
                None => std::thread::sleep(Duration::from_millis(20)),
            }
        }
        let mut bytes = Vec::new();
        use std::io::{Seek, SeekFrom};
        let mut output = output;
        output.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        output
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("Container helper exceeded its output limit.".into());
        }
        serde_json::from_slice(&bytes)
            .map_err(|e| format!("Invalid container helper response: {e}"))?
    }
}

/// Stream one retained ZIP member as bounded decoded-byte records.
/// No record may be published as evidence until the member reaches END.
fn zip_unit(path: &Path, index: usize, offset: u64) -> Result<(), String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use std::io::Write;
    let mut zip = zip::ZipArchive::new(File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("ZIP archive could not be read: {e}"))?;
    if index >= zip.len() {
        println!("DONE");
        return Ok(());
    }
    let mut member = match zip.by_index(index) {
        Ok(member) => member,
        Err(_) => {
            println!("SKIP {index} unreadable-or-encrypted");
            return Ok(());
        }
    };
    if member.is_dir() {
        println!("SKIP {index} directory");
        return Ok(());
    }
    let name = match member.name() {
        Ok(name) => name.into_owned(),
        Err(_) => {
            println!("SKIP {index} invalid-name");
            return Ok(());
        }
    };
    if member.enclosed_name().is_none() {
        println!("SKIP {index} unsafe-path");
        return Ok(());
    }
    if member.size() > 64 * 1024 * 1024 {
        println!("SKIP {index} expansion-limit");
        return Ok(());
    }
    let extension = Path::new(&name)
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(
        extension.as_str(),
        "txt"
            | "md"
            | "csv"
            | "json"
            | "jsonl"
            | "xml"
            | "yaml"
            | "yml"
            | "toml"
            | "rs"
            | "py"
            | "js"
            | "ts"
    ) {
        println!("SKIP {index} unsupported");
        return Ok(());
    }
    if offset > member.size() {
        return Err("Decoded member offset is outside the member.".into());
    }
    let mut skip = offset;
    let mut bytes = [0u8; 65536];
    while skip > 0 {
        let length = usize::try_from(skip.min(bytes.len() as u64)).unwrap();
        let count = member
            .read(&mut bytes[..length])
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("ZIP member ended before its checkpoint.".into());
        }
        skip -= count as u64;
    }
    let mut position = offset;
    loop {
        let count = member
            .read(&mut bytes)
            .map_err(|e| format!("ZIP member could not be decoded: {e}"))?;
        if count == 0 {
            println!("END {index} {position}");
            return Ok(());
        }
        println!(
            "DATA {index} {} {position} {count} {}",
            locator(&name),
            STANDARD.encode(&bytes[..count])
        );
        std::io::stdout().flush().map_err(|e| e.to_string())?;
        position += count as u64;
    }
}
