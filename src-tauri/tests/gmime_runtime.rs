#![cfg(target_os = "macos")]
use std::{io::Write, path::Path, process::Command};

fn run(input: &Path, allowed: &Path, part: usize, offset: usize) -> std::process::Output {
    let exe = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("kg-mime-unit")
        .canonicalize()
        .unwrap();
    // Match the helper's actual loader paths, including a shared Cargo target
    // outside this checkout. The sandbox grants only that exact runtime tree.
    let directory = exe.parent().unwrap();
    let runtime = [
        directory.join("../Resources/container-runtime"),
        directory.join("../../resources/container-runtime"),
    ]
    .into_iter()
    .find_map(|path| path.canonicalize().ok())
    .unwrap();
    let profile = "(version 1) (deny default) (allow process-exec (literal (param \"EXECUTABLE\"))) (allow file-read* (literal \"/\") (literal (param \"INPUT\")) (subpath (param \"RUNTIME\")) (subpath \"/usr/lib\") (subpath \"/System/Library\")) (allow sysctl-read) (allow mach-lookup)";
    Command::new("/usr/bin/sandbox-exec")
        .arg("-D")
        .arg(format!(
            "INPUT={}",
            allowed.canonicalize().unwrap().display()
        ))
        .arg("-D")
        .arg(format!("EXECUTABLE={}", exe.display()))
        .arg("-D")
        .arg(format!("RUNTIME={}", runtime.display()))
        .arg("-p")
        .arg(profile)
        .arg(exe)
        .arg(input.canonicalize().unwrap())
        .arg(part.to_string())
        .arg(offset.to_string())
        .output()
        .unwrap()
}

#[test]
fn pinned_helper_streams_a_later_part_and_denies_other_files() {
    let mut email = tempfile::NamedTempFile::new().unwrap();
    write!(email, "Subject: Probe\r\nContent-Type: multipart/mixed; boundary=x\r\n\r\n--x\r\nContent-Type: application/octet-stream\r\n\r\n").unwrap();
    for _ in 0..140 {
        email.write_all(&[b'a'; 65536]).unwrap();
    }
    write!(
        email,
        "\r\n--x\r\nContent-Type: text/plain\r\n\r\nThe garden has three trees.\r\n--x--\r\n"
    )
    .unwrap();
    let result = run(email.path(), email.path(), 1, 0);
    assert!(result.status.success(), "{:?}", result);
    let output = String::from_utf8(result.stdout).unwrap();
    assert!(output.contains("DATA 1.2 0 27 VGhlIGdhcmRlbiBoYXMgdGhyZWUgdHJlZXMu"));
    assert!(output.contains("END 1.2 27"));
    let other = tempfile::NamedTempFile::new().unwrap();
    assert_eq!(run(other.path(), email.path(), 0, 0).status.code(), Some(3));
}
