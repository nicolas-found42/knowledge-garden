use std::{fs, path::Path};

pub fn assert_retains(root: &Path, expected: &[u8]) {
    fn find(directory: &Path, expected: &[u8]) -> bool {
        let Ok(entries) = fs::read_dir(directory) else {
            return false;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() && find(&entry.path(), expected) {
                return true;
            }
            if kind.is_file()
                && entry
                    .metadata()
                    .is_ok_and(|m| m.len() == expected.len() as u64)
                && fs::read(entry.path()).is_ok_and(|bytes| bytes == expected)
            {
                return true;
            }
        }
        false
    }
    assert!(
        find(&root.join(".recovery"), expected) || find(&root.join("owner-edits"), expected),
        "No recovery file retains the exact {} supplied bytes",
        expected.len()
    );
}

/// Build a historical persisted fixture, including its verified baseline. This is
/// deliberately separate from external-save tests, which must edit visible files
/// only and prove those edits cannot change active authority.
#[allow(dead_code)]
pub fn seed_metadata(path: &Path, bytes: &[u8]) {
    use sha2::{Digest, Sha256};
    let root = path
        .ancestors()
        .find(|p| p.join(".recovery/trusted-pages").is_dir())
        .unwrap();
    let relative = path.strip_prefix(root).unwrap();
    let digest = hex::encode(Sha256::digest(bytes));
    let records = root.join(".recovery/trusted-pages/records");
    let record_path = records.join(hex::encode(Sha256::digest(
        relative.to_string_lossy().as_bytes(),
    )));
    let mut record = fs::read(&record_path)
        .ok()
        .map(|b| serde_json::from_slice::<serde_json::Value>(&b).unwrap())
        .unwrap_or_else(|| serde_json::json!({"path": relative, "sha256": digest}));
    record["sha256"] = digest.clone().into();
    fs::create_dir_all(&records).unwrap();
    fs::write(
        root.join(".recovery/trusted-pages/blobs").join(digest),
        bytes,
    )
    .unwrap();
    fs::write(record_path, serde_json::to_vec(&record).unwrap()).unwrap();
    if relative.file_name().is_some_and(|name| name == "index.md") {
        let seed_path = path.parent().unwrap().join(".trusted-index");
        if let Ok(seed) = fs::read(&seed_path) {
            let mut seed: serde_json::Value = serde_json::from_slice(&seed).unwrap();
            seed["bytes"] = serde_json::to_value(bytes).unwrap();
            seed["sha256"] = hex::encode(Sha256::digest(bytes)).into();
            fs::write(seed_path, serde_json::to_vec(&seed).unwrap()).unwrap();
        }
    }
    fs::write(path, bytes).unwrap();
}
