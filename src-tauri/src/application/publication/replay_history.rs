//! Reconstruct authenticated record outputs from previously archived transactions.
use super::*;
use std::os::unix::fs::MetadataExt;

pub(super) struct History {
    path: Option<PathBuf>,
    directory: Option<fs::ReadDir>,
    identity: Option<(u64, u64, i64, i64)>,
    _work: tempfile::TempDir,
    index: Connection,
    after: String,
    discovered: bool,
    active: Option<Archived>,
    verified: Option<std::sync::Arc<migration_backup::verified_index::VerifiedIndex>>,
}
impl History {
    pub(super) fn new(
        root: &Path,
        generation: &str,
        verified: Option<std::sync::Arc<migration_backup::verified_index::VerifiedIndex>>,
    ) -> Result<Self> {
        let work = tempfile::tempdir()?;
        let index = Connection::open(work.path().join("history.sqlite"))?;
        index.execute_batch("CREATE TABLE names (name TEXT PRIMARY KEY) WITHOUT ROWID")?;
        let journal = Path::new(".recovery/migration/pre-recovery.json");
        let path = match migration_backup::read_bounded_nofollow(root, journal, 4096) {
            Ok(bytes) => {
                let id: String = serde_json::from_slice(&bytes)?;
                if !id.starts_with("backup-") || Path::new(&id).components().count() != 1 {
                    return Err(GardenError::Recovery(
                        "Publication backup identity is invalid.".into(),
                    ));
                }
                Some(
                    root.join(".recovery/migration/publication-history")
                        .join(id)
                        .join(generation),
                )
            }
            Err(GardenError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        let mut directory = None;
        let mut identity = None;
        if let Some(path) = &path {
            validate_real_collection_ancestors(root, &path.join("entry"))?;
            match fs::symlink_metadata(path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
                Ok(m) if m.file_type().is_dir() => {
                    identity = Some((m.dev(), m.ino(), m.mtime(), m.mtime_nsec()));
                    directory = Some(fs::read_dir(path)?);
                }
                Ok(_) => {
                    return Err(GardenError::Recovery(
                        "Publication history is not a directory.".into(),
                    ))
                }
            }
        }
        let discovered = directory.is_none();
        Ok(Self {
            path,
            directory,
            identity,
            _work: work,
            index,
            after: String::new(),
            discovered,
            active: None,
            verified,
        })
    }
    pub(super) fn precedes(&self, name: &str) -> Result<bool> {
        let last: Option<String> = self.index.query_row("SELECT CASE WHEN length(name)<=255 THEN name ELSE '' END FROM names ORDER BY name DESC LIMIT 1", [], |row| row.get(0)).optional()?;
        if last.as_ref().is_some_and(|value| value.is_empty()) {
            return Err(GardenError::Recovery(
                "Archived publication name is invalid.".into(),
            ));
        }
        Ok(last.is_none_or(|value| value.as_str() < name))
    }
    pub(super) fn contains(&self, name: &str) -> Result<bool> {
        let found: Option<i64> = self
            .index
            .query_row("SELECT 1 FROM names WHERE name=?1", [name], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(found.is_some())
    }
    pub(super) fn step(
        &mut self,
        root: &Path,
        generation: &str,
        overlay: &page_identity::RecordOverlay,
    ) -> Result<bool> {
        if !self.discovered {
            let m = fs::symlink_metadata(self.path.as_ref().unwrap())?;
            if !m.file_type().is_dir()
                || Some((m.dev(), m.ino(), m.mtime(), m.mtime_nsec())) != self.identity
            {
                return Err(GardenError::Recovery(
                    "Publication history changed during discovery.".into(),
                ));
            }
            if let Some(entry) = self.directory.as_mut().unwrap().next() {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    return Err(GardenError::Recovery(
                        "Archived publication is not a directory.".into(),
                    ));
                }
                let name = entry.file_name().into_string().map_err(|_| {
                    GardenError::Recovery("Archived publication name is invalid.".into())
                })?;
                self.index
                    .execute("INSERT INTO names(name) VALUES (?1)", [&name])?;
                return Ok(false);
            }
            self.directory = None;
            self.discovered = true;
            return Ok(false);
        }
        if let Some(active) = self.active.as_mut() {
            if active.step(root, overlay)? {
                self.after = active.name.clone();
                self.active = None;
            }
            return Ok(false);
        }
        let name: Option<String> = self
            .index
            .query_row(
                "SELECT CASE WHEN length(name)<=255 THEN name ELSE '' END FROM names WHERE name>?1 ORDER BY name LIMIT 1",
                [&self.after],
                |row| row.get(0),
            )
            .optional()?;
        let Some(name) = name else {
            return Ok(true);
        };
        if name.is_empty() {
            return Err(GardenError::Recovery(
                "Archived publication name is invalid.".into(),
            ));
        }
        let path = self.path.as_ref().unwrap().join(&name);
        match fs::symlink_metadata(path.join("manifest.json")) {
            Ok(m) if m.file_type().is_file() => {
                self.active = Some(Archived::new(
                    root,
                    &path,
                    &name,
                    generation,
                    self.verified.clone(),
                )?);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => self.after = name,
            Err(error) => return Err(error.into()),
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Archived manifest is not a regular file.".into(),
                ))
            }
        }
        Ok(false)
    }
}
struct Archived {
    name: String,
    path: PathBuf,
    scanner: replay_manifest::Scanner,
    authority: Option<migration_backup::replay_authority::Session>,
    expected: Option<migration_backup::replay_authority::Expected>,
    scanned: bool,
    entry: u64,
    pending: Option<(
        replay_entry::Meta,
        migration_backup::replay_authority::Session,
    )>,
    cleaned: bool,
    verified: Option<std::sync::Arc<migration_backup::verified_index::VerifiedIndex>>,
}
impl Archived {
    fn new(
        root: &Path,
        path: &Path,
        name: &str,
        generation: &str,
        verified: Option<std::sync::Arc<migration_backup::verified_index::VerifiedIndex>>,
    ) -> Result<Self> {
        let original = Path::new(".staging/transactions")
            .join(name)
            .join("manifest.json");
        let authority = migration_backup::replay_authority::Session::new_with_index(
            root,
            &original,
            verified.clone(),
        )?
        .ok_or_else(|| {
            GardenError::Recovery("Archived publication lacks snapshot evidence.".into())
        })?;
        Ok(Self {
            name: name.into(),
            path: path.into(),
            scanner: replay_manifest::Scanner::new(root, path, generation)?,
            authority: Some(authority),
            expected: None,
            scanned: false,
            entry: 0,
            pending: None,
            cleaned: false,
            verified,
        })
    }
    fn step(&mut self, root: &Path, overlay: &page_identity::RecordOverlay) -> Result<bool> {
        if let Some(authority) = self.authority.as_mut() {
            if let Some(expected) = authority.step()? {
                self.expected = Some(expected);
                self.authority = None;
            }
            return Ok(false);
        }
        if !self.scanned {
            self.scanned = self.scanner.step()?;
            if self.scanned {
                self.scanner.verify_digest(self.expected.as_ref())?;
            }
            return Ok(false);
        }
        self.scanner.verify()?;
        if let Some((entry, authority)) = self.pending.as_mut() {
            if let Some(expected) = authority.step()? {
                let relative = self
                    .path
                    .strip_prefix(root)
                    .map_err(|_| {
                        GardenError::Recovery("Archived publication escaped collection.".into())
                    })?
                    .join(&entry.staged);
                let bytes = migration_backup::read_bounded_nofollow(root, &relative, 16384)?;
                expected.verify_bytes(&bytes)?;
                let receipt = self
                    .path
                    .strip_prefix(root)
                    .map_err(|_| {
                        GardenError::Recovery("Archived publication escaped collection.".into())
                    })?
                    .join(format!("files/{:04}.done", self.entry));
                if migration_backup::read_bounded_nofollow(root, &receipt, 16384)? != bytes {
                    return Err(GardenError::Recovery(
                        "Archived trusted record completion proof changed.".into(),
                    ));
                }
                overlay.put(Path::new(&entry.destination), &bytes)?;
                self.pending = None;
                self.entry += 1;
            }
            return Ok(false);
        }
        if self.entry < self.scanner.entry_count() {
            let entry = self.scanner.entry(self.entry)?;
            if Path::new(&entry.destination).parent()
                == Some(Path::new(".recovery/trusted-pages/records"))
            {
                let original = Path::new(".staging/transactions")
                    .join(&self.name)
                    .join(&entry.staged);
                let authority = migration_backup::replay_authority::Session::new_with_index(
                    root,
                    &original,
                    self.verified.clone(),
                )?
                .ok_or_else(|| {
                    GardenError::Recovery("Archived record lacks snapshot evidence.".into())
                })?;
                self.pending = Some((entry, authority));
            } else {
                self.entry += 1;
            }
            return Ok(false);
        }
        if !self.cleaned {
            self.cleaned = self.scanner.cleanup_step()?;
            return Ok(false);
        }
        Ok(true)
    }
}
