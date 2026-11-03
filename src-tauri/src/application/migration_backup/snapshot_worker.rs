//! Experimental, durable collection snapshot worker. Work is kept under a
//! unique recovery directory and the completed snapshot is published once.
use super::*;

#[derive(Serialize, Deserialize)]
struct State {
    phase: String,
    backup: String,
}
pub(super) struct Session {
    tree: traversal::Session,
    verify: incremental_verify::Session,
    verification_work: Option<tempfile::TempDir>,
    initialized: bool,
}
impl Session {
    pub(super) fn new() -> Self {
        Self {
            tree: traversal::Session::new(),
            verify: incremental_verify::Session::ephemeral(),
            verification_work: None,
            initialized: false,
        }
    }
}
fn path(root: &Path, work: &Path, name: &str) -> PathBuf {
    root.join(work).join(name)
}
fn save(root: &Path, work: &Path, state: &State) -> Result<()> {
    write_atomic(&path(root, work, "state.json"), &serde_json::to_vec(state)?)
}
fn read(root: &Path, work: &Path) -> Result<State> {
    let mut bytes = Vec::new();
    bounded_copy::source_file(root, &work.join("state.json"))?
        .take(4097)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(GardenError::Recovery("Snapshot state is too large.".into()));
    }
    let state: State = serde_json::from_slice(&bytes)?;
    if !state.backup.starts_with("backup-") || Path::new(&state.backup).components().count() != 1 {
        return Err(GardenError::Recovery(
            "Snapshot identity is invalid.".into(),
        ));
    }
    Ok(state)
}
pub(super) fn begin(root: &Path, work: &Path) -> Result<()> {
    validate_relative_collection_path(work)?;
    if !work.starts_with(".recovery/migration") {
        return Err(GardenError::Recovery(
            "Snapshot work must stay in recovery.".into(),
        ));
    }
    let target = root.join(work);
    validate_real_collection_ancestors(root, &target)?;
    fs::create_dir_all(target.parent().unwrap())?;
    let stage = tempfile::tempdir_in(target.parent().unwrap())?;
    fs::create_dir_all(stage.path().join("snapshot/files"))?;
    manifest_records::create(&stage.path().join("snapshot/manifest.sqlite"))?;
    let state = State {
        phase: "copy".into(),
        backup: format!("backup-{}", Uuid::new_v4()),
    };
    write_atomic(
        &stage.path().join("state.json"),
        &serde_json::to_vec(&state)?,
    )?;
    write_atomic(
        &stage.path().join("snapshot/identity.json"),
        &serde_json::to_vec(&state.backup)?,
    )?;
    sync_directory(&stage.path().join("snapshot/files"))?;
    sync_directory(&stage.path().join("snapshot"))?;
    sync_directory(stage.path())?;
    let staging = stage.keep();
    use rustix::fs::{renameat_with, RenameFlags, CWD};
    renameat_with(CWD, &staging, CWD, &target, RenameFlags::NOREPLACE)
        .map_err(std::io::Error::from)?;
    sync_directory(target.parent().unwrap())
}
fn is_file(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => Ok(true),
        Ok(_) => Err(GardenError::Recovery(
            "Snapshot journal is not a file.".into(),
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}
pub(super) fn progress(root: &Path, work: &Path) -> Result<Option<String>> {
    let tree = work.join("tree.json");
    if !is_file(&root.join(&tree))? {
        return Ok(None);
    }
    let Some(relative) = traversal::pending(root, &tree)? else {
        return Ok(None);
    };
    let key = hex::encode(Sha256::digest(serde_json::to_vec(&relative)?));
    let journal = work.join("copies").join(format!("{key}.json"));
    if !is_file(&root.join(&journal))? {
        return Ok(None);
    }
    let p = bounded_copy::progress(root, &journal)?;
    let (done, label) = if p.copied < p.total {
        (p.copied, "copied")
    } else {
        (p.verified, "checked")
    };
    Ok(Some(format!(
        "Backup: {done} of {} bytes {label} for the current file.",
        p.total
    )))
}

pub(super) fn step(root: &Path, work: &Path, session: &mut Session) -> Result<Option<PathBuf>> {
    let mut state = read(root, work)?;
    if !session.initialized {
        if ["verify_snapshot", "ready", "published"].contains(&state.phase.as_str()) {
            state.phase = "verify_source".into();
            save(root, work, &state)?;
        }
        session.initialized = true;
    }
    let staging = path(root, work, "snapshot");
    let published = root.join(".recovery/migration").join(&state.backup);
    let snapshot = if fs::symlink_metadata(&staging).is_ok() {
        staging
    } else {
        published
    };
    let db = snapshot.join("manifest.sqlite");
    let tree = work.join("tree.json");
    match state.phase.as_str() {
        "copy" => {
            if !is_file(&root.join(&tree))? {
                traversal::begin(root, root, &tree)?;
                return Ok(None);
            }
            match traversal::next(root, root, &tree, &mut session.tree)? {
                traversal::Turn::More => Ok(None),
                traversal::Turn::Complete => {
                    state.phase = "verify_source".into();
                    save(root, work, &state)?;
                    Ok(None)
                }
                traversal::Turn::Entry(relative, directory) => {
                    let destination = work.join("snapshot/files").join(&relative);
                    if directory {
                        fs::create_dir_all(root.join(&destination))?;
                        manifest_records::put(&db, &relative, None)?;
                        traversal::acknowledge(root, root, &tree)?;
                        return Ok(None);
                    }
                    let key = hex::encode(Sha256::digest(serde_json::to_vec(&relative)?));
                    let copy = work.join("copies").join(format!("{key}.json"));
                    let hash = work.join("hashes").join(format!("{key}.json"));
                    let recorded = manifest_records::get(&db, &relative)?;
                    if recorded.is_none() {
                        if !is_file(&root.join(&copy))? {
                            let temp = work.join("temps").join(Uuid::new_v4().to_string());
                            if !bounded_copy::begin_verified_small(root, &copy, &relative, &temp)? {
                                bounded_copy::begin(root, &copy, &relative, &temp)?;
                                return Ok(None);
                            }
                        }
                        if !bounded_copy::step(root, &copy, 4 * 1024 * 1024)? {
                            return Ok(None);
                        }
                        let temp = bounded_copy::destination(root, &copy)?;
                        let digest = if let Some(digest) = manifest_cursor::digest_bounded(
                            root,
                            &temp,
                            manifest_cursor::BLOCK as u64,
                        )? {
                            digest
                        } else {
                            if !is_file(&root.join(&hash))? {
                                manifest_cursor::begin(root, root, &hash, &temp)?;
                                return Ok(None);
                            }
                            let Some(digest) = manifest_cursor::step(root, root, &hash)? else {
                                return Ok(None);
                            };
                            digest
                        };
                        manifest_records::put(&db, &relative, Some(&digest))?;
                    }
                    let temp = bounded_copy::destination(root, &copy)?;
                    if fs::symlink_metadata(root.join(&temp)).is_ok() {
                        fs::create_dir_all(root.join(destination.parent().unwrap()))?;
                        use rustix::fs::{renameat_with, RenameFlags, CWD};
                        renameat_with(
                            CWD,
                            root.join(&temp),
                            CWD,
                            root.join(&destination),
                            RenameFlags::NOREPLACE,
                        )
                        .map_err(std::io::Error::from)?;
                        sync_directory(root.join(destination.parent().unwrap()).as_path())?;
                        sync_directory(root.join(temp.parent().unwrap()).as_path())?;
                    } else if !is_file(&root.join(&destination))? {
                        return Err(GardenError::Recovery(
                            "Copied snapshot member is missing.".into(),
                        ));
                    }
                    traversal::acknowledge(root, root, &tree)?;
                    Ok(None)
                }
            }
        }
        "verify_source" | "verify_snapshot" => {
            let source = if state.phase == "verify_source" {
                root.to_owned()
            } else {
                snapshot.join("files")
            };
            if session.verification_work.is_none() {
                let scratch = tempfile::tempdir()?;
                incremental_verify::begin(&source, &db, scratch.path())?;
                session.verification_work = Some(scratch);
                return Ok(None);
            }
            let scratch = session.verification_work.as_ref().unwrap().path();
            if incremental_verify::step(&source, &db, scratch, &mut session.verify)? {
                state.phase = if state.phase == "verify_source" {
                    "verify_snapshot"
                } else {
                    "ready"
                }
                .into();
                session.verify = incremental_verify::Session::ephemeral();
                session.verification_work = None;
                save(root, work, &state)?;
            }
            Ok(None)
        }
        "ready" => {
            let source = path(root, work, "snapshot");
            let target = root.join(".recovery/migration").join(&state.backup);
            if fs::symlink_metadata(&source).is_ok() {
                use rustix::fs::{renameat_with, RenameFlags, CWD};
                renameat_with(CWD, &source, CWD, &target, RenameFlags::NOREPLACE)
                    .map_err(std::io::Error::from)?;
                sync_directory(source.parent().unwrap())?;
                sync_directory(target.parent().unwrap())?;
                #[cfg(test)]
                if std::env::var_os("KG_SNAPSHOT_STOP_AFTER_PUBLISH").is_some() {
                    std::process::exit(89);
                }
            } else if !fs::symlink_metadata(&target)?.is_dir() {
                return Err(GardenError::Recovery(
                    "Published snapshot is missing.".into(),
                ));
            }
            let identity: String =
                serde_json::from_str(&read_bounded_text(&target.join("identity.json"), 4096)?)?;
            if identity != state.backup {
                return Err(GardenError::Recovery(
                    "Published snapshot identity conflicts with its journal.".into(),
                ));
            }
            state.phase = "published".into();
            save(root, work, &state)?;
            Ok(Some(target))
        }
        "published" => {
            let target = root.join(".recovery/migration").join(&state.backup);
            let identity_path = target.join("identity.json");
            validate_real_collection_ancestors(root, &identity_path)?;
            if !fs::symlink_metadata(&identity_path)?.is_file() {
                return Err(GardenError::Recovery(
                    "Published snapshot identity is not a file.".into(),
                ));
            }
            let identity: String = serde_json::from_str(&read_bounded_text(&identity_path, 4096)?)?;
            if identity != state.backup {
                return Err(GardenError::Recovery(
                    "Published snapshot identity conflicts with its journal.".into(),
                ));
            }
            Ok(Some(target))
        }
        _ => Err(GardenError::Recovery("Snapshot phase is invalid.".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restarted_verification_rechecks_prior_source_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::write(root.join("owner"), b"original owner bytes").unwrap();
        let work = Path::new(".recovery/migration/work-current");
        begin(root, work).unwrap();
        let mut session = Session::new();
        for _ in 0..200 {
            step(root, work, &mut session).unwrap();
            if read(root, work).unwrap().phase == "verify_source"
                && session.verification_work.is_some()
            {
                break;
            }
        }
        for _ in 0..5 {
            step(root, work, &mut session).unwrap();
        }
        fs::write(root.join("owner"), b"changed owner bytes!").unwrap();
        let mut restarted = Session::new();
        let mut rejected = false;
        for _ in 0..300 {
            match step(root, work, &mut restarted) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(Some(_)) => break,
                Ok(None) => (),
            }
        }
        assert!(rejected);
        assert_eq!(
            fs::read(root.join("owner")).unwrap(),
            b"changed owner bytes!"
        );
    }
    #[test]
    fn crash_child() {
        let Some(root) = std::env::var_os("KG_SNAPSHOT_CRASH_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let work = Path::new(".recovery/migration/work-current");
        begin(&root, work).unwrap();
        let mut session = Session::new();
        for _ in 0..6 {
            assert!(step(&root, work, &mut session).unwrap().is_none());
        }
        std::process::exit(88);
    }
    #[test]
    fn publish_crash_child() {
        let Some(root) = std::env::var_os("KG_SNAPSHOT_PUBLISH_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let work = Path::new(".recovery/migration/work-current");
        begin(&root, work).unwrap();
        let mut session = Session::new();
        loop {
            assert!(step(&root, work, &mut session).unwrap().is_none());
        }
    }
    #[test]
    fn changed_source_after_publish_blocks_resumption() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::write(root.join("owner"), b"owner").unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::snapshot_worker::tests::publish_crash_child",
            ])
            .env("KG_SNAPSHOT_PUBLISH_ROOT", root)
            .env("KG_SNAPSHOT_STOP_AFTER_PUBLISH", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(89));
        fs::write(root.join("owner"), b"changed").unwrap();
        let work = Path::new(".recovery/migration/work-current");
        let mut session = Session::new();
        let mut rejected = false;
        for _ in 0..500 {
            match step(root, work, &mut session) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(Some(_)) => break,
                Ok(None) => (),
            }
        }
        assert!(rejected);
        assert_eq!(fs::read(root.join("owner")).unwrap(), b"changed");
    }
    #[test]
    fn abrupt_exit_after_publish_resumes_the_same_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::write(root.join("owner"), b"owner").unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::snapshot_worker::tests::publish_crash_child",
            ])
            .env("KG_SNAPSHOT_PUBLISH_ROOT", root)
            .env("KG_SNAPSHOT_STOP_AFTER_PUBLISH", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(89));
        let work = Path::new(".recovery/migration/work-current");
        let mut session = Session::new();
        let saved = loop {
            if let Some(saved) = step(root, work, &mut session).unwrap() {
                break saved;
            }
        };
        assert_eq!(fs::read(saved.join("files/owner")).unwrap(), b"owner");
        assert_eq!(step(root, work, &mut session).unwrap(), Some(saved));
    }
    #[test]
    fn interrupted_snapshot_resumes_and_publishes_verified_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::write(root.join("owner"), vec![6; 150_000]).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::snapshot_worker::tests::crash_child",
            ])
            .env("KG_SNAPSHOT_CRASH_ROOT", root)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(88));
        let work = Path::new(".recovery/migration/work-current");
        let mut session = Session::new();
        let mut saved = None;
        for _ in 0..1000 {
            if let Some(path) = step(root, work, &mut session).unwrap() {
                saved = Some(path);
                break;
            }
        }
        let saved = saved.expect("snapshot did not finish");
        assert_eq!(
            fs::read(saved.join("files/owner")).unwrap(),
            vec![6; 150_000]
        );
        assert!(
            manifest_records::get(&saved.join("manifest.sqlite"), Path::new("owner"))
                .unwrap()
                .is_some()
        );
        let restored = root
            .parent()
            .unwrap()
            .join(format!("kg-restored-{}", Uuid::new_v4()));
        restore_to_new_directory(&saved, &restored).unwrap();
        assert_eq!(fs::read(restored.join("owner")).unwrap(), vec![6; 150_000]);
        fs::remove_dir_all(restored).unwrap();
        assert_eq!(step(root, work, &mut session).unwrap(), Some(saved));
    }
}
