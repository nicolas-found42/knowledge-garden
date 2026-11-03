//! Byte-bounded resumable fingerprinting for a backup member. This is a
//! versioned block-chain digest, not the legacy manifest's whole-file SHA-256.
use super::*;
use std::io::{Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;

pub(super) const BLOCK: usize = 64 * 1024;
pub(super) fn initial(length: u64) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"knowledge-garden-backup-v2/start\0");
    h.update(length.to_be_bytes());
    h.finalize().into()
}
pub(super) fn advance(previous: [u8; 32], offset: u64, bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"knowledge-garden-backup-v2/block\0");
    h.update(previous);
    h.update(offset.to_be_bytes());
    h.update((bytes.len() as u64).to_be_bytes());
    h.update(bytes);
    h.finalize().into()
}
pub(super) fn finish(digest: [u8; 32], length: u64) -> String {
    let mut h = Sha256::new();
    h.update(b"knowledge-garden-backup-v2/end\0");
    h.update(length.to_be_bytes());
    h.update(digest);
    hex::encode(h.finalize())
}
pub(super) fn digest_bytes(bytes: &[u8]) -> String {
    let mut digest = initial(bytes.len() as u64);
    let mut offset = 0u64;
    for block in bytes.chunks(BLOCK) {
        digest = advance(digest, offset, block);
        offset += block.len() as u64;
    }
    finish(digest, bytes.len() as u64)
}
pub(super) fn digest_file(root: &Path, path: &Path) -> Result<String> {
    digest_bounded(root, path, u64::MAX)?
        .ok_or_else(|| GardenError::Recovery("Manifest member exceeds its digest limit.".into()))
}
/// Return no digest when the opened file exceeds this turn's byte budget.
pub(super) fn digest_bounded(root: &Path, path: &Path, limit: u64) -> Result<Option<String>> {
    let mut file = bounded_copy::source_file(root, path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(GardenError::Recovery(
            "Manifest member is not a regular file.".into(),
        ));
    }
    let length = metadata.len();
    if length > limit {
        return Ok(None);
    }
    let identity = Cursor {
        path: path.to_owned(),
        device: metadata.dev(),
        inode: metadata.ino(),
        length,
        modified: metadata.mtime(),
        modified_ns: metadata.mtime_nsec(),
        offset: 0,
        digest: [0; 32],
    };
    let mut offset = 0u64;
    let mut digest = initial(length);
    let mut bytes = vec![0; BLOCK];
    while offset < length {
        let count = (length - offset).min(BLOCK as u64) as usize;
        file.read_exact(&mut bytes[..count])?;
        digest = advance(digest, offset, &bytes[..count]);
        offset += count as u64;
    }
    check(&file, &identity)?;
    check(&bounded_copy::source_file(root, path)?, &identity)?;
    Ok(Some(finish(digest, length)))
}
#[derive(Serialize, Deserialize)]
struct Cursor {
    path: PathBuf,
    device: u64,
    inode: u64,
    length: u64,
    modified: i64,
    modified_ns: i64,
    offset: u64,
    digest: [u8; 32],
}
fn check(file: &File, cursor: &Cursor) -> Result<()> {
    let m = file.metadata()?;
    if !m.is_file()
        || (m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec())
            != (
                cursor.device,
                cursor.inode,
                cursor.length,
                cursor.modified,
                cursor.modified_ns,
            )
    {
        return Err(GardenError::Recovery(
            "A backup member changed while its manifest was being built.".into(),
        ));
    }
    Ok(())
}
fn read(root: &Path, journal: &Path) -> Result<Cursor> {
    validate_relative_collection_path(journal)?;
    if !journal.starts_with(".recovery/migration") {
        return Err(GardenError::Recovery(
            "Manifest work must stay in migration recovery.".into(),
        ));
    }
    let mut bytes = Vec::new();
    bounded_copy::source_file(root, journal)?
        .take(16385)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 16384 {
        return Err(GardenError::Recovery(
            "Manifest cursor is too large.".into(),
        ));
    }
    let cursor: Cursor = serde_json::from_slice(&bytes)?;
    validate_relative_collection_path(&cursor.path)?;
    if cursor.offset > cursor.length {
        return Err(GardenError::Recovery("Manifest cursor is invalid.".into()));
    }
    Ok(cursor)
}
pub(super) fn begin(root: &Path, work_root: &Path, journal: &Path, source: &Path) -> Result<()> {
    validate_relative_collection_path(journal)?;
    if !journal.starts_with(".recovery/migration") {
        return Err(GardenError::Recovery(
            "Manifest work must stay in migration recovery.".into(),
        ));
    }
    let file = bounded_copy::source_file(root, source)?;
    let m = file.metadata()?;
    if !m.is_file() {
        return Err(GardenError::Recovery(
            "Manifest member is not a regular file.".into(),
        ));
    }
    let cursor = Cursor {
        path: source.to_owned(),
        device: m.dev(),
        inode: m.ino(),
        length: m.len(),
        modified: m.mtime(),
        modified_ns: m.mtime_nsec(),
        offset: 0,
        digest: initial(m.len()),
    };
    let path = work_root.join(journal);
    validate_real_collection_ancestors(work_root, &path)?;
    fs::create_dir_all(path.parent().unwrap())?;
    let mut tmp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    tmp.write_all(&serde_json::to_vec(&cursor)?)?;
    tmp.as_file().sync_all()?;
    tmp.persist_noclobber(&path)
        .map_err(|e| GardenError::Io(e.error))?;
    sync_directory(path.parent().unwrap())
}
/// Fixed-size blocks make the digest independent of the caller's time budget.
/// A crash can replay the last block because the cursor is atomically replaced.
pub(super) fn step(root: &Path, work_root: &Path, journal: &Path) -> Result<Option<String>> {
    let mut cursor = read(work_root, journal)?;
    let mut file = bounded_copy::source_file(root, &cursor.path)?;
    check(&file, &cursor)?;
    if cursor.offset == cursor.length {
        return Ok(Some(finish(cursor.digest, cursor.length)));
    }
    let length = (cursor.length - cursor.offset).min(BLOCK as u64) as usize;
    let mut bytes = vec![0; length];
    file.seek(SeekFrom::Start(cursor.offset))?;
    file.read_exact(&mut bytes)?;
    check(&file, &cursor)?;
    check(&bounded_copy::source_file(root, &cursor.path)?, &cursor)?;
    cursor.digest = advance(cursor.digest, cursor.offset, &bytes);
    cursor.offset += length as u64;
    write_atomic(&work_root.join(journal), &serde_json::to_vec(&cursor)?)?;
    if cursor.offset == cursor.length {
        Ok(Some(finish(cursor.digest, cursor.length)))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crash_child() {
        let Some(root) = std::env::var_os("KG_MANIFEST_CRASH_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let journal = Path::new(".recovery/migration/work/manifest.json");
        begin(&root, &root, journal, Path::new("snapshot/files/owner")).unwrap();
        assert!(step(&root, &root, journal).unwrap().is_none());
        std::process::exit(87);
    }
    #[test]
    fn interrupted_manifest_can_be_completed_and_restored() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("snapshot/files")).unwrap();
        let content = vec![41; 180_000];
        fs::write(root.join("snapshot/files/owner"), &content).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::manifest_cursor::tests::crash_child",
            ])
            .env("KG_MANIFEST_CRASH_ROOT", root)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(87));
        let journal = Path::new(".recovery/migration/work/manifest.json");
        let digest = loop {
            if let Some(value) = step(root, root, journal).unwrap() {
                break value;
            }
        };
        let manifest = ManifestV2 {
            version: 2,
            entries: vec![(PathBuf::from("owner"), Some(digest))],
        };
        fs::write(
            root.join("snapshot/manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        restore_to_new_directory(&root.join("snapshot"), &root.join("restored")).unwrap();
        assert_eq!(fs::read(root.join("restored/owner")).unwrap(), content);
    }
    #[test]
    fn resumes_across_bounded_blocks_and_detects_changed_source() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("source"), vec![7; 200_000]).unwrap();
        let journal = Path::new(".recovery/migration/work/manifest.json");
        begin(root, root, journal, Path::new("source")).unwrap();
        assert!(step(root, root, journal).unwrap().is_none());
        assert_eq!(read(root, journal).unwrap().offset, 65536);
        let result = loop {
            if let Some(digest) = step(root, root, journal).unwrap() {
                break digest;
            }
        };
        assert_eq!(step(root, root, journal).unwrap(), Some(result.clone()));
        assert_eq!(digest_file(root, Path::new("source")).unwrap(), result);
        fs::write(root.join("source"), vec![8; 200_000]).unwrap();
        assert!(step(root, root, journal).is_err());
    }
}
