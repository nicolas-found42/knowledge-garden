//! Resumable verification of a disk-backed v3 manifest. Each turn hashes at
//! most one fixed block or visits at most sixteen directory entries.
use super::*;
use std::os::unix::fs::MetadataExt;

#[derive(Clone, Serialize, Deserialize)]
struct State {
    after: Option<Vec<u8>>,
    phase: String,
    device: u64,
    inode: u64,
    length: u64,
    modified: i64,
    modified_ns: i64,
    #[serde(default)]
    ignored_identity: Option<(u64, u64, u64, i64, i64)>,
}
pub(super) struct Session {
    traversal: traversal::Session,
    verified_index: Option<verified_index::VerifiedIndex>,
    ephemeral: bool,
    state: Option<State>,
    scope: Option<(PathBuf, PathBuf, PathBuf, Option<PathBuf>)>,
    memory_traversal: Option<traversal::MemoryTraversal>,
}
impl Session {
    pub(super) fn new() -> Self {
        Self {
            traversal: traversal::Session::new(),
            verified_index: None,
            ephemeral: false,
            state: None,
            scope: None,
            memory_traversal: None,
        }
    }
    /// Only for caller-owned scratch directories, discarded on every restart.
    /// Verification never resumes from an earlier process's proof or cursor.
    pub(super) fn ephemeral() -> Self {
        Self {
            ephemeral: true,
            ..Self::new()
        }
    }
    fn save(&mut self, work: &Path, state: &State) -> Result<()> {
        if self.ephemeral {
            self.state = Some(state.clone());
            Ok(())
        } else {
            save(work, state)
        }
    }
    fn next_tree(&mut self, files: &Path, work: &Path) -> Result<traversal::Turn> {
        if self.ephemeral {
            if self.memory_traversal.is_none() {
                self.memory_traversal = Some(traversal::MemoryTraversal::new(files)?);
            }
            return self.memory_traversal.as_mut().unwrap().next(files);
        }
        match fs::symlink_metadata(work.join(TREE)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                traversal::begin(files, work, Path::new(TREE))?;
                return Ok(traversal::Turn::More);
            }
            Err(e) => return Err(e.into()),
            Ok(m) if m.is_file() => (),
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Verification traversal journal is not a file.".into(),
                ))
            }
        }
        traversal::next(files, work, Path::new(TREE), &mut self.traversal)
    }
    fn acknowledge_tree(&mut self, files: &Path, work: &Path, descend: bool) -> Result<()> {
        if self.ephemeral {
            self.memory_traversal
                .as_mut()
                .unwrap()
                .acknowledge(files, descend)
        } else if descend {
            traversal::acknowledge(files, work, Path::new(TREE))
        } else {
            traversal::skip(files, work, Path::new(TREE))
        }
    }
    pub(super) fn capture(&mut self) -> Result<()> {
        self.verified_index = Some(verified_index::VerifiedIndex::new()?);
        Ok(())
    }
    pub(super) fn take_index(&mut self) -> Option<verified_index::VerifiedIndex> {
        self.verified_index.take()
    }
}
const STATE: &str = ".recovery/migration/verify/state.json";
const TREE: &str = ".recovery/migration/verify/tree.json";
fn check_db(path: &Path, state: &State) -> Result<()> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file()
        || (m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec())
            != (
                state.device,
                state.inode,
                state.length,
                state.modified,
                state.modified_ns,
            )
    {
        return Err(GardenError::Recovery(
            "Backup manifest changed during verification.".into(),
        ));
    }
    Ok(())
}
fn database_identity(path: &Path) -> Result<(u64, u64, u64, i64, i64)> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() {
        return Err(GardenError::Recovery(
            "Ignored-source manifest is not a regular file.".into(),
        ));
    }
    Ok((m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec()))
}
fn ignored_source(database: &Path, path: &Path) -> Result<bool> {
    let Some(entry) = manifest_records::get(database, path)? else {
        return Ok(false);
    };
    let mut parts = path.components();
    let prefix = parts.next();
    let name = parts.next();
    let digest = name.and_then(|part| match part {
        std::path::Component::Normal(value) => value.to_str(),
        _ => None,
    });
    if prefix != Some(std::path::Component::Normal("sources".as_ref()))
        || parts.next().is_some()
        || digest.is_none_or(|id| {
            id.len() != 64
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        })
        || entry.digest.is_some()
    {
        return Err(GardenError::Recovery(
            "Ignored migration source identity is invalid.".into(),
        ));
    }
    Ok(true)
}
fn save(work: &Path, state: &State) -> Result<()> {
    write_atomic(&work.join(STATE), &serde_json::to_vec(state)?)
}
pub(super) fn begin(files: &Path, database: &Path, work: &Path) -> Result<()> {
    begin_ignoring(files, database, work, None)
}
pub(super) fn begin_ignoring(
    _files: &Path,
    database: &Path,
    work: &Path,
    ignored: Option<&Path>,
) -> Result<()> {
    let m = fs::symlink_metadata(database)?;
    if !m.is_file() {
        return Err(GardenError::Recovery(
            "Backup manifest is not a regular file.".into(),
        ));
    }
    let path = work.join(STATE);
    validate_real_collection_ancestors(work, &path)?;
    fs::create_dir_all(path.parent().unwrap())?;
    let state = State {
        after: None,
        phase: "records".into(),
        device: m.dev(),
        inode: m.ino(),
        length: m.len(),
        modified: m.mtime(),
        modified_ns: m.mtime_nsec(),
        ignored_identity: ignored.map(database_identity).transpose()?,
    };
    let mut tmp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    tmp.write_all(&serde_json::to_vec(&state)?)?;
    tmp.as_file().sync_all()?;
    tmp.persist_noclobber(&path)
        .map_err(|e| GardenError::Io(e.error))?;
    sync_directory(path.parent().unwrap())
}
pub(super) fn step(
    files: &Path,
    database: &Path,
    work: &Path,
    session: &mut Session,
) -> Result<bool> {
    step_ignoring(files, database, work, session, None)
}
pub(super) fn step_ignoring(
    files: &Path,
    database: &Path,
    work: &Path,
    session: &mut Session,
    ignored: Option<&Path>,
) -> Result<bool> {
    if session.ephemeral {
        let scope = (
            files.to_owned(),
            database.to_owned(),
            work.to_owned(),
            ignored.map(Path::to_owned),
        );
        if session.scope.as_ref().is_some_and(|prior| prior != &scope) {
            return Err(GardenError::Recovery(
                "Verification scope changed during the session.".into(),
            ));
        }
        session.scope = Some(scope);
    }
    let mut state: State = if let Some(state) = &session.state {
        state.clone()
    } else {
        let mut bytes = Vec::new();
        bounded_copy::source_file(work, Path::new(STATE))?
            .take(16385)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 16384 {
            return Err(GardenError::Recovery(
                "Verification state is too large.".into(),
            ));
        }
        serde_json::from_slice(&bytes)?
    };
    if session.ephemeral && session.state.is_none() {
        state.after = None;
        state.phase = "records".into();
        session.state = Some(state.clone());
    }
    check_db(database, &state)?;
    if ignored.map(database_identity).transpose()? != state.ignored_identity {
        return Err(GardenError::Recovery(
            "Ignored migration sources changed during verification.".into(),
        ));
    }
    match state.phase.as_str() {
        "records" => {
            let Some((key, entry)) = manifest_records::next(database, state.after.as_deref())?
            else {
                if let Some(index) = &session.verified_index {
                    index.seal()?;
                }
                state.phase = "tree".into();
                session.save(work, &state)?;
                return Ok(false);
            };
            if let Some(ignored) = ignored {
                if ignored_source(ignored, &entry.path)? {
                    return Err(GardenError::Recovery(
                        "Migration source collides with the retained snapshot.".into(),
                    ));
                }
            }
            let m = fs::symlink_metadata(files.join(&entry.path))?;
            if let Some(expected) = entry.digest.as_deref() {
                if !m.is_file() {
                    return Err(GardenError::Recovery(
                        "Manifest file is missing or changed type.".into(),
                    ));
                }
                // A single-block member is already bounded work. Verify it in
                // one pass rather than syncing a restart cursor for a tiny file.
                // digest_file checks both the open file and its live identity.
                let actual = if m.len() <= manifest_cursor::BLOCK as u64 {
                    let Some(digest) = manifest_cursor::digest_bounded(
                        files,
                        &entry.path,
                        manifest_cursor::BLOCK as u64,
                    )?
                    else {
                        return Ok(false);
                    };
                    digest
                } else {
                    let name = hex::encode(Sha256::digest(&key));
                    let journal =
                        PathBuf::from(format!(".recovery/migration/verify/hashes/{name}.json"));
                    match fs::symlink_metadata(work.join(&journal)) {
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            manifest_cursor::begin(files, work, &journal, &entry.path)?
                        }
                        Err(e) => return Err(e.into()),
                        Ok(m) if m.is_file() => (),
                        Ok(_) => {
                            return Err(GardenError::Recovery(
                                "Verification cursor is not a file.".into(),
                            ))
                        }
                    }
                    let Some(actual) = manifest_cursor::step(files, work, &journal)? else {
                        return Ok(false);
                    };
                    actual
                };
                if actual != expected {
                    return Err(GardenError::Recovery(
                        "Backup member failed manifest verification.".into(),
                    ));
                }
            } else if !m.is_dir() {
                return Err(GardenError::Recovery(
                    "Manifest directory is missing or changed type.".into(),
                ));
            }
            check_db(database, &state)?;
            if let Some(index) = &session.verified_index {
                index.put(&entry.path, entry.digest.as_deref())?;
            }
            state.after = Some(key);
            session.save(work, &state)?;
            Ok(false)
        }
        "tree" => match session.next_tree(files, work)? {
            traversal::Turn::More => Ok(false),
            traversal::Turn::Complete => {
                state.phase = "complete".into();
                session.save(work, &state)?;
                Ok(true)
            }
            traversal::Turn::Entry(path, directory) => {
                if let Some(ignored) = ignored {
                    if ignored_source(ignored, &path)? {
                        if !directory {
                            return Err(GardenError::Recovery(
                                "Migration source path changed type.".into(),
                            ));
                        }
                        session.acknowledge_tree(files, work, false)?;
                        return Ok(false);
                    }
                }
                let digest = if let Some(index) = &session.verified_index {
                    index.get(&path)?
                } else {
                    manifest_records::get(database, &path)?.map(|entry| entry.digest)
                }
                .ok_or_else(|| {
                    GardenError::Recovery(
                        "Backup contains a member absent from its manifest.".into(),
                    )
                })?;
                if directory != digest.is_none() {
                    return Err(GardenError::Recovery(
                        "Backup member type differs from its manifest.".into(),
                    ));
                }
                session.acknowledge_tree(files, work, true)?;
                Ok(false)
            }
        },
        "complete" => Ok(true),
        _ => Err(GardenError::Recovery(
            "Verification state is invalid.".into(),
        )),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ephemeral_verification_rejects_tampered_bytes_even_with_a_forged_complete_cursor() {
        let temp = tempfile::tempdir().unwrap();
        let files = temp.path().join("files");
        let work = temp.path().join("work");
        fs::create_dir(&files).unwrap();
        fs::create_dir(&work).unwrap();
        fs::write(files.join("page"), b"original").unwrap();
        let db = temp.path().join("manifest.sqlite");
        manifest_records::create(&db).unwrap();
        let digest = manifest_cursor::digest_file(&files, Path::new("page")).unwrap();
        manifest_records::put(&db, Path::new("page"), Some(&digest)).unwrap();
        begin(&files, &db, &work).unwrap();
        let mut cursor: serde_json::Value =
            serde_json::from_slice(&fs::read(work.join(STATE)).unwrap()).unwrap();
        cursor["phase"] = serde_json::json!("complete");
        fs::write(work.join(STATE), serde_json::to_vec(&cursor).unwrap()).unwrap();
        fs::write(files.join("page"), b"tampered").unwrap();
        assert!(step(&files, &db, &work, &mut Session::ephemeral()).is_err());
        assert_eq!(fs::read(files.join("page")).unwrap(), b"tampered");
    }
    #[test]
    fn ephemeral_verification_cannot_reuse_progress_for_a_different_directory() {
        let temp = tempfile::tempdir().unwrap();
        let files = temp.path().join("files");
        let other = temp.path().join("other");
        let work = temp.path().join("work");
        fs::create_dir(&files).unwrap();
        fs::create_dir(&other).unwrap();
        fs::create_dir(&work).unwrap();
        fs::write(files.join("page"), b"original").unwrap();
        fs::write(other.join("page"), b"tampered").unwrap();
        let db = temp.path().join("manifest.sqlite");
        manifest_records::create(&db).unwrap();
        let digest = manifest_cursor::digest_file(&files, Path::new("page")).unwrap();
        manifest_records::put(&db, Path::new("page"), Some(&digest)).unwrap();
        begin(&files, &db, &work).unwrap();
        let mut session = Session::ephemeral();
        assert!(!step(&files, &db, &work, &mut session).unwrap());
        assert!(step(&other, &db, &work, &mut session).is_err());
    }
    #[test]
    fn verified_rows_remain_bound_when_manifest_changes_after_verification() {
        let temp = tempfile::tempdir().unwrap();
        let files = temp.path().join("files");
        fs::create_dir(&files).unwrap();
        fs::write(files.join("page"), b"original").unwrap();
        let db = temp.path().join("manifest.sqlite");
        manifest_records::create(&db).unwrap();
        let digest = manifest_cursor::digest_file(&files, Path::new("page")).unwrap();
        manifest_records::put(&db, Path::new("page"), Some(&digest)).unwrap();
        let work = temp.path().join("work");
        fs::create_dir(&work).unwrap();
        begin(&files, &db, &work).unwrap();
        let mut session = Session::new();
        session.capture().unwrap();
        for _ in 0..1000 {
            if step(&files, &db, &work, &mut session).unwrap() {
                break;
            }
        }
        let index = session.take_index().unwrap();
        manifest_records::set(&db, Path::new("page"), Some(&"f".repeat(64))).unwrap();
        assert_eq!(index.get(Path::new("page")).unwrap(), Some(Some(digest)));
        assert_eq!(index.next(None).unwrap().unwrap().1, PathBuf::from("page"));
    }
    #[test]
    fn replay_lookups_use_held_verified_rows_after_manifest_mutation() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let saved = root.join(".recovery/migration/backup-test");
        let files = saved.join("files");
        fs::create_dir_all(&files).unwrap();
        fs::write(files.join("page"), b"original").unwrap();
        let db = saved.join("manifest.sqlite");
        manifest_records::create(&db).unwrap();
        let digest = manifest_cursor::digest_file(&files, Path::new("page")).unwrap();
        manifest_records::put(&db, Path::new("page"), Some(&digest)).unwrap();
        fs::write(
            root.join(".recovery/migration/pre-recovery.json"),
            br#""backup-test""#,
        )
        .unwrap();
        let work = temp.path().join("work");
        fs::create_dir(&work).unwrap();
        begin(&files, &db, &work).unwrap();
        let mut session = Session::new();
        session.capture().unwrap();
        let mut complete = false;
        for _ in 0..1000 {
            if step(&files, &db, &work, &mut session).unwrap() {
                complete = true;
                break;
            }
        }
        assert!(complete);
        let index = std::sync::Arc::new(session.take_index().unwrap());
        manifest_records::set(&db, Path::new("page"), Some(&"f".repeat(64))).unwrap();
        let mut lookup =
            replay_authority::Session::new_with_index(root, Path::new("page"), Some(index.clone()))
                .unwrap()
                .unwrap();
        let expected = lookup.step().unwrap().unwrap();
        expected.verify_bytes(b"original").unwrap();
        assert!(expected.verify_bytes(b"modified").is_err());
        let mut rows = replay_authority::SnapshotRows::new(root, Some(index)).unwrap();
        let (path, expected) = rows.next(root).unwrap().unwrap();
        assert_eq!(path, PathBuf::from("page"));
        expected.unwrap().verify_bytes(b"original").unwrap();
    }
    #[test]
    fn planned_imports_are_excluded_but_unrelated_files_are_not() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("sources")).unwrap();
        fs::write(root.join("owner"), b"original").unwrap();
        let db = temp.path().join("manifest.sqlite");
        manifest_records::create(&db).unwrap();
        manifest_records::put(&db, Path::new("sources"), None).unwrap();
        let digest = manifest_cursor::digest_file(&root, Path::new("owner")).unwrap();
        manifest_records::put(&db, Path::new("owner"), Some(&digest)).unwrap();
        let ignored = temp.path().join("ignored.sqlite");
        manifest_records::create(&ignored).unwrap();
        let planned = PathBuf::from("sources").join("a".repeat(64));
        manifest_records::put(&ignored, &planned, None).unwrap();
        fs::create_dir(root.join(&planned)).unwrap();
        fs::write(root.join(&planned).join("index.md"), b"migration import").unwrap();
        let work = temp.path().join("work");
        fs::create_dir(&work).unwrap();
        begin_ignoring(&root, &db, &work, Some(&ignored)).unwrap();
        let mut session = Session::new();
        let mut done = false;
        for _ in 0..100 {
            if step_ignoring(&root, &db, &work, &mut session, Some(&ignored)).unwrap() {
                done = true;
                break;
            }
        }
        assert!(done);
        fs::write(root.join("unrelated"), b"unexpected").unwrap();
        let work2 = temp.path().join("work2");
        fs::create_dir(&work2).unwrap();
        begin_ignoring(&root, &db, &work2, Some(&ignored)).unwrap();
        let mut session = Session::new();
        let mut rejected = false;
        for _ in 0..100 {
            match step_ignoring(&root, &db, &work2, &mut session, Some(&ignored)) {
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
    #[test]
    fn verifies_records_and_rejects_unlisted_member() {
        let temp = tempfile::tempdir().unwrap();
        let files = temp.path().join("files");
        let work = temp.path().join("work");
        fs::create_dir(&files).unwrap();
        fs::create_dir(&work).unwrap();
        fs::write(files.join("owner"), vec![3; 140_000]).unwrap();
        let db = temp.path().join("manifest.sqlite");
        manifest_records::create(&db).unwrap();
        let digest = manifest_cursor::digest_file(&files, Path::new("owner")).unwrap();
        manifest_records::put(&db, Path::new("owner"), Some(&digest)).unwrap();
        begin(&files, &db, &work).unwrap();
        let mut session = Session::new();
        let mut done = false;
        for turn in 0..100 {
            if turn % 3 == 0 {
                session = Session::new();
            }
            if step(&files, &db, &work, &mut session).unwrap() {
                done = true;
                break;
            }
        }
        assert!(done);
        let other = temp.path().join("other");
        fs::create_dir(&other).unwrap();
        fs::write(other.join("owner"), vec![3; 140_000]).unwrap();
        fs::write(other.join("extra"), b"x").unwrap();
        let work2 = temp.path().join("work2");
        fs::create_dir(&work2).unwrap();
        begin(&other, &db, &work2).unwrap();
        let mut session = Session::new();
        let mut rejected = false;
        for _ in 0..100 {
            match step(&other, &db, &work2, &mut session) {
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
