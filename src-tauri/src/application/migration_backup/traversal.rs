//! Durable enumeration of collection members. An entry remains pending until
//! acknowledged, so interruption cannot silently skip a member.
use super::*;
use std::os::unix::fs::MetadataExt;

#[derive(Serialize, Deserialize)]
struct Frame {
    path: PathBuf,

    device: u64,
    inode: u64,
    modified: i64,
    modified_ns: i64,
}
#[derive(Serialize, Deserialize)]
struct Pending {
    path: PathBuf,
    directory: bool,
}
#[derive(Serialize, Deserialize)]
struct Cursor {
    stack: Vec<Frame>,
    pending: Option<Pending>,
}

struct Directory(*mut libc::DIR, fs::Metadata);
impl Directory {
    fn open(root: &Path, relative: &Path) -> Result<Self> {
        use rustix::fs::{openat, Mode, OFlags, CWD};
        use std::os::fd::IntoRawFd;
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let mut fd = openat(CWD, root, flags, Mode::empty()).map_err(std::io::Error::from)?;
        for part in relative.components() {
            fd = openat(&fd, part.as_os_str(), flags, Mode::empty())
                .map_err(std::io::Error::from)?;
        }
        let file: File = fd.into();
        let metadata = file.metadata()?;
        let raw = file.into_raw_fd();
        let dir = unsafe { libc::fdopendir(raw) };
        if dir.is_null() {
            unsafe {
                libc::close(raw);
            }
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(Self(dir, metadata))
    }
    fn read(&mut self) -> Result<Option<String>> {
        unsafe {
            #[cfg(target_os = "macos")]
            {
                *libc::__error() = 0;
            }
            #[cfg(target_os = "linux")]
            {
                *libc::__errno_location() = 0;
            }
            let entry = libc::readdir(self.0);
            if entry.is_null() {
                let error = std::io::Error::last_os_error();
                return if error.raw_os_error() == Some(0) {
                    Ok(None)
                } else {
                    Err(error.into())
                };
            }
            let name = std::ffi::CStr::from_ptr((*entry).d_name.as_ptr())
                .to_str()
                .map_err(|_| {
                    GardenError::Recovery(
                        "A collection entry has a non-UTF-8 name; it remains preserved.".into(),
                    )
                })?
                .to_owned();
            Ok(Some(name))
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.0);
        }
    }
}

fn frame(root: &Path, path: PathBuf) -> Result<Frame> {
    let full = root.join(&path);
    validate_real_collection_ancestors(root, &full.join("entry"))?;
    let metadata = fs::symlink_metadata(full)?;
    if !metadata.is_dir() {
        return Err(GardenError::Recovery(
            "A collection directory changed during backup.".into(),
        ));
    }
    Ok(Frame {
        path,
        device: metadata.dev(),
        inode: metadata.ino(),
        modified: metadata.mtime(),
        modified_ns: metadata.mtime_nsec(),
    })
}

fn save(root: &Path, journal: &Path, state: &Cursor) -> Result<()> {
    write_atomic(&root.join(journal), &serde_json::to_vec(state)?)
}

pub(super) fn begin(root: &Path, work_root: &Path, journal: &Path) -> Result<()> {
    validate_relative_collection_path(journal)?;
    if !journal.starts_with(".recovery/migration") {
        return Err(GardenError::Recovery(
            "Traversal journal must stay in migration recovery.".into(),
        ));
    }
    let path = work_root.join(journal);
    validate_real_collection_ancestors(work_root, &path)?;
    fs::create_dir_all(path.parent().unwrap())?;
    if fs::symlink_metadata(&path).is_ok() {
        return Err(GardenError::Recovery(
            "Traversal journal already exists.".into(),
        ));
    }
    let state = Cursor {
        stack: vec![frame(root, PathBuf::new())?],
        pending: None,
    };
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    temporary.write_all(&serde_json::to_vec(&state)?)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(&path)
        .map_err(|e| GardenError::Io(e.error))?;
    sync_directory(path.parent().unwrap())
}

fn read(root: &Path, journal: &Path) -> Result<Cursor> {
    validate_relative_collection_path(journal)?;
    if !journal.starts_with(".recovery/migration") {
        return Err(GardenError::Recovery(
            "Traversal journal must stay in migration recovery.".into(),
        ));
    }
    validate_real_collection_ancestors(root, &root.join(journal))?;
    Ok(serde_json::from_slice(&fs::read(root.join(journal))?)?)
}

