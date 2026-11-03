//! Incremental verification for v1 array manifests. A new session starts from
//! the beginning, so no stale completion record is trusted after interruption.
use super::*;
use std::os::unix::fs::MetadataExt;

pub(super) struct Session {
    work: tempfile::TempDir,
    phase: u8,
    offset: u64,
    after: Option<Vec<u8>>,
    manifest_identity: (u64, u64, u64, i64, i64),
    hash: legacy_hash::Session,
    tree: traversal::Session,
    verified_index: Option<verified_index::VerifiedIndex>,
}
fn identity(path: &Path) -> Result<(u64, u64, u64, i64, i64)> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() {
        return Err(GardenError::Recovery(
            "Legacy manifest is not a regular file.".into(),
        ));
    }
    Ok((m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec()))
}
impl Session {
    pub(super) fn new(saved: &Path) -> Result<Self> {
        let work = tempfile::tempdir()?;
        manifest_records::create(&work.path().join("entries.sqlite"))?;
        Ok(Self {
            work,
            phase: 0,
            offset: 0,
            after: None,
            manifest_identity: identity(&saved.join("manifest.json"))?,
            hash: legacy_hash::Session::new(),
            tree: traversal::Session::new(),
            verified_index: None,
        })
    }
    pub(super) fn capture(&mut self) -> Result<()> {
        self.verified_index = Some(verified_index::VerifiedIndex::new()?);
        Ok(())
    }
    pub(super) fn take_index(&mut self) -> Option<verified_index::VerifiedIndex> {
        self.verified_index.take()
    }
}
/// Verify one manifest record, one hash block, or one directory batch.
pub(super) fn step(saved: &Path, session: &mut Session) -> Result<bool> {
    let manifest = saved.join("manifest.json");
    if identity(&manifest)? != session.manifest_identity {
        return Err(GardenError::Recovery(
            "Legacy backup manifest changed during verification.".into(),
        ));
    }
    let files = saved.join("files");
    let db = session.work.path().join("entries.sqlite");
    match session.phase {
        0 => match legacy_manifest::next(&manifest, session.offset)? {
            Some((offset, path, digest)) => {
                if manifest_records::get(&db, &path)?.is_some() {
                    return Err(GardenError::Recovery(
                        "Legacy manifest contains a duplicate path.".into(),
                    ));
                }
                manifest_records::put(&db, &path, digest.as_deref())?;
                session.offset = offset;
                Ok(false)
            }
            None => {
                session.phase = 1;
                Ok(false)
            }
        },
        1 => {
            let Some((key, entry)) = manifest_records::next(&db, session.after.as_deref())? else {
                if let Some(index) = &session.verified_index {
                    index.seal()?;
                }
                traversal::begin(
                    &files,
                    session.work.path(),
                    Path::new(".recovery/migration/legacy-tree/tree.json"),
                )?;
                session.phase = 2;
                return Ok(false);
            };
            let m = fs::symlink_metadata(files.join(&entry.path))?;
            if let Some(expected) = entry.digest.as_deref() {
                if !m.is_file() {
                    return Err(GardenError::Recovery(
                        "Legacy backup file changed type.".into(),
                    ));
                }
                let Some(actual) = legacy_hash::step(&files, &entry.path, &mut session.hash)?
                else {
                    return Ok(false);
                };
                if actual != expected {
                    return Err(GardenError::Recovery(
                        "Legacy backup failed integrity verification.".into(),
                    ));
                }
            } else if !m.is_dir() {
                return Err(GardenError::Recovery(
                    "Legacy backup directory changed type.".into(),
                ));
            }
            if let Some(index) = &session.verified_index {
                index.put(&entry.path, entry.digest.as_deref())?;
            }
            session.after = Some(key);
            Ok(false)
        }
        2 => match traversal::next(
            &files,
            session.work.path(),
            Path::new(".recovery/migration/legacy-tree/tree.json"),
            &mut session.tree,
        )? {
            traversal::Turn::More => Ok(false),
            traversal::Turn::Complete => {
                session.phase = 3;
                Ok(true)
            }
            traversal::Turn::Entry(path, directory) => {
                let digest = if let Some(index) = &session.verified_index {
                    index.get(&path)?
                } else {
                    manifest_records::get(&db, &path)?.map(|entry| entry.digest)
                }
                .ok_or_else(|| {
                    GardenError::Recovery("Legacy backup has an unlisted member.".into())
                })?;
                if directory != digest.is_none() {
                    return Err(GardenError::Recovery(
                        "Legacy backup member changed type.".into(),
                    ));
                }
                traversal::acknowledge(
                    &files,
                    session.work.path(),
                    Path::new(".recovery/migration/legacy-tree/tree.json"),
                )?;
                Ok(false)
            }
        },
        3 => Ok(true),
        _ => Err(GardenError::Recovery(
            "Legacy verification phase is invalid.".into(),
        )),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_backup_verifies_incrementally_and_rejects_extra() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("owner"), vec![4; 140_000]).unwrap();
        let saved = backup(&root).unwrap();
        let mut session = Session::new(&saved).unwrap();
        let mut complete = false;
        for _ in 0..200 {
            if step(&saved, &mut session).unwrap() {
                complete = true;
                break;
            }
        }
        assert!(complete);
        fs::write(saved.join("files/extra"), b"unlisted").unwrap();
        let mut session = Session::new(&saved).unwrap();
        let mut rejected = false;
        for _ in 0..200 {
            match step(&saved, &mut session) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(true) => break,
                Ok(false) => (),
            }
        }
        assert!(rejected);
    }
}
