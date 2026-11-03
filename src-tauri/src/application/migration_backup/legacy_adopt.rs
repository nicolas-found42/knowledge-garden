//! Convert a verified legacy manifest to a v3 index in bounded turns. The
//! original legacy manifest and saved files remain unchanged.
use super::*;
use std::os::unix::fs::MetadataExt;

#[derive(Serialize, Deserialize)]
struct State {
    backup: String,
    offset: u64,
    phase: String,
    manifest_identity: (u64, u64, u64, i64, i64),
}
pub(crate) struct Session {
    root: PathBuf,
    work: PathBuf,
    saved: PathBuf,
    state: State,
    current: Option<(u64, PathBuf, String, legacy_hash::Session)>,
    old_checked: bool,
    v3_verify: Option<(tempfile::TempDir, incremental_verify::Session)>,
    legacy_verify: Option<legacy_verify::Session>,
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
fn save(root: &Path, work: &Path, state: &State) -> Result<()> {
    write_atomic(
        &root.join(work).join("state.json"),
        &serde_json::to_vec(state)?,
    )
}
impl Session {
    pub(crate) fn new(root: &Path) -> Result<Option<Self>> {
        let mut raw = Vec::new();
        bounded_copy::source_file(root, Path::new(".recovery/migration/state.json"))?
            .take(4097)
            .read_to_end(&mut raw)?;
        if raw.len() > 4096 {
            return Err(GardenError::Recovery(
                "Migration state is too large.".into(),
            ));
        }
        let migration: MigrationState = serde_json::from_slice(&raw)?;
        if !["backup_verified", "evidence_imported", "cutover_preparing"]
            .contains(&migration.phase.as_str())
        {
            return Ok(None);
        }
        if !migration.backup.starts_with("backup-")
            || Path::new(&migration.backup).components().count() != 1
        {
            return Err(GardenError::Recovery(
                "Migration backup identity is invalid.".into(),
            ));
        }
        let saved = root.join(".recovery/migration").join(&migration.backup);
        let published = match fs::symlink_metadata(saved.join("manifest.sqlite")) {
            Ok(m) if m.is_file() => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e.into()),
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Migration manifest is not a regular file.".into(),
                ))
            }
        };
        let work =
            PathBuf::from(".recovery/migration").join(format!("legacy-adopt-{}", migration.backup));
        let state_path = root.join(&work).join("state.json");
        validate_real_collection_ancestors(root, &state_path)?;
        if published && !state_path.exists() {
            return Ok(None);
        }
        if !state_path.exists() {
            let parent = root.join(".recovery/migration");
            let stage = tempfile::tempdir_in(&parent)?;
            manifest_records::create(&stage.path().join("manifest.sqlite"))?;
            let state = State {
                backup: migration.backup.clone(),
                offset: 0,
                phase: "index".into(),
                manifest_identity: identity(&saved.join("manifest.json"))?,
            };
            write_atomic(
                &stage.path().join("state.json"),
                &serde_json::to_vec(&state)?,
            )?;
            sync_directory(stage.path())?;
            let temp = stage.keep();
            use rustix::fs::{renameat_with, RenameFlags, CWD};
            renameat_with(CWD, &temp, CWD, root.join(&work), RenameFlags::NOREPLACE)
                .map_err(std::io::Error::from)?;
            sync_directory(&parent)?;
        }
        let mut bytes = Vec::new();
        bounded_copy::source_file(root, &work.join("state.json"))?
            .take(16385)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 16384 {
            return Err(GardenError::Recovery(
                "Legacy adoption state is too large.".into(),
            ));
        }
        let mut state: State = serde_json::from_slice(&bytes)?;
        if state.backup != migration.backup
            || identity(&saved.join("manifest.json"))? != state.manifest_identity
        {
            return Err(GardenError::Recovery(
                "Legacy adoption identity changed.".into(),
            ));
        }
        if state.phase == "complete" && published {
            return Ok(None);
        }
        if state.phase != "index" {
            state.phase = "verify_v3".into();
            save(root, &work, &state)?;
        }
        Ok(Some(Self {
            root: root.to_owned(),
            work,
            saved,
            state,
            current: None,
            old_checked: false,
            v3_verify: None,
            legacy_verify: None,
        }))
    }
    fn database(&self) -> PathBuf {
        let staged = self.root.join(&self.work).join("manifest.sqlite");
        if staged.is_file() {
            staged
        } else {
            self.saved.join("manifest.sqlite")
        }
    }
    pub(crate) fn step(&mut self) -> Result<bool> {
        if identity(&self.saved.join("manifest.json"))? != self.state.manifest_identity {
            return Err(GardenError::Recovery(
                "Legacy manifest changed during adoption.".into(),
            ));
        }
        let database = self.database();
        match self.state.phase.as_str() {
            "index" => {
                if self.current.is_none() {
                    let Some((offset, path, digest)) = legacy_manifest::next(
                        &self.saved.join("manifest.json"),
                        self.state.offset,
                    )?
                    else {
                        self.state.phase = "verify_v3".into();
                        save(&self.root, &self.work, &self.state)?;
                        return Ok(false);
                    };
                    if digest.is_none() {
                        manifest_records::put(&database, &path, None)?;
                        self.state.offset = offset;
                        save(&self.root, &self.work, &self.state)?;
                        return Ok(false);
                    }
                    self.current =
                        Some((offset, path, digest.unwrap(), legacy_hash::Session::new()));
                    self.old_checked = false;
                }
                let (offset, relative, expected, legacy) = self.current.as_mut().unwrap();
                if !self.old_checked {
                    let Some(actual) =
                        legacy_hash::step(&self.saved.join("files"), relative, legacy)?
                    else {
                        return Ok(false);
                    };
                    if &actual != expected {
                        return Err(GardenError::Recovery(
                            "Legacy backup member failed verification.".into(),
                        ));
                    }
                    self.old_checked = true;
                    return Ok(false);
                }
                let key = hex::encode(Sha256::digest(serde_json::to_vec(&*relative)?));
                let journal = self.work.join("hashes").join(format!("{key}.json"));
                let source = self
                    .saved
                    .strip_prefix(&self.root)
                    .map_err(|_| {
                        GardenError::Recovery("Legacy backup escaped the collection.".into())
                    })?
                    .join("files")
                    .join(&*relative);
                match fs::symlink_metadata(self.root.join(&journal)) {
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        manifest_cursor::begin(&self.root, &self.root, &journal, &source)?;
                        return Ok(false);
                    }
                    Err(e) => return Err(e.into()),
                    Ok(m) if m.is_file() => (),
                    Ok(_) => {
                        return Err(GardenError::Recovery(
                            "Legacy adoption cursor is not a file.".into(),
                        ))
                    }
                }
                let Some(hash) = manifest_cursor::step(&self.root, &self.root, &journal)? else {
                    return Ok(false);
                };
                manifest_records::put(&database, relative, Some(&hash))?;
                #[cfg(test)]
                if std::env::var_os("KG_LEGACY_ADOPT_STOP_AFTER_ENTRY").is_some() {
                    std::process::exit(97);
                }
                self.state.offset = *offset;
                save(&self.root, &self.work, &self.state)?;
                self.current = None;
                self.old_checked = false;
                Ok(false)
            }
            "verify_v3" => {
                if self.v3_verify.is_none() {
                    let work = tempfile::tempdir()?;
                    incremental_verify::begin(&self.saved.join("files"), &database, work.path())?;
                    self.v3_verify = Some((work, incremental_verify::Session::new()));
                    return Ok(false);
                }
                let (work, session) = self.v3_verify.as_mut().unwrap();
                if incremental_verify::step(
                    &self.saved.join("files"),
                    &database,
                    work.path(),
                    session,
                )? {
                    self.state.phase = "verify_legacy".into();
                    save(&self.root, &self.work, &self.state)?;
                    self.v3_verify = None;
                }
                Ok(false)
            }
            "verify_legacy" => {
                if self.legacy_verify.is_none() {
                    self.legacy_verify = Some(legacy_verify::Session::new(&self.saved)?);
                    return Ok(false);
                }
                if legacy_verify::step(&self.saved, self.legacy_verify.as_mut().unwrap())? {
                    self.state.phase = "publish".into();
                    save(&self.root, &self.work, &self.state)?;
                    self.legacy_verify = None;
                }
                Ok(false)
            }
            "publish" => {
                let target = self.saved.join("manifest.sqlite");
                let staged = self.root.join(&self.work).join("manifest.sqlite");
                if staged.exists() {
                    use rustix::fs::{renameat_with, RenameFlags, CWD};
                    renameat_with(CWD, &staged, CWD, &target, RenameFlags::NOREPLACE)
                        .map_err(std::io::Error::from)?;
                    sync_directory(staged.parent().unwrap())?;
                    sync_directory(&self.saved)?;
                } else if !fs::symlink_metadata(&target)?.is_file() {
                    return Err(GardenError::Recovery("Adopted manifest is missing.".into()));
                }
                #[cfg(test)]
                if std::env::var_os("KG_LEGACY_ADOPT_STOP_AFTER_PUBLISH").is_some() {
                    std::process::exit(96);
                }
                self.state.phase = "complete".into();
                save(&self.root, &self.work, &self.state)?;
                Ok(true)
            }
            "complete" => Ok(true),
            _ => Err(GardenError::Recovery(
                "Legacy adoption phase is invalid.".into(),
            )),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn child() {
        let Some(root) = std::env::var_os("KG_LEGACY_ADOPT_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let mut session = Session::new(&root).unwrap().unwrap();
        loop {
            if session.step().unwrap() {
                panic!("hook did not run");
            }
        }
    }
    #[test]
    fn interrupted_legacy_adoption_retains_original_manifest_and_bytes() {
        for (hook, code) in [
            ("KG_LEGACY_ADOPT_STOP_AFTER_ENTRY", 97),
            ("KG_LEGACY_ADOPT_STOP_AFTER_PUBLISH", 96),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("collection");
            fs::create_dir(&root).unwrap();
            let root = fs::canonicalize(root).unwrap();
            fs::write(root.join("owner"), vec![9; 150_000]).unwrap();
            let saved = backup(&root).unwrap();
            let manifest = fs::read(saved.join("manifest.json")).unwrap();
            let state = MigrationState {
                backup: saved.file_name().unwrap().to_str().unwrap().into(),
                phase: "backup_verified".into(),
            };
            fs::write(
                root.join(".recovery/migration/state.json"),
                serde_json::to_vec(&state).unwrap(),
            )
            .unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "application::migration_backup::legacy_adopt::tests::child",
                ])
                .env("KG_LEGACY_ADOPT_ROOT", &root)
                .env(hook, "1")
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(code));
            let mut resumed = Session::new(&root).unwrap().unwrap();
            let mut complete = false;
            for _ in 0..2000 {
                if resumed.step().unwrap() {
                    complete = true;
                    break;
                }
            }
            assert!(complete);
            assert!(saved.join("manifest.sqlite").is_file());
            assert_eq!(fs::read(saved.join("manifest.json")).unwrap(), manifest);
            verify_backup(&saved).unwrap();
            assert_eq!(
                fs::read(saved.join("files/owner")).unwrap(),
                vec![9; 150_000]
            );
        }
    }
}