pub(super) fn pending(work_root: &Path, journal: &Path) -> Result<Option<PathBuf>> {
    Ok(read(work_root, journal)?.pending.map(|entry| entry.path))
}

pub(super) struct Session {
    directory: Option<(PathBuf, Directory)>,
}
impl Session {
    pub(super) fn new() -> Self {
        Self { directory: None }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Turn {
    Entry(PathBuf, bool),
    More,
    Complete,
}

fn excluded(current: &Path, name: &str, path: &Path) -> bool {
    (current.as_os_str().is_empty() && [".derived", ".collection.lock"].contains(&name))
        || path == Path::new(".recovery/migration")
}

/// Read-only verification starts afresh after restart. Keeping one stream per
/// ancestor avoids durable `seen` markers without buffering the directory tree.
pub(super) struct MemoryTraversal {
    stack: Vec<(Frame, Directory)>,
    pending: Option<Pending>,
    failure: Option<String>,
}
impl MemoryTraversal {
    pub(super) fn new(root: &Path) -> Result<Self> {
        Ok(Self {
            stack: vec![Self::open_frame(root, PathBuf::new())?],
            pending: None,
            failure: None,
        })
    }
    fn open_frame(root: &Path, path: PathBuf) -> Result<(Frame, Directory)> {
        let expected = frame(root, path.clone())?;
        let directory = Directory::open(root, &path)?;
        let m = &directory.1;
        if (
            expected.device,
            expected.inode,
            expected.modified,
            expected.modified_ns,
        ) != (m.dev(), m.ino(), m.mtime(), m.mtime_nsec())
        {
            return Err(GardenError::Recovery(
                "Verification directory changed while opening it.".into(),
            ));
        }
        Ok((expected, directory))
    }
    pub(super) fn next(&mut self, root: &Path) -> Result<Turn> {
        if let Some(error) = &self.failure {
            return Err(GardenError::Recovery(error.clone()));
        }
        let result = self.advance(root);
        if let Err(error) = &result {
            self.failure = Some(error.to_string());
        }
        result
    }
    fn advance(&mut self, root: &Path) -> Result<Turn> {
        if let Some(item) = &self.pending {
            return Ok(Turn::Entry(item.path.clone(), item.directory));
        }
        let Some((expected, directory)) = self.stack.last_mut() else {
            return Ok(Turn::Complete);
        };
        let actual = frame(root, expected.path.clone())?;
        if (
            actual.device,
            actual.inode,
            actual.modified,
            actual.modified_ns,
        ) != (
            expected.device,
            expected.inode,
            expected.modified,
            expected.modified_ns,
        ) {
            return Err(GardenError::Recovery(
                "A collection directory changed during verification.".into(),
            ));
        }
        for _ in 0..16 {
            let Some(name) = directory.read()? else {
                self.stack.pop();
                return Ok(Turn::More);
            };
            if name == "." || name == ".." {
                continue;
            }
            let path = expected.path.join(&name);
            if excluded(&expected.path, &name, &path) {
                continue;
            }
            let metadata = fs::symlink_metadata(root.join(&path))?;
            if !metadata.is_file() && !metadata.is_dir() {
                return Err(GardenError::Recovery(
                    "Verification found a non-regular collection entry.".into(),
                ));
            }
            let directory = metadata.is_dir();
            self.pending = Some(Pending {
                path: path.clone(),
                directory,
            });
            return Ok(Turn::Entry(path, directory));
        }
        Ok(Turn::More)
    }
    pub(super) fn acknowledge(&mut self, root: &Path, descend: bool) -> Result<()> {
        let item = self
            .pending
            .as_ref()
            .ok_or_else(|| GardenError::Recovery("No verification entry is pending.".into()))?;
        if item.directory && descend {
            self.stack.push(Self::open_frame(root, item.path.clone())?);
        }
        self.pending = None;
        Ok(())
    }
}

fn marker(root: &Path, journal: &Path, path: &Path) -> Result<PathBuf> {
    validate_relative_collection_path(path)?;
    let bytes = serde_json::to_vec(path)?;
    let name = hex::encode(Sha256::digest(&bytes));
    Ok(root.join(journal).parent().unwrap().join("seen").join(name))
}
fn seen(root: &Path, journal: &Path, path: &Path) -> Result<bool> {
    let path_marker = marker(root, journal, path)?;
    match fs::symlink_metadata(&path_marker) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
        Ok(m) if m.is_file() => {
            let expected = serde_json::to_vec(path)?;
            let mut bytes = Vec::new();
            File::open(path_marker)?
                .take((expected.len() + 1) as u64)
                .read_to_end(&mut bytes)?;
            if bytes != expected {
                return Err(GardenError::Recovery(
                    "Traversal entry marker is damaged.".into(),
                ));
            }
            Ok(true)
        }
        Ok(_) => Err(GardenError::Recovery(
            "Traversal entry marker is not a file.".into(),
        )),
    }
}

/// Consume at most sixteen directory entries per turn. An in-memory directory
/// stream avoids rescanning during normal operation. After restart, entries are
/// rescanned in bounded turns and durable markers suppress duplicates.
pub(super) fn next(
    root: &Path,
    work_root: &Path,
    journal: &Path,
    session: &mut Session,
) -> Result<Turn> {
    let mut state = read(work_root, journal)?;
    if let Some(item) = state.pending.as_ref() {
        return Ok(Turn::Entry(item.path.clone(), item.directory));
    }
    let Some(current) = state.stack.last() else {
        return Ok(Turn::Complete);
    };
    let actual = frame(root, current.path.clone())?;
    if (
        actual.device,
        actual.inode,
        actual.modified,
        actual.modified_ns,
    ) != (
        current.device,
        current.inode,
        current.modified,
        current.modified_ns,
    ) {
        return Err(GardenError::Recovery("A collection directory changed during backup; the cursor and original files remain preserved.".into()));
    }
    if session
        .directory
        .as_ref()
        .is_none_or(|(path, _)| *path != current.path)
    {
        session.directory = Some((current.path.clone(), Directory::open(root, &current.path)?));
    }
    let directory = &mut session.directory.as_mut().unwrap().1;
    for _ in 0..16 {
        let Some(name) = directory.read()? else {
            state.stack.pop();
            session.directory = None;
            save(work_root, journal, &state)?;
            return Ok(Turn::More);
        };
        if name == "." || name == ".." {
            continue;
        }
        let path = current.path.join(&name);
        if excluded(&current.path, &name, &path) {
            continue;
        }
        if seen(work_root, journal, &path)? {
            continue;
        }
        let metadata = fs::symlink_metadata(root.join(&path))?;
        if !metadata.is_file() && !metadata.is_dir() {
            return Err(GardenError::Recovery(format!(
                "Cannot back up non-regular collection entry {}.",
                path.display()
            )));
        }
        state.pending = Some(Pending {
            path: path.clone(),
            directory: metadata.is_dir(),
        });
        save(work_root, journal, &state)?;
        return Ok(Turn::Entry(path, metadata.is_dir()));
    }
    Ok(Turn::More)
}

pub(super) fn acknowledge(root: &Path, work_root: &Path, journal: &Path) -> Result<()> {
    acknowledge_with_descend(root, work_root, journal, true)
}
pub(super) fn skip(root: &Path, work_root: &Path, journal: &Path) -> Result<()> {
    acknowledge_with_descend(root, work_root, journal, false)
}
fn acknowledge_with_descend(
    root: &Path,
    work_root: &Path,
    journal: &Path,
    descend: bool,
) -> Result<()> {
    let mut state = read(work_root, journal)?;
    let item = state
        .pending
        .take()
        .ok_or_else(|| GardenError::Recovery("No traversal entry is pending.".into()))?;
    let parent = state
        .stack
        .last()
        .ok_or_else(|| GardenError::Recovery("Traversal journal has no parent.".into()))?;
    if item.path.parent() != Some(parent.path.as_path()) {
        return Err(GardenError::Recovery(
            "Traversal journal contains an invalid path.".into(),
        ));
    }
    let mark = marker(work_root, journal, &item.path)?;
    fs::create_dir_all(mark.parent().unwrap())?;
    let bytes = serde_json::to_vec(&item.path)?;
    if !seen(work_root, journal, &item.path)? {
        write_atomic(&mark, &bytes)?;
    }
    if item.directory && descend {
        state.stack.push(frame(root, item.path)?);
    }
    save(work_root, journal, &state)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn memory_cursor_keeps_pending_entries_and_nested_parent_positions() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir(root.join("nested")).unwrap();
        fs::write(root.join("nested/file"), b"retained").unwrap();
        for i in 0..80 {
            fs::write(root.join(format!("item-{i}")), b"x").unwrap();
        }
        let mut cursor = MemoryTraversal::new(root).unwrap();
        let mut seen = Vec::new();
        for _ in 0..1000 {
            match cursor.next(root).unwrap() {
                Turn::Entry(path, directory) => {
                    assert_eq!(
                        cursor.next(root).unwrap(),
                        Turn::Entry(path.clone(), directory)
                    );
                    assert!(!seen.contains(&path));
                    seen.push(path);
                    cursor.acknowledge(root, true).unwrap();
                }
                Turn::More => (),
                Turn::Complete => break,
            }
        }
        assert_eq!(seen.len(), 82);
        assert!(seen.contains(&PathBuf::from("nested/file")));
        assert!(seen.contains(&PathBuf::from("item-79")));
    }
    #[test]
    fn memory_cursor_never_skips_an_error_on_a_changed_directory() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir(root.join("nested")).unwrap();
        let mut cursor = MemoryTraversal::new(root).unwrap();
        assert_eq!(
            cursor.next(root).unwrap(),
            Turn::Entry("nested".into(), true)
        );
        cursor.acknowledge(root, true).unwrap();
        fs::write(root.join("nested/new-file"), b"changed").unwrap();
        assert!(cursor.next(root).is_err());
        assert!(cursor.next(root).is_err());
    }
    #[test]
    fn cursor_repeats_unacknowledged_entry_and_resumes_nested_directories() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir(root.join("nested")).unwrap();
        fs::write(root.join("nested/file"), b"x").unwrap();
        for n in 0..80 {
            fs::write(root.join(format!("file-{n}")), b"y").unwrap();
        }
        let journal = Path::new(".recovery/migration/work/traversal.json");
        begin(root, root, journal).unwrap();
        let mut session = Session::new();
        let first = loop {
            match next(root, root, journal, &mut session).unwrap() {
                Turn::Entry(p, d) => break (p, d),
                Turn::More => (),
                Turn::Complete => panic!(),
            }
        };
        assert_eq!(
            next(root, root, journal, &mut Session::new()).unwrap(),
            Turn::Entry(first.0, first.1)
        );
        let mut seen = Vec::new();
        for turn in 0..1000 {
            if turn % 7 == 0 {
                session = Session::new();
            }
            match next(root, root, journal, &mut session).unwrap() {
                Turn::Entry(path, _) => {
                    assert!(!seen.contains(&path));
                    seen.push(path);
                    acknowledge(root, root, journal).unwrap();
                }
                Turn::More => (),
                Turn::Complete => break,
            }
        }
        assert!(seen.contains(&PathBuf::from("nested/file")));
        assert!(seen.contains(&PathBuf::from("file-79")));
        assert_eq!(
            next(root, root, journal, &mut session).unwrap(),
            Turn::Complete
        );
    }
    #[test]
    fn crash_child() {
        let Some(root) = std::env::var_os("KG_TRAVERSAL_CRASH_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let journal = Path::new(".recovery/migration/work/traversal.json");
        begin(&root, &root, journal).unwrap();
        let mut session = Session::new();
        let mut count = 0;
        loop {
            match next(&root, &root, journal, &mut session).unwrap() {
                Turn::Entry(_, _) => {
                    acknowledge(&root, &root, journal).unwrap();
                    count += 1;
                    if count == 4 {
                        std::process::exit(86);
                    }
                }
                Turn::More => (),
                Turn::Complete => panic!("not enough entries"),
            }
        }
    }
    #[test]
    fn abrupt_exit_resumes_without_skipping_members() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        for n in 0..30 {
            fs::write(root.join(format!("item-{n}")), b"x").unwrap();
        }
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::traversal::tests::crash_child",
            ])
            .env("KG_TRAVERSAL_CRASH_ROOT", root)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(86));
        let journal = Path::new(".recovery/migration/work/traversal.json");
        let mut session = Session::new();
        for _ in 0..1000 {
            match next(root, root, journal, &mut session).unwrap() {
                Turn::Entry(_, _) => acknowledge(root, root, journal).unwrap(),
                Turn::More => (),
                Turn::Complete => break,
            }
        }
        for n in 0..30 {
            assert!(seen(root, journal, &PathBuf::from(format!("item-{n}"))).unwrap());
        }
        assert_eq!(
            next(root, root, journal, &mut session).unwrap(),
            Turn::Complete
        );
    }
    #[test]
    fn changed_directory_stops_enumeration() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let journal = Path::new(".recovery/migration/work/traversal.json");
        begin(root, root, journal).unwrap();
        fs::write(root.join("new"), b"x").unwrap();
        assert!(next(root, root, journal, &mut Session::new()).is_err());
    }
}
