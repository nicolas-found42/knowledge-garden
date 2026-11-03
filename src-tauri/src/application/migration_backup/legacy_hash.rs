//! Bounded legacy SHA-256 verification. Hash state deliberately lives only in
//! memory: after interruption the current file starts again at byte zero.
use super::*;
use std::os::unix::fs::MetadataExt;

struct Active {
    path: PathBuf,
    device: u64,
    inode: u64,
    length: u64,
    modified: i64,
    modified_ns: i64,
    offset: u64,
    hash: Sha256,
}
pub(super) struct Session {
    active: Option<Active>,
}
impl Session {
    pub(super) fn new() -> Self {
        Self { active: None }
    }
}
fn check(file: &File, active: &Active) -> Result<()> {
    let m = file.metadata()?;
    if !m.is_file()
        || (m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec())
            != (
                active.device,
                active.inode,
                active.length,
                active.modified,
                active.modified_ns,
            )
    {
        return Err(GardenError::Recovery(
            "A legacy backup member changed during verification.".into(),
        ));
    }
    Ok(())
}
/// Each call reads at most 64 KiB; recreating the session restarts the hash.
pub(super) fn step(root: &Path, path: &Path, session: &mut Session) -> Result<Option<String>> {
    use std::io::{Seek, SeekFrom};
    let mut file = bounded_copy::source_file(root, path)?;
    if session.active.as_ref().is_none_or(|a| a.path != path) {
        let m = file.metadata()?;
        if !m.is_file() {
            return Err(GardenError::Recovery(
                "A legacy backup member is not a regular file.".into(),
            ));
        }
        session.active = Some(Active {
            path: path.to_owned(),
            device: m.dev(),
            inode: m.ino(),
            length: m.len(),
            modified: m.mtime(),
            modified_ns: m.mtime_nsec(),
            offset: 0,
            hash: Sha256::new(),
        });
    }
    let active = session.active.as_mut().unwrap();
    check(&file, active)?;
    let len = (active.length - active.offset).min(64 * 1024) as usize;
    if len > 0 {
        let mut bytes = vec![0; len];
        file.seek(SeekFrom::Start(active.offset))?;
        file.read_exact(&mut bytes)?;
        check(&file, active)?;
        check(&bounded_copy::source_file(root, path)?, active)?;
        active.hash.update(bytes);
        active.offset += len as u64;
    }
    if active.offset == active.length {
        let done = session.active.take().unwrap();
        Ok(Some(hex::encode(done.hash.finalize())))
    } else {
        Ok(None)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restarting_current_file_rehashes_from_the_start() {
        let temp = tempfile::tempdir().unwrap();
        let bytes = vec![11; 200_000];
        fs::write(temp.path().join("old"), &bytes).unwrap();
        let mut session = Session::new();
        assert!(step(temp.path(), Path::new("old"), &mut session)
            .unwrap()
            .is_none());
        let mut restarted = Session::new();
        let digest = loop {
            if let Some(value) = step(temp.path(), Path::new("old"), &mut restarted).unwrap() {
                break value;
            }
        };
        assert_eq!(digest, hex::encode(Sha256::digest(bytes)));
    }
}
