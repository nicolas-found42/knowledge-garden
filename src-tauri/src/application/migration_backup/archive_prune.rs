//! Incrementally prune archived replay inputs only when an independently
//! verified pre-recovery snapshot retains identical bytes. Unknown and
//! post-snapshot files, including displaced saves and receipts, stay intact.
use super::*;
use std::os::unix::fs::MetadataExt;

#[derive(Serialize, Deserialize)]
struct Cursor {
    backup: String,
    generation: String,
    after: Option<Vec<u8>>,
    quarantine_dir: PathBuf,
}
struct Current {
    key: Vec<u8>,
    original: PathBuf,
    archived: PathBuf,
    quarantine: PathBuf,
    digest: String,
    phase: u8,
    work: Option<tempfile::TempDir>,
    identity: Option<(u64, u64, u64, i64, i64)>,
    backup_identity: Option<(u64, u64, u64, i64, i64)>,
}
pub(super) struct Session {
    root: PathBuf,
    backup: PathBuf,
    archive: PathBuf,
    cursor_path: PathBuf,
    cursor: Cursor,
    current: Option<Current>,
    manifest_identity: (u64, u64, u64, i64, i64),
}
const JOURNAL: &str = ".recovery/migration/prune/hash.json";
fn identity(m: &fs::Metadata) -> (u64, u64, u64, i64, i64) {
    (m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec())
}
fn read_file(root: &Path, path: &Path) -> Result<Option<File>> {
    match fs::symlink_metadata(root.join(path)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
        Ok(m) if m.file_type().is_file() => Ok(Some(bounded_copy::source_file(root, path)?)),
        Ok(_) => Err(GardenError::Recovery(
            "Archived publication entry is not a regular file.".into(),
        )),
    }
}
impl Session {
    pub(super) fn new(root: &Path, id: &str, generation: &str) -> Result<Option<Self>> {
        if !id.starts_with("backup-") || Path::new(id).components().count() != 1 {
            return Err(GardenError::Recovery(
                "Pruning backup identity is invalid.".into(),
            ));
        }
        if !generation.starts_with("pre-work-")
            || Path::new(generation).components().count() != 1
            || generation.len() > 128
        {
            return Err(GardenError::Recovery(
                "Pruning generation is invalid.".into(),
            ));
        }
        let backup = root.join(".recovery/migration").join(id);
        let manifest_relative = PathBuf::from(".recovery/migration")
            .join(id)
            .join("manifest.sqlite");
        match fs::symlink_metadata(root.join(&manifest_relative)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
            Ok(metadata) if metadata.file_type().is_file() => (),
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Pruning manifest is not a regular file.".into(),
                ))
            }
        }
        let manifest_identity =
            identity(&bounded_copy::source_file(root, &manifest_relative)?.metadata()?);
        let archive = root
            .join(".recovery/migration/publication-history")
            .join(id)
            .join(generation);
        match fs::symlink_metadata(&archive) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
            Ok(m) if m.file_type().is_dir() => (),
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Publication history is not a real directory.".into(),
                ))
            }
        }
        let cursor_path =
            PathBuf::from(".recovery/migration").join(format!("prune-{id}-{generation}.json"));
        let cursor = match fs::symlink_metadata(root.join(&cursor_path)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let state = Cursor {
                    backup: id.into(),
                    generation: generation.into(),
                    after: None,
                    quarantine_dir: create_quarantine(root)?,
                };
                save_cursor(root, &cursor_path, &serde_json::to_vec(&state)?)?;
                state
            }
            Err(e) => return Err(e.into()),
            Ok(m) if m.file_type().is_file() => {
                let bytes = read_bounded_nofollow(root, &cursor_path, 131072)?;
                let state: Cursor = serde_json::from_slice(&bytes)?;
                if state.backup != id || state.generation != generation {
                    return Err(GardenError::Recovery(
                        "Pruning cursor identity changed.".into(),
                    ));
                }
                state
            }
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Pruning cursor is not a regular file.".into(),
                ))
            }
        };
        validate_quarantine(root, &cursor.quarantine_dir)?;
        Ok(Some(Self {
            root: root.to_owned(),
            backup,
            archive,
            cursor_path,
            cursor,
            current: None,
            manifest_identity,
        }))
    }
    fn advance(&mut self, key: Vec<u8>) -> Result<()> {
        self.cursor.after = Some(key);
        save_cursor(
            &self.root,
            &self.cursor_path,
            &serde_json::to_vec(&self.cursor)?,
        )?;
        self.current = None;
        Ok(())
    }
    pub(super) fn step(&mut self) -> Result<bool> {
        validate_quarantine(&self.root, &self.cursor.quarantine_dir)?;
        let manifest = self
            .backup
            .strip_prefix(&self.root)
            .map_err(|_| GardenError::Recovery("Backup escaped the collection.".into()))?
            .join("manifest.sqlite");
        if identity(&bounded_copy::source_file(&self.root, &manifest)?.metadata()?)
            != self.manifest_identity
        {
            return Err(GardenError::Recovery("Pruning manifest changed.".into()));
        }
        if self.current.is_none() {
            let next = manifest_records::next(
                &self.backup.join("manifest.sqlite"),
                self.cursor.after.as_deref(),
            )?;
            if identity(&bounded_copy::source_file(&self.root, &manifest)?.metadata()?)
                != self.manifest_identity
            {
                return Err(GardenError::Recovery(
                    "Pruning manifest changed during lookup.".into(),
                ));
            }
            let Some((key, entry)) = next else {
                return Ok(true);
            };
            let Some(digest) = entry.digest else {
                self.advance(key)?;
                return Ok(false);
            };
            let Ok(archived) = entry.path.strip_prefix(".staging/transactions") else {
                self.advance(key)?;
                return Ok(false);
            };
            if archived.components().count() < 2
                || archived
                    .file_name()
                    .and_then(|v| v.to_str())
                    .is_some_and(|n| n.ends_with(".held") || n.ends_with(".done"))
            {
                self.advance(key)?;
                return Ok(false);
            }
            validate_relative_collection_path(archived)?;
            let quarantine = self
                .cursor
                .quarantine_dir
                .join(format!(".kg-prune-{}", hex::encode(Sha256::digest(&key))));
            let archive_relative = self.archive.strip_prefix(&self.root).map_err(|_| {
                GardenError::Recovery("Publication history escaped the collection.".into())
            })?;
            let original_exists =
                read_file(&self.root, &archive_relative.join(archived))?.is_some();
            let quarantine_exists = read_file(&self.root, &quarantine)?.is_some();
            if original_exists && quarantine_exists {
                return Err(GardenError::Recovery(
                    "Pruning quarantine conflicts with an archived entry.".into(),
                ));
            }
            if !original_exists && !quarantine_exists {
                self.advance(key)?;
                return Ok(false);
            }
            self.current = Some(Current {
                key,
                original: entry.path.clone(),
                archived: archived.to_owned(),
                quarantine,
                digest,
                phase: 0,
                work: None,
                identity: None,
                backup_identity: None,
            });
        }
        let current = self.current.as_mut().unwrap();
        let backup_relative = self
            .backup
            .strip_prefix(&self.root)
            .map_err(|_| GardenError::Recovery("Backup escaped the collection.".into()))?;
        let archive_relative = self.archive.strip_prefix(&self.root).map_err(|_| {
            GardenError::Recovery("Publication history escaped the collection.".into())
        })?;
        let (base, path) = match current.phase {
            0 | 3 => (
                self.root.clone(),
                backup_relative.join("files").join(&current.original),
            ),
            1 => (
                self.root.clone(),
                if read_file(&self.root, &current.quarantine)?.is_some() {
                    current.quarantine.clone()
                } else {
                    archive_relative.join(&current.archived)
                },
            ),
            2 | 4 => (self.root.clone(), current.quarantine.clone()),
            _ => return Err(GardenError::Recovery("Pruning phase is invalid.".into())),
        };
        if current.work.is_none() {
            let file = bounded_copy::source_file(&base, &path)?;
            current.identity = Some(identity(&file.metadata()?));
            let work = tempfile::tempdir()?;
            manifest_cursor::begin(&base, work.path(), Path::new(JOURNAL), &path)?;
            current.work = Some(work);
            return Ok(false);
        }
        let result = manifest_cursor::step(
            &base,
            current.work.as_ref().unwrap().path(),
            Path::new(JOURNAL),
        )?;
        let Some(actual) = result else {
            return Ok(false);
        };
        if actual != current.digest {
            return Err(GardenError::Recovery(
                "Archived publication bytes differ from the retained snapshot.".into(),
            ));
        }
        let opened = bounded_copy::source_file(&base, &path)?;
        if Some(identity(&opened.metadata()?)) != current.identity {
            return Err(GardenError::Recovery(
                "Archived publication changed during pruning.".into(),
            ));
        }
        current.work = None;
        if current.phase == 0 {
            current.phase = 1;
            return Ok(false);
        }
        if current.phase == 1 && path == archive_relative.join(&current.archived) {
            move_to_quarantine(
                &self.root,
                &archive_relative.join(&current.archived),
                &current.quarantine,
            )?;
            #[cfg(test)]
            if std::env::var_os("KG_PRUNE_STOP_AFTER_QUARANTINE").is_some() {
                std::process::exit(85);
            }
            current.phase = 2;
            return Ok(false);
        }
        if current.phase == 1 || current.phase == 2 {
            current.phase = 3;
            return Ok(false);
        }
        if current.phase == 3 {
            current.backup_identity = current.identity;
            current.phase = 4;
            return Ok(false);
        }
        let retained = bounded_copy::source_file(
            &self.root,
            &backup_relative.join("files").join(&current.original),
        )?;
        if Some(identity(&retained.metadata()?)) != current.backup_identity {
            return Err(GardenError::Recovery(
                "Retained snapshot changed before pruning completed.".into(),
            ));
        }
        remove_quarantine(&self.root, &current.quarantine, current.identity.unwrap())?;
        let key = current.key.clone();
        self.advance(key)?;
        Ok(false)
    }
}
fn save_cursor(root: &Path, path: &Path, bytes: &[u8]) -> Result<()> {
    use rustix::fs::{openat, renameat_with, Mode, OFlags, RenameFlags};
    let (parent, name) = parent_fd(root, path)?;
    let temporary = format!(".prune-cursor-{}", Uuid::new_v4());
    let fd = openat(
        &parent,
        temporary.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_bits_truncate(0o600),
    )
    .map_err(std::io::Error::from)?;
    let mut file = File::from(fd);
    file.write_all(bytes)?;
    file.sync_all()?;
    renameat_with(
        &parent,
        temporary.as_str(),
        &parent,
        &name,
        RenameFlags::empty(),
    )
    .map_err(std::io::Error::from)?;
    File::from(parent).sync_all()?;
    Ok(())
}
fn create_quarantine(root: &Path) -> Result<PathBuf> {
    use rustix::fs::{mkdirat, openat, Mode, OFlags};
    use rustix::io::Errno;
    let parent_path = Path::new(".recovery/migration/prune-quarantine");
    let (migration, name) = parent_fd(root, parent_path)?;
    match mkdirat(&migration, &name, Mode::from_bits_truncate(0o700)) {
        Ok(()) | Err(Errno::EXIST) => (),
        Err(error) => return Err(std::io::Error::from(error).into()),
    }
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let parent = openat(&migration, &name, flags, Mode::empty()).map_err(std::io::Error::from)?;
    let id = Uuid::new_v4().to_string();
    mkdirat(&parent, id.as_str(), Mode::from_bits_truncate(0o700)).map_err(std::io::Error::from)?;
    File::from(parent).sync_all()?;
    File::from(migration).sync_all()?;
    Ok(parent_path.join(id))
}
fn validate_quarantine(root: &Path, path: &Path) -> Result<()> {
    let base = Path::new(".recovery/migration/prune-quarantine");
    let id = path
        .strip_prefix(base)
        .map_err(|_| GardenError::Recovery("Pruning quarantine path is invalid.".into()))?;
    if id.components().count() != 1
        || id
            .to_str()
            .and_then(|value| Uuid::parse_str(value).ok())
            .is_none()
    {
        return Err(GardenError::Recovery(
            "Pruning quarantine identity is invalid.".into(),
        ));
    }
    let (directory, _) = parent_fd(root, &path.join("entry"))?;
    let metadata = File::from(directory).metadata()?;
    if metadata.mode() & 0o777 != 0o700 || metadata.uid() != unsafe { libc::geteuid() } {
        return Err(GardenError::Recovery(
            "Pruning quarantine is not private.".into(),
        ));
    }
    Ok(())
}
fn parent_fd(root: &Path, relative: &Path) -> Result<(rustix::fd::OwnedFd, std::ffi::OsString)> {
    use rustix::fs::{openat, Mode, OFlags, CWD};
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut fd = openat(CWD, root, flags, Mode::empty()).map_err(std::io::Error::from)?;
    let parent = relative
        .parent()
        .ok_or_else(|| GardenError::Recovery("Pruning path has no parent.".into()))?;
    for part in parent.components() {
        fd = openat(&fd, part.as_os_str(), flags, Mode::empty()).map_err(std::io::Error::from)?;
    }
    Ok((fd, relative.file_name().unwrap().to_owned()))
}
fn move_to_quarantine(root: &Path, from: &Path, to: &Path) -> Result<()> {
    use rustix::fs::{renameat_with, RenameFlags};
    let (source_parent, name) = parent_fd(root, from)?;
    let (target_parent, target_name) = parent_fd(root, to)?;
    renameat_with(
        &source_parent,
        &name,
        &target_parent,
        &target_name,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    File::from(source_parent).sync_all()?;
    File::from(target_parent).sync_all()?;
    Ok(())
}
fn remove_quarantine(root: &Path, path: &Path, expected: (u64, u64, u64, i64, i64)) -> Result<()> {
    use rustix::fs::{openat, unlinkat, AtFlags, Mode, OFlags};
    let (parent, name) = parent_fd(root, path)?;
    let fd = openat(
        &parent,
        &name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)?;
    let file = File::from(fd);
    if !file.metadata()?.is_file() || identity(&file.metadata()?) != expected {
        return Err(GardenError::Recovery(
            "Pruning quarantine was replaced.".into(),
        ));
    }
    unlinkat(&parent, &name, AtFlags::empty()).map_err(std::io::Error::from)?;
    File::from(parent).sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinct_replay_generations_prune_against_the_same_backup() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let transaction = root.join(".staging/transactions/pending");
        fs::create_dir_all(transaction.join("files")).unwrap();
        fs::create_dir_all(root.join("notes")).unwrap();
        fs::write(transaction.join("files/0000"), b"planned").unwrap();
        let manifest = serde_json::to_vec(&serde_json::json!({"entries":[{"destination":"notes/page","staged":"files/0000","expected":null}],"removals":[]})).unwrap();
        fs::write(transaction.join("manifest.json"), &manifest).unwrap();
        let work = Path::new(".recovery/migration/shared-prune-snapshot");
        snapshot_worker::begin(&root, work).unwrap();
        let mut worker = snapshot_worker::Session::new();
        let saved = loop {
            if let Some(saved) = snapshot_worker::step(&root, work, &mut worker).unwrap() {
                break saved;
            }
        };
        let id = saved.file_name().unwrap().to_str().unwrap();
        fs::write(
            root.join(".recovery/migration/pre-recovery.json"),
            serde_json::to_vec(id).unwrap(),
        )
        .unwrap();
        for (index, generation) in ["pre-work-first", "pre-work-second"]
            .into_iter()
            .enumerate()
        {
            if index != 0 {
                fs::create_dir_all(transaction.join("files")).unwrap();
                fs::write(transaction.join("manifest.json"), &manifest).unwrap();
                fs::write(transaction.join("files/0000"), b"planned").unwrap();
            }
            let mut replay =
                crate::application::publication::RecoverySession::for_generation(generation)
                    .unwrap();
            let mut replayed = false;
            for _ in 0..1000 {
                if replay.step(&root).unwrap() {
                    replayed = true;
                    break;
                }
            }
            assert!(replayed);
            let mut prune = Session::new(&root, id, generation).unwrap().unwrap();
            let mut pruned = false;
            for _ in 0..10000 {
                if prune.step().unwrap() {
                    pruned = true;
                    break;
                }
            }
            assert!(pruned);
            assert!(!root
                .join(".recovery/migration/publication-history")
                .join(id)
                .join(generation)
                .join("pending/files/0000")
                .exists());
        }
        assert_eq!(fs::read(root.join("notes/page")).unwrap(), b"planned");
        assert_eq!(
            fs::read(saved.join("files/.staging/transactions/pending/files/0000")).unwrap(),
            b"planned"
        );
    }
    #[test]
    fn prune_child() {
        let Some(root) = std::env::var_os("KG_PRUNE_ROOT") else {
            return;
        };
        let id = std::env::var("KG_PRUNE_BACKUP").unwrap();
        let mut session = Session::new(Path::new(&root), &id, "pre-work-test")
            .unwrap()
            .unwrap();
        for _ in 0..10000 {
            if session.step().unwrap() {
                panic!("Pruning finished before exit hook.");
            }
        }
        panic!("Pruning did not reach quarantine.");
    }
    fn fixture(root: &Path) -> (String, PathBuf) {
        fs::create_dir_all(root.join(".staging/transactions/pending/files")).unwrap();
        fs::write(
            root.join(".staging/transactions/pending/files/0000"),
            vec![7; 150_000],
        )
        .unwrap();
        fs::write(
            root.join(".staging/transactions/pending/manifest.json"),
            b"retained manifest",
        )
        .unwrap();
        let work = Path::new(".recovery/migration/test-prune-snapshot");
        snapshot_worker::begin(root, work).unwrap();
        let mut snapshot = snapshot_worker::Session::new();
        let saved = loop {
            if let Some(path) = snapshot_worker::step(root, work, &mut snapshot).unwrap() {
                break path;
            }
        };
        let id = saved.file_name().unwrap().to_str().unwrap().to_owned();
        let history = root
            .join(".recovery/migration/publication-history")
            .join(&id)
            .join("pre-work-test");
        fs::create_dir_all(&history).unwrap();
        fs::rename(
            root.join(".staging/transactions/pending"),
            history.join("pending"),
        )
        .unwrap();
        fs::write(
            history.join("pending/files/0000.held"),
            b"external owner save",
        )
        .unwrap();
        (id, history)
    }
    fn find_quarantine(root: &Path) -> Option<PathBuf> {
        for entry in fs::read_dir(root).ok()? {
            let entry = entry.ok()?;
            let path = entry.path();
            if entry.file_type().ok()?.is_dir() {
                if let Some(found) = find_quarantine(&path) {
                    return Some(found);
                }
            } else if entry
                .file_name()
                .to_string_lossy()
                .starts_with(".kg-prune-")
            {
                return Some(path);
            }
        }
        None
    }
    #[test]
    fn quarantine_restart_prunes_only_snapshotted_bytes_and_retains_displaced_saves() {
        for mode in ["normal", "substitute", "external-save"] {
            let temp = tempfile::tempdir().unwrap();
            let root = fs::canonicalize(temp.path()).unwrap();
            let (id, history) = fixture(&root);
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "application::migration_backup::archive_prune::tests::prune_child",
                ])
                .env("KG_PRUNE_ROOT", &root)
                .env("KG_PRUNE_BACKUP", &id)
                .env("KG_PRUNE_STOP_AFTER_QUARANTINE", "1")
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(85));
            let quarantine =
                find_quarantine(&root.join(".recovery/migration/prune-quarantine")).unwrap();
            if mode == "substitute" {
                fs::write(&quarantine, b"substituted owner data").unwrap();
            }
            let external = [
                history.join("pending/manifest.json"),
                history.join("pending/files/0000"),
            ]
            .into_iter()
            .find(|path| !path.exists())
            .unwrap();
            if mode == "external-save" {
                fs::write(&external, b"concurrent external save").unwrap();
            }
            let mut session = Session::new(&root, &id, "pre-work-test").unwrap().unwrap();
            let mut done = false;
            let mut rejected = false;
            for _ in 0..10000 {
                match session.step() {
                    Ok(true) => {
                        done = true;
                        break;
                    }
                    Ok(false) => (),
                    Err(_) => {
                        rejected = true;
                        break;
                    }
                }
            }
            if mode == "substitute" {
                assert!(rejected);
                assert_eq!(fs::read(quarantine).unwrap(), b"substituted owner data");
            } else if mode == "external-save" {
                assert!(rejected);
                assert_eq!(fs::read(external).unwrap(), b"concurrent external save");
                assert!(quarantine.is_file());
            } else {
                assert!(done);
                assert!(!quarantine.exists());
            }
            assert_eq!(
                fs::read(history.join("pending/files/0000.held")).unwrap(),
                b"external owner save"
            );
            assert_eq!(
                fs::read(
                    root.join(".recovery/migration")
                        .join(&id)
                        .join("files/.staging/transactions/pending/files/0000")
                )
                .unwrap(),
                vec![7; 150_000]
            );
        }
    }
}
