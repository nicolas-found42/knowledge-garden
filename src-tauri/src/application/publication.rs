//! A recoverable publication retains both sides of an atomic exchange. The
//! collection lock serializes app writers; external editors need not take it.
mod page_identity;
use super::*;
mod replay_entry;
mod replay_history;
mod replay_manifest;

#[derive(Serialize, Deserialize)]
struct PublicationManifest {
    entries: Vec<PublicationEntry>,
    #[serde(default)]
    removals: Vec<String>,
}
#[derive(Serialize, Deserialize)]
struct PublicationEntry {
    destination: String,
    staged: String,
    #[serde(default)]
    expected: Option<Vec<u8>>,
}

pub(super) struct OriginalPublication {
    pub destination: PathBuf,
    pub source: PathBuf,
    pub proposed_digest: String,
}

pub(super) fn commit_publication_with_originals(
    root: &Path,
    files: Vec<(PathBuf, Vec<u8>)>,
    originals: Vec<OriginalPublication>,
    bases: &HashMap<PathBuf, Vec<u8>>,
    fail_after_files: Option<usize>,
) -> Result<()> {
    let has_originals = !originals.is_empty();
    for original in originals {
        retain_version_original(
            root,
            &original.destination,
            &original.source,
            &original.proposed_digest,
        )?;
    }
    // The original is durably installed before the journal can activate it.
    // Simulate the boundary between those operations with the existing failpoint.
    let fail_after_files = if has_originals && fail_after_files == Some(usize::MAX) {
        Some(0)
    } else {
        fail_after_files
    };
    commit_publication(root, files, bases, fail_after_files)
}

fn open_collection_directory(root: &Path, relative: &Path) -> Result<File> {
    use rustix::fs::{openat, Mode, OFlags, CWD};
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut directory = openat(CWD, root, flags, Mode::empty()).map_err(std::io::Error::from)?;
    for component in relative.components() {
        let std::path::Component::Normal(name) = component else {
            return Err(GardenError::Invalid(
                "Invalid publication directory.".into(),
            ));
        };
        directory = openat(&directory, name, flags, Mode::empty()).map_err(std::io::Error::from)?;
    }
    Ok(File::from(directory))
}

fn open_or_create_collection_directory(root: &Path, relative: &Path) -> Result<File> {
    use rustix::fs::{mkdirat, openat, Mode, OFlags, CWD};
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut directory = openat(CWD, root, flags, Mode::empty()).map_err(std::io::Error::from)?;
    for component in relative.components() {
        let std::path::Component::Normal(name) = component else {
            return Err(GardenError::Invalid("Invalid archive directory.".into()));
        };
        match mkdirat(&directory, name, Mode::from_bits_truncate(0o700)) {
            Ok(()) => rustix::fs::fsync(&directory).map_err(std::io::Error::from)?,
            Err(rustix::io::Errno::EXIST) => {}
            Err(error) => return Err(GardenError::Io(error.into())),
        }
        directory = openat(&directory, name, flags, Mode::empty()).map_err(std::io::Error::from)?;
    }
    Ok(File::from(directory))
}

pub(super) fn retain_version_original(
    root: &Path,
    path: &Path,
    source: &Path,
    expected: &str,
) -> Result<()> {
    use rustix::fs::{linkat, openat, unlinkat, AtFlags, Mode, OFlags};
    let relative = path
        .strip_prefix(root)
        .map_err(|_| GardenError::Invalid("Archive escaped collection.".into()))?;
    let source_relative = source
        .strip_prefix(root)
        .map_err(|_| GardenError::Invalid("Original escaped collection.".into()))?;
    validate_relative_collection_path(relative)?;
    validate_relative_collection_path(source_relative)?;
    let source_dir = open_collection_directory(root, source_relative.parent().unwrap())?;
    let mut input = File::from(
        openat(
            &source_dir,
            source_relative.file_name().unwrap(),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(std::io::Error::from)?,
    );
    if !input.metadata()?.is_file() {
        return Err(GardenError::Invalid(
            "The current original is not a regular file.".into(),
        ));
    }
    let dir = open_or_create_collection_directory(root, relative.parent().unwrap())?;
    let temporary = format!(".archive-{}", uuid::Uuid::new_v4());
    let output = openat(
        &dir,
        temporary.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_bits_truncate(0o600),
    )
    .map_err(std::io::Error::from)?;
    let mut output = File::from(output);
    let result = (|| -> Result<()> {
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            digest.update(&buffer[..n]);
            output.write_all(&buffer[..n])?;
        }
        if hex::encode(digest.finalize()) != expected {
            return Err(GardenError::Invalid(
                "The current original changed before its version could be retained.".into(),
            ));
        }
        output.sync_all()?;
        let target = relative.file_name().unwrap();
        match linkat(&dir, temporary.as_str(), &dir, target, AtFlags::empty()) {
            Ok(()) => dir.sync_all()?,
            Err(rustix::io::Errno::EXIST) => {
                if digest_at(&dir, target)?.as_deref() != Some(expected) {
                    return Err(GardenError::Invalid("The retained version conflicts with the current generation; it was not overwritten.".into()));
                }
            }
            Err(error) => return Err(GardenError::Io(error.into())),
        }
        Ok(())
    })();
    let cleanup = unlinkat(&dir, temporary.as_str(), AtFlags::empty());
    if result.is_ok() {
        cleanup.map_err(std::io::Error::from)?;
        dir.sync_all()?;
    }
    result
}

fn digest_at(directory: &File, name: &std::ffi::OsStr) -> Result<Option<String>> {
    use rustix::fs::{openat, Mode, OFlags};
    let fd = match openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(error) => return Err(GardenError::Io(error.into())),
    };
    let mut file = File::from(fd);
    if !file.metadata()?.is_file() {
        return Err(GardenError::Invalid(
            "An original publication path is not a regular file.".into(),
        ));
    }
    let mut digest = Sha256::new();
    let mut buf = [0_u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    Ok(Some(hex::encode(digest.finalize())))
}

pub(super) fn commit_publication(
    root: &Path,
    files: Vec<(PathBuf, Vec<u8>)>,
    bases: &HashMap<PathBuf, Vec<u8>>,
    fail_after_files: Option<usize>,
) -> Result<()> {
    commit_publication_with_removals(root, files, Vec::new(), bases, fail_after_files)
}

pub(super) fn commit_publication_with_removals(
    root: &Path,
    files: Vec<(PathBuf, Vec<u8>)>,
    removals: Vec<PathBuf>,
    bases: &HashMap<PathBuf, Vec<u8>>,
    fail_after_files: Option<usize>,
) -> Result<()> {
    if files.is_empty() && removals.is_empty() {
        return Ok(());
    }
    let mut files = files;
    if trusted_pages::active(root)? {
        let mut trusted = Vec::new();
        for (destination, bytes) in &files {
            let relative = destination.strip_prefix(root).map_err(|_| {
                GardenError::Invalid("A publication path escaped the collection.".into())
            })?;
            if (relative.starts_with("pages")
                && relative
                    .extension()
                    .is_some_and(|extension| extension == "md"))
                || (relative.starts_with("sources")
                    && relative.file_name().is_some_and(|name| name == "index.md"))
            {
                trusted.extend(trusted_pages::publication_files(root, relative, bytes)?);
            }
        }
        files.extend(trusted);
    }
    let mut removals = removals;
    if trusted_pages::active(root)? {
        let mut trusted_removals = Vec::new();
        for path in &removals {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| GardenError::Invalid("A removal escaped the collection.".into()))?;
            trusted_removals.extend(trusted_pages::records_under(root, relative)?);
        }
        removals.extend(trusted_removals);
    }
    let removals = removals
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| GardenError::Invalid("A removal escaped the collection.".into()))?;
            validate_relative_collection_path(relative)?;
            Ok(relative.to_string_lossy().into_owned())
        })
        .collect::<Result<Vec<_>>>()?;
    let transactions = root.join(".staging/transactions");
    fs::create_dir_all(&transactions)?;
    let staging = tempfile::tempdir_in(&transactions)?;
    fs::create_dir_all(staging.path().join("files"))?;
    let mut entries = Vec::new();
    for (index, (destination, bytes)) in files.into_iter().enumerate() {
        let relative = destination.strip_prefix(root).map_err(|_| {
            GardenError::Invalid("A publication path escaped the collection.".into())
        })?;
        validate_relative_collection_path(relative)?;
        let staged = format!("files/{index:04}");
        write_atomic(&staging.path().join(&staged), &bytes)?;
        entries.push(PublicationEntry {
            destination: relative.to_string_lossy().into_owned(),
            staged,
            expected: bases
                .get(&destination)
                .cloned()
                .or_else(|| fs::read(&destination).ok()),
        });
    }
    write_atomic(
        &staging.path().join("manifest.json"),
        &serde_json::to_vec(&PublicationManifest { entries, removals })?,
    )?;
    sync_directory(staging.path())?;
    let transaction_path = staging.keep();
    sync_directory(&transactions)?;
    if !apply_publication(root, &transaction_path, fail_after_files)? {
        return Err(GardenError::Invalid("Publication is retained for automatic recovery while a page identity is missing or ambiguous.".into()));
    }
    fs::remove_dir_all(transaction_path)?;
    sync_directory(&transactions)
}

pub(super) fn recover_publications(root: &Path) -> Result<()> {
    let transactions = root.join(".staging/transactions");
    fs::create_dir_all(&transactions)?;
    let mut paths = fs::read_dir(&transactions)?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();
    for path in paths {
        if !path.is_dir() {
            continue;
        }
        if path.join("manifest.json").exists() && !apply_publication(root, &path, None)? {
            continue;
        }
        fs::remove_dir_all(path)?;
    }
    sync_directory(&transactions)
}

/// Discover a transaction directory incrementally before replaying it in sorted
/// order. A restart repeats discovery; each transaction retains its own done
/// records until replay and cleanup finish.
pub(super) struct RecoverySession {
    generation: String,
    directory: Option<fs::ReadDir>,
    identity: Option<(u64, u64, i64, i64)>,
    index: Option<(tempfile::TempDir, Connection)>,
    discovered: bool,
    discovered_count: u64,
    completed_count: u64,
    active: Option<ActiveReplay>,
    after: String,
    record_overlay: Option<page_identity::RecordOverlay>,
    history: Option<replay_history::History>,
    history_done: bool,
    verified_index: Option<std::sync::Arc<migration_backup::verified_index::VerifiedIndex>>,
}
impl RecoverySession {
    pub(super) fn new() -> Self {
        Self {
            generation: "pre-work-unassociated".into(),
            directory: None,
            identity: None,
            index: None,
            discovered: false,
            discovered_count: 0,
            completed_count: 0,
            active: None,
            after: String::new(),
            record_overlay: None,
            history: None,
            history_done: false,
            verified_index: None,
        }
    }
    pub(super) fn for_generation(generation: &str) -> Result<Self> {
        if !generation.starts_with("pre-work-")
            || Path::new(generation).components().count() != 1
            || generation.len() > 128
        {
            return Err(GardenError::Recovery(
                "Publication replay generation is invalid.".into(),
            ));
        }
        let mut session = Self::new();
        session.generation = generation.into();
        Ok(session)
    }
    pub(crate) fn set_verified_index(
        &mut self,
        index: std::sync::Arc<migration_backup::verified_index::VerifiedIndex>,
    ) {
        self.verified_index = Some(index);
    }
    pub(super) fn progress(&self) -> String {
        if self.discovered {
            format!(
                "Recovering interrupted publications: {} of {} transactions completed.",
                self.completed_count, self.discovered_count
            )
        } else {
            format!(
                "Finding interrupted publications: {} transactions checked.",
                self.discovered_count
            )
        }
    }
    pub(super) fn step(&mut self, root: &Path) -> Result<bool> {
        use std::os::unix::fs::MetadataExt;
        let transactions = root.join(".staging/transactions");
        if self.record_overlay.is_none() {
            self.record_overlay = Some(page_identity::RecordOverlay::new()?);
        }
        if !self.history_done {
            if self.history.is_none() {
                self.history = Some(replay_history::History::new(
                    root,
                    &self.generation,
                    self.verified_index.clone(),
                )?);
            }
            if self.history.as_mut().unwrap().step(
                root,
                &self.generation,
                self.record_overlay.as_ref().unwrap(),
            )? {
                self.history_done = true;
            }
            return Ok(false);
        }
        if self.index.is_none() {
            validate_real_collection_ancestors(root, &transactions.join("entry"))?;
            fs::create_dir_all(&transactions)?;
            let metadata = fs::symlink_metadata(&transactions)?;
            if !metadata.file_type().is_dir() {
                return Err(GardenError::Recovery(
                    "Publication directory is not a real directory.".into(),
                ));
            }
            let work = tempfile::tempdir()?;
            let index = Connection::open(work.path().join("transactions.sqlite"))?;
            index.execute_batch("CREATE TABLE entries (name TEXT PRIMARY KEY) WITHOUT ROWID")?;
            self.identity = Some((
                metadata.dev(),
                metadata.ino(),
                metadata.mtime(),
                metadata.mtime_nsec(),
            ));
            self.directory = Some(fs::read_dir(&transactions)?);
            self.index = Some((work, index));
        }
        if !self.discovered {
            let metadata = fs::symlink_metadata(&transactions)?;
            if !metadata.file_type().is_dir()
                || self.identity
                    != Some((
                        metadata.dev(),
                        metadata.ino(),
                        metadata.mtime(),
                        metadata.mtime_nsec(),
                    ))
            {
                return Err(GardenError::Recovery(
                    "Publication directory changed during discovery.".into(),
                ));
            }
            if let Some(entry) = self.directory.as_mut().unwrap().next() {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    return Err(GardenError::Recovery(
                        "Publication transaction is not a real directory.".into(),
                    ));
                }
                let name = entry.file_name().into_string().map_err(|_| {
                    GardenError::Recovery("Publication transaction has an invalid name.".into())
                })?;
                if self.history.as_ref().unwrap().contains(&name)? {
                    return Err(GardenError::Recovery("Publication exists both pending and archived; both copies remain preserved.".into()));
                }
                if !self.history.as_ref().unwrap().precedes(&name)? {
                    return Err(GardenError::Recovery("Pending publications precede archived history; all copies remain preserved.".into()));
                }
                self.index
                    .as_ref()
                    .unwrap()
                    .1
                    .execute("INSERT INTO entries(name) VALUES (?1)", [&name])?;
                self.discovered_count += 1;
                return Ok(false);
            }
            self.directory = None;
            self.discovered = true;
            return Ok(false);
        }
        if let Some(active) = self.active.as_mut() {
            if active.step(
                root,
                &self.generation,
                self.record_overlay.as_ref().unwrap(),
                self.verified_index.clone(),
            )? {
                self.after = active.name.clone();
                self.completed_count += 1;
                self.active = None;
            }
            return Ok(false);
        }
        let name: Option<String> = self
            .index
            .as_ref()
            .unwrap()
            .1
            .query_row(
                "SELECT name FROM entries WHERE name > ?1 ORDER BY name LIMIT 1",
                [&self.after],
                |row| row.get(0),
            )
            .optional()?;
        let Some(name) = name else {
            sync_directory(&transactions)?;
            return Ok(true);
        };
        let path = transactions.join(&name);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
            Ok(metadata) if metadata.file_type().is_dir() => {
                match fs::symlink_metadata(path.join("manifest.json")) {
                    Ok(metadata) if metadata.file_type().is_file() => {
                        self.active = Some(ActiveReplay::new(
                            root,
                            &path,
                            &name,
                            &self.generation,
                            self.verified_index.clone(),
                        )?);
                        return Ok(false);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                    Err(error) => return Err(error.into()),
                    Ok(_) => {
                        return Err(GardenError::Recovery(
                            "Publication manifest is not a regular file.".into(),
                        ))
                    }
                }
                archive_replayed_transaction(root, &self.generation, &name)?;
            }
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Publication transaction changed type.".into(),
                ))
            }
        }
        self.after = name;
        self.completed_count += 1;
        Ok(false)
    }
}

struct ActiveReplay {
    name: String,
    path: PathBuf,
    manifest: replay_manifest::Scanner,
    authority: Option<migration_backup::replay_authority::Session>,
    expected: Option<migration_backup::replay_authority::Expected>,
    scanned: bool,
    entry: u64,
    removal: u64,
    pending: Option<PendingReplayEntry>,
    scratch_cleaned: bool,
}
struct PendingReplayEntry {
    entry: replay_entry::Meta,
    authority: Option<migration_backup::replay_authority::Session>,
    expected: Option<migration_backup::replay_authority::Expected>,
    verifier: Option<migration_backup::replay_authority::FileVerifier>,
    verified: bool,
    large: Option<LargeReplay>,
    page_candidate: Option<page_identity::AuthenticatedCandidate>,
    page_resolver: Option<page_identity::Resolver>,
    page_destination: Option<Option<PathBuf>>,
}
struct LargeReplay {
    source: PathBuf,
    destination: PathBuf,
    journal: PathBuf,
    expected: migration_backup::replay_authority::Expected,
    base: Option<migration_backup::replay_authority::Expected>,
    verifier: Option<migration_backup::replay_authority::FileVerifier>,
    phase: u8,
    existing: bool,
}
impl LargeReplay {
    fn new(
        root: &Path,
        transaction: &Path,
        index: u64,
        generation: &str,
        entry: &replay_entry::Meta,
        expected: migration_backup::replay_authority::Expected,
    ) -> Result<Self> {
        let source = transaction
            .strip_prefix(root)
            .map_err(|_| {
                GardenError::Recovery("Publication transaction escaped the collection.".into())
            })?
            .join(&entry.staged);
        let destination = PathBuf::from(&entry.destination);
        validate_relative_collection_path(&source)?;
        validate_relative_collection_path(&destination)?;
        let key = hex::encode(Sha256::digest(serde_json::to_vec(&(
            source.clone(),
            index,
        ))?));
        let journal = PathBuf::from(".recovery/migration/replay-large")
            .join(generation)
            .join(format!("{key}.json"));
        let phase = match fs::symlink_metadata(root.join(&destination)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error.into()),
            Ok(metadata) if metadata.file_type().is_file() => 5,
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Large publication destination is not a regular file.".into(),
                ))
            }
        };
        Ok(Self {
            source,
            destination,
            journal,
            expected,
            base: entry
                .base_sha256
                .as_ref()
                .map(|digest| migration_backup::replay_authority::Expected::Legacy(digest.clone())),
            verifier: None,
            phase,
            existing: false,
        })
    }
    fn verify(&mut self, root: &Path, relative: &Path) -> Result<bool> {
        if self.verifier.is_none() {
            self.verifier = Some(migration_backup::replay_authority::FileVerifier::new(
                root,
                relative,
                self.expected.clone(),
            )?);
            return Ok(false);
        }
        if self.verifier.as_mut().unwrap().step()? {
            self.verifier = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn step(&mut self, root: &Path) -> Result<bool> {
        match self.phase {
            0 => {
                match fs::symlink_metadata(root.join(&self.journal)) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        migration_backup::begin_replay_copy(root, &self.journal, &self.source)?
                    }
                    Err(error) => return Err(error.into()),
                    Ok(metadata) if metadata.file_type().is_file() => (),
                    Ok(_) => {
                        return Err(GardenError::Recovery(
                            "Large publication copy journal is not a regular file.".into(),
                        ))
                    }
                }
                self.phase = 1;
                Ok(false)
            }
            1 => {
                if migration_backup::step_replay_copy(root, &self.journal, &self.source)? {
                    self.phase = 2;
                }
                Ok(false)
            }
            2 => {
                let copy = migration_backup::replay_copy_destination(root, &self.journal)?;
                if self.verify(root, &copy)? {
                    self.phase = 3;
                }
                Ok(false)
            }
            3 => {
                let source = self.source.clone();
                if self.verify(root, &source)? {
                    #[cfg(test)]
                    if std::env::var_os("KG_LARGE_REPLAY_STOP_BEFORE_LINK").is_some() {
                        std::process::exit(80);
                    }
                    self.phase = 4;
                }
                Ok(false)
            }
            4 => {
                let copy = migration_backup::replay_copy_destination(root, &self.journal)?;
                if self.existing {
                    swap_replay_copy(root, &copy, &self.destination)?;
                } else {
                    link_replay_copy(root, &copy, &self.destination)?;
                }
                #[cfg(test)]
                if std::env::var_os("KG_LARGE_REPLAY_STOP_AFTER_LINK").is_some() {
                    std::process::exit(81);
                }
                self.phase = 5;
                Ok(false)
            }
            5 => {
                let destination = self.destination.clone();
                if self.verifier.is_none() {
                    self.verifier = Some(migration_backup::replay_authority::FileVerifier::new(
                        root,
                        &destination,
                        self.expected.clone(),
                    )?);
                    return Ok(false);
                }
                let Some(matches) = self.verifier.as_mut().unwrap().step_match()? else {
                    return Ok(false);
                };
                self.verifier = None;
                if !matches {
                    self.phase = 6;
                    return Ok(false);
                }
                // If a crash followed the exclusive link, detach only the
                // matching private work link; the destination remains live.
                if fs::symlink_metadata(root.join(&self.journal)).is_ok() {
                    let copy = migration_backup::replay_copy_destination(root, &self.journal)?;
                    if replay_copy_is_distinct(root, &copy, &self.destination)? {
                        self.phase = 7;
                        return Ok(false);
                    }
                    detach_replay_copy(root, &copy, &self.destination)?;
                    remove_replay_journal(root, &self.journal)?;
                }
                Ok(true)
            }
            6 => {
                let base = self.base.clone().ok_or_else(|| GardenError::Recovery("Large publication collides with an external save; both versions remain preserved.".into()))?;
                if self.verifier.is_none() {
                    self.verifier = Some(migration_backup::replay_authority::FileVerifier::new(
                        root,
                        &self.destination,
                        base,
                    )?);
                    return Ok(false);
                }
                let Some(matches) = self.verifier.as_mut().unwrap().step_match()? else {
                    return Ok(false);
                };
                self.verifier = None;
                if !matches {
                    return Err(GardenError::Recovery("Large publication collides with an external save; both versions remain preserved.".into()));
                }
                self.existing = true;
                self.phase = 0;
                Ok(false)
            }
            7 => {
                let base = self.base.clone().ok_or_else(|| {
                    GardenError::Recovery(
                        "Displaced large publication remains preserved for recovery.".into(),
                    )
                })?;
                let copy = migration_backup::replay_copy_destination(root, &self.journal)?;
                if self.verifier.is_none() {
                    self.verifier = Some(migration_backup::replay_authority::FileVerifier::new(
                        root, &copy, base,
                    )?);
                    return Ok(false);
                }
                let Some(matches) = self.verifier.as_mut().unwrap().step_match()? else {
                    return Ok(false);
                };
                let identity = self.verifier.as_ref().unwrap().identity();
                self.verifier = None;
                if !matches {
                    return Err(GardenError::Recovery(
                        "A concurrent large external save remains preserved for recovery.".into(),
                    ));
                }
                remove_verified_replay_copy(root, &copy, identity)?;
                remove_replay_journal(root, &self.journal)?;
                Ok(true)
            }
            _ => Err(GardenError::Recovery(
                "Large publication phase is invalid.".into(),
            )),
        }
    }
}
fn replay_parent(
    root: &Path,
    relative: &Path,
    create: bool,
) -> Result<(rustix::fd::OwnedFd, std::ffi::OsString)> {
    use rustix::fs::{mkdirat, openat, Mode, OFlags, CWD};
    use rustix::io::Errno;
    validate_relative_collection_path(relative)?;
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut directory = openat(CWD, root, flags, Mode::empty()).map_err(std::io::Error::from)?;
    for part in relative.parent().unwrap().components() {
        let next = match openat(&directory, part.as_os_str(), flags, Mode::empty()) {
            Ok(next) => next,
            Err(Errno::NOENT) if create => {
                match mkdirat(
                    &directory,
                    part.as_os_str(),
                    Mode::from_bits_truncate(0o700),
                ) {
                    Ok(()) | Err(Errno::EXIST) => (),
                    Err(error) => return Err(std::io::Error::from(error).into()),
                }
                File::from(directory.try_clone()?).sync_all()?;
                openat(&directory, part.as_os_str(), flags, Mode::empty())
                    .map_err(std::io::Error::from)?
            }
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        directory = next;
    }
    Ok((directory, relative.file_name().unwrap().to_owned()))
}
fn link_replay_copy(root: &Path, copy: &Path, destination: &Path) -> Result<()> {
    use rustix::fs::{linkat, AtFlags};
    let (source_parent, source_name) = replay_parent(root, copy, false)?;
    let (destination_parent, destination_name) = replay_parent(root, destination, true)?;
    linkat(
        &source_parent,
        &source_name,
        &destination_parent,
        &destination_name,
        AtFlags::empty(),
    )
    .map_err(std::io::Error::from)?;
    File::from(destination_parent).sync_all()?;
    Ok(())
}
fn swap_replay_copy(root: &Path, copy: &Path, destination: &Path) -> Result<()> {
    use rustix::fs::{renameat_with, RenameFlags};
    let (source_parent, source_name) = replay_parent(root, copy, false)?;
    let (destination_parent, destination_name) = replay_parent(root, destination, false)?;
    renameat_with(
        &source_parent,
        &source_name,
        &destination_parent,
        &destination_name,
        RenameFlags::EXCHANGE,
    )
    .map_err(std::io::Error::from)?;
    File::from(source_parent).sync_all()?;
    File::from(destination_parent).sync_all()?;
    Ok(())
}
fn replay_copy_is_distinct(root: &Path, copy: &Path, destination: &Path) -> Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let destination = migration_backup::open_nofollow(root, destination)?.metadata()?;
    let copy = match migration_backup::open_nofollow(root, copy) {
        Ok(file) => file.metadata()?,
        Err(GardenError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(false)
        }
        Err(error) => return Err(error),
    };
    Ok((copy.dev(), copy.ino()) != (destination.dev(), destination.ino()))
}
fn remove_replay_journal(root: &Path, journal: &Path) -> Result<()> {
    use rustix::fs::{unlinkat, AtFlags};
    let (parent, name) = replay_parent(root, journal, false)?;
    match unlinkat(&parent, &name, AtFlags::empty()) {
        Ok(()) => File::from(parent).sync_all()?,
        Err(rustix::io::Errno::NOENT) => (),
        Err(error) => return Err(std::io::Error::from(error).into()),
    }
    Ok(())
}
fn remove_verified_replay_copy(
    root: &Path,
    copy: &Path,
    expected: (u64, u64, u64, i64, i64),
) -> Result<()> {
    use rustix::fs::{openat, unlinkat, AtFlags, Mode, OFlags};
    use std::os::unix::fs::MetadataExt;
    let (parent, name) = replay_parent(root, copy, false)?;
    let fd = openat(
        &parent,
        &name,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)?;
    let metadata = File::from(fd).metadata()?;
    if !metadata.is_file()
        || (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
        ) != expected
    {
        return Err(GardenError::Recovery(
            "Displaced publication changed before cleanup; it remains preserved.".into(),
        ));
    }
    unlinkat(&parent, &name, AtFlags::empty()).map_err(std::io::Error::from)?;
    File::from(parent).sync_all()?;
    Ok(())
}
fn detach_replay_copy(root: &Path, copy: &Path, destination: &Path) -> Result<()> {
    use rustix::fs::{unlinkat, AtFlags};
    use std::os::unix::fs::MetadataExt;
    let destination_file = migration_backup::open_nofollow(root, destination)?;
    let copy_file = match migration_backup::open_nofollow(root, copy) {
        Ok(file) => file,
        Err(GardenError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(())
        }
        Err(error) => return Err(error),
    };
    let a = destination_file.metadata()?;
    let b = copy_file.metadata()?;
    if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
        return Ok(());
    }
    let (parent, name) = replay_parent(root, copy, false)?;
    unlinkat(&parent, &name, AtFlags::empty()).map_err(std::io::Error::from)?;
    File::from(parent).sync_all()?;
    Ok(())
}
impl ActiveReplay {
    fn new(
        root: &Path,
        path: &Path,
        name: &str,
        generation: &str,
        verified: Option<std::sync::Arc<migration_backup::verified_index::VerifiedIndex>>,
    ) -> Result<Self> {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| {
                GardenError::Recovery("Publication transaction escaped the collection.".into())
            })?
            .join("manifest.json");
        Ok(Self {
            name: name.into(),
            path: path.into(),
            manifest: replay_manifest::Scanner::new(root, path, generation)?,
            authority: migration_backup::replay_authority::Session::new_with_index(
                root, &relative, verified,
            )?,
            expected: None,
            scanned: false,
            entry: 0,
            removal: 0,
            pending: None,
            scratch_cleaned: false,
        })
    }
    fn step(
        &mut self,
        root: &Path,
        generation: &str,
        record_overlay: &page_identity::RecordOverlay,
        verified: Option<std::sync::Arc<migration_backup::verified_index::VerifiedIndex>>,
    ) -> Result<bool> {
        if let Some(authority) = self.authority.as_mut() {
            if let Some(expected) = authority.step()? {
                self.expected = Some(expected);
                self.authority = None;
            }
            return Ok(false);
        }
        if !self.scanned {
            self.scanned = self.manifest.step()?;
            if self.scanned {
                self.manifest.verify_digest(self.expected.as_ref())?;
            }
            return Ok(false);
        }
        self.manifest.verify()?;
        if self.entry < self.manifest.entry_count() {
            if self.pending.is_none() {
                let entry = self.manifest.entry(self.entry)?;
                let relative = self
                    .path
                    .strip_prefix(root)
                    .map_err(|_| {
                        GardenError::Recovery(
                            "Publication transaction escaped the collection.".into(),
                        )
                    })?
                    .join(&entry.staged);
                let authority = migration_backup::replay_authority::Session::new_with_index(
                    root,
                    &relative,
                    verified.clone(),
                )?;
                let verified = authority.is_none();
                self.pending = Some(PendingReplayEntry {
                    entry,
                    authority,
                    expected: None,
                    verifier: None,
                    verified,
                    large: None,
                    page_candidate: None,
                    page_resolver: None,
                    page_destination: None,
                });
                return Ok(false);
            }
            let pending = self.pending.as_mut().unwrap();
            if let Some(authority) = pending.authority.as_mut() {
                if let Some(expected) = authority.step()? {
                    let relative = self
                        .path
                        .strip_prefix(root)
                        .map_err(|_| {
                            GardenError::Recovery(
                                "Publication transaction escaped the collection.".into(),
                            )
                        })?
                        .join(&pending.entry.staged);
                    pending.verifier = Some(migration_backup::replay_authority::FileVerifier::new(
                        root,
                        &relative,
                        expected.clone(),
                    )?);
                    pending.expected = Some(expected);
                    pending.authority = None;
                }
                return Ok(false);
            }
            if let Some(verifier) = pending.verifier.as_mut() {
                if verifier.step()? {
                    pending.verified = true;
                    pending.verifier = None;
                    #[cfg(test)]
                    if std::env::var_os("KG_REPLAY_STOP_AFTER_STAGE_VERIFY").is_some() {
                        std::process::exit(82);
                    }
                }
                return Ok(false);
            }
            if !pending.verified {
                return Err(GardenError::Recovery(
                    "Staged publication was not verified.".into(),
                ));
            }
            let staged_relative = self
                .path
                .strip_prefix(root)
                .map_err(|_| {
                    GardenError::Recovery("Publication transaction escaped the collection.".into())
                })?
                .join(&pending.entry.staged);
            let staged_size = migration_backup::open_nofollow(root, &staged_relative)?
                .metadata()?
                .len();
            let page_expected = pending.expected.clone().filter(|_| {
                Path::new(&pending.entry.destination).starts_with("pages")
                    && pending.page_destination.is_none()
            });
            if let Some(page_expected) = page_expected {
                if pending.page_resolver.is_none() {
                    if pending.page_candidate.is_none() {
                        pending.page_candidate = Some(page_identity::AuthenticatedCandidate::new(
                            root,
                            &staged_relative,
                            page_expected.clone(),
                        )?);
                        return Ok(false);
                    }
                    if let Some(id) = pending.page_candidate.as_mut().unwrap().step()? {
                        pending.page_candidate = None;
                        let mut resolver = page_identity::Resolver::new_with_index(
                            root,
                            id,
                            trusted_pages::active(root)?,
                            verified.clone(),
                        )?;
                        resolver.permit_published_output(
                            staged_relative.clone(),
                            page_expected.clone(),
                            pending.entry.destination.clone(),
                        );
                        pending.page_resolver = Some(resolver);
                    }
                    return Ok(false);
                }
                if let Some(destination) = pending
                    .page_resolver
                    .as_mut()
                    .unwrap()
                    .step_with_overlay(root, Some(record_overlay))?
                {
                    pending.page_destination = Some(destination);
                }
                return Ok(false);
            }
            if staged_size > 4 * 1024 * 1024 || pending.entry.base_length > 4 * 1024 * 1024 {
                if Path::new(&pending.entry.destination).starts_with("pages") {
                    let target = pending
                        .page_destination
                        .as_ref()
                        .and_then(|value| value.as_deref())
                        .unwrap_or_else(|| Path::new(&pending.entry.destination));
                    if target != Path::new(&pending.entry.destination) {
                        return Err(GardenError::Recovery(
                            "A large moved page remains preserved pending bounded link rebasing."
                                .into(),
                        ));
                    }
                }
                let expected = pending.expected.as_ref().ok_or_else(|| {
                    GardenError::Recovery(
                        "Large publication requires a verified recovery snapshot.".into(),
                    )
                })?;
                if pending.large.is_none() {
                    pending.large = Some(LargeReplay::new(
                        root,
                        &self.path,
                        self.entry,
                        generation,
                        &pending.entry,
                        expected.clone(),
                    )?);
                }
                if !pending.large.as_mut().unwrap().step(root)? {
                    return Ok(false);
                }
                self.pending = None;
                self.entry += 1;
                return Ok(false);
            }
            let pending = self.pending.take().unwrap();
            if matches!(pending.page_destination, Some(None))
                && (pending.entry.base_sha256.is_some()
                    || root.join(&pending.entry.destination).exists())
            {
                return Err(GardenError::Recovery(
                    "Publication page identity is absent from the verified snapshot.".into(),
                ));
            }
            let expected_base = if let Some(digest) = &pending.entry.base_sha256 {
                let bytes = self.manifest.base(self.entry, pending.entry.base_length)?;
                if bytes.len() as u64 != pending.entry.base_length {
                    return Err(GardenError::Recovery(
                        "Publication base length changed.".into(),
                    ));
                }
                migration_backup::replay_authority::Expected::Legacy(digest.clone())
                    .verify_bytes(&bytes)?;
                Some(bytes)
            } else {
                None
            };
            let resolved_page = pending.page_destination.as_ref().map(|value| {
                value
                    .clone()
                    .unwrap_or_else(|| PathBuf::from(&pending.entry.destination))
            });
            let record_destination = PathBuf::from(&pending.entry.destination);
            let record_stage = self
                .path
                .strip_prefix(root)
                .unwrap()
                .join(&pending.entry.staged);
            let entry = PublicationEntry {
                destination: pending.entry.destination,
                staged: pending.entry.staged,
                expected: expected_base,
            };
            let index = usize::try_from(self.entry).map_err(|_| {
                GardenError::Recovery("Publication entry index is too large.".into())
            })?;
            if !apply_publication_entry(
                root,
                &self.path,
                index,
                entry,
                None,
                pending.expected.as_ref(),
                resolved_page.as_deref(),
            )? {
                return Err(GardenError::Recovery(
                    "Publication remains pending because a page identity is ambiguous.".into(),
                ));
            }
            if record_destination.parent() == Some(Path::new(".recovery/trusted-pages/records")) {
                let expected = pending.expected.as_ref().ok_or_else(|| {
                    GardenError::Recovery(
                        "Trusted record replay requires snapshot evidence.".into(),
                    )
                })?;
                let staged = migration_backup::read_bounded_nofollow(root, &record_stage, 16384)?;
                expected.verify_bytes(&staged)?;
                let live =
                    migration_backup::read_bounded_nofollow(root, &record_destination, 16384)?;
                if live != staged {
                    return Err(GardenError::Recovery(
                        "Replayed trusted record differs from its authenticated output.".into(),
                    ));
                }
                record_overlay.put(&record_destination, &staged)?;
            }
            self.entry += 1;
            #[cfg(test)]
            if std::env::var_os("KG_REPLAY_STOP_AFTER_ENTRY").is_some()
                || std::env::var("KG_REPLAY_STOP_AFTER_ENTRY_INDEX")
                    .ok()
                    .and_then(|value| value.parse::<u64>().ok())
                    == Some(self.entry)
            {
                std::process::exit(84);
            }
            return Ok(false);
        }
        if self.removal < self.manifest.removal_count() {
            let removal = self.manifest.removal(self.removal)?;
            let relative = Path::new(&removal);
            validate_relative_collection_path(relative)?;
            // A replay can restart after the first removal, while an external
            // editor creates another save at the same path. Keep each version.
            retain_removed_entry(root, relative)?;
            self.removal += 1;
            #[cfg(test)]
            if std::env::var_os("KG_REPLAY_STOP_AFTER_REMOVAL").is_some() {
                std::process::exit(83);
            }
            return Ok(false);
        }
        if !self.scratch_cleaned {
            self.scratch_cleaned = self.manifest.cleanup_step()?;
            return Ok(false);
        }
        archive_replayed_transaction(root, generation, &self.name)?;
        #[cfg(test)]
        if std::env::var_os("KG_REPLAY_STOP_AFTER_ARCHIVE").is_some() {
            std::process::exit(85);
        }
        Ok(true)
    }
}

// Rename rather than recursively deleting staged bytes. A crash during cleanup
// must never leave a partly deleted transaction that cannot be replayed.
fn archive_replayed_transaction(root: &Path, replay_generation: &str, name: &str) -> Result<()> {
    use rustix::fs::{mkdirat, openat, renameat_with, Mode, OFlags, RenameFlags, CWD};
    use rustix::io::Errno;
    let journal = root.join(".recovery/migration/pre-recovery.json");
    let generation = match fs::symlink_metadata(&journal) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => "unassociated".to_owned(),
        Err(error) => return Err(error.into()),
        Ok(metadata) if metadata.file_type().is_file() => {
            let bytes = migration_backup::read_bounded_nofollow(
                root,
                Path::new(".recovery/migration/pre-recovery.json"),
                4096,
            )?;
            let id: String = serde_json::from_slice(&bytes)?;
            if !id.starts_with("backup-") || Path::new(&id).components().count() != 1 {
                return Err(GardenError::Recovery(
                    "Publication backup identity is invalid.".into(),
                ));
            }
            id
        }
        Ok(_) => {
            return Err(GardenError::Recovery(
                "Publication backup journal is not a regular file.".into(),
            ))
        }
    };
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let root_fd = openat(CWD, root, flags, Mode::empty()).map_err(std::io::Error::from)?;
    let staging =
        openat(&root_fd, ".staging", flags, Mode::empty()).map_err(std::io::Error::from)?;
    let transactions =
        openat(&staging, "transactions", flags, Mode::empty()).map_err(std::io::Error::from)?;
    {
        let (parent, name) = (&root_fd, ".recovery");
        match mkdirat(parent, name, Mode::from_bits_truncate(0o700)) {
            Ok(()) | Err(Errno::EXIST) => (),
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    let recovery =
        openat(&root_fd, ".recovery", flags, Mode::empty()).map_err(std::io::Error::from)?;
    match mkdirat(&recovery, "migration", Mode::from_bits_truncate(0o700)) {
        Ok(()) | Err(Errno::EXIST) => (),
        Err(error) => return Err(std::io::Error::from(error).into()),
    }
    let migration =
        openat(&recovery, "migration", flags, Mode::empty()).map_err(std::io::Error::from)?;
    match mkdirat(
        &migration,
        "publication-history",
        Mode::from_bits_truncate(0o700),
    ) {
        Ok(()) | Err(Errno::EXIST) => (),
        Err(error) => return Err(std::io::Error::from(error).into()),
    }
    let history = openat(&migration, "publication-history", flags, Mode::empty())
        .map_err(std::io::Error::from)?;
    match mkdirat(
        &history,
        generation.as_str(),
        Mode::from_bits_truncate(0o700),
    ) {
        Ok(()) | Err(Errno::EXIST) => (),
        Err(error) => return Err(std::io::Error::from(error).into()),
    }
    let generation_fd = openat(&history, generation.as_str(), flags, Mode::empty())
        .map_err(std::io::Error::from)?;
    match mkdirat(
        &generation_fd,
        replay_generation,
        Mode::from_bits_truncate(0o700),
    ) {
        Ok(()) | Err(Errno::EXIST) => (),
        Err(error) => return Err(std::io::Error::from(error).into()),
    }
    let replay_fd = openat(&generation_fd, replay_generation, flags, Mode::empty())
        .map_err(std::io::Error::from)?;
    renameat_with(
        &transactions,
        name,
        &replay_fd,
        name,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    File::from(transactions).sync_all()?;
    File::from(replay_fd).sync_all()?;
    File::from(generation_fd).sync_all()?;
    File::from(history).sync_all()?;
    File::from(migration).sync_all()?;
    File::from(recovery).sync_all()?;
    File::from(root_fd).sync_all()?;
    Ok(())
}

fn apply_publication(
    root: &Path,
    transaction: &Path,
    fail_after_files: Option<usize>,
) -> Result<bool> {
    let relative = transaction.strip_prefix(root).map_err(|_| {
        GardenError::Recovery("Publication transaction escaped the collection.".into())
    })?;
    let manifest: PublicationManifest = serde_json::from_reader(std::io::BufReader::new(
        migration_backup::open_nofollow(root, &relative.join("manifest.json"))?,
    ))?;
    if fail_after_files == Some(0) {
        return interrupted();
    }
    let mut complete = true;
    let removals = manifest.removals;
    for (index, entry) in manifest.entries.into_iter().enumerate() {
        if !apply_publication_entry(
            root,
            transaction,
            index,
            entry,
            fail_after_files,
            None,
            None,
        )? {
            complete = false;
        }
    }
    // A retained publication keeps its removals until every page identity resolves.
    if complete {
        for removal in removals {
            let relative = Path::new(&removal);
            validate_relative_collection_path(relative)?;
            if trusted_pages::active(root)?
                && (relative.starts_with("sources") || relative.starts_with("pages"))
            {
                retain_removed_entry(root, relative)?;
            } else {
                remove_collection_entry(root, relative)?;
            }
            if fail_after_files == Some(usize::MAX - 1) {
                return interrupted();
            }
        }
    }
    Ok(complete)
}

fn apply_publication_entry(
    root: &Path,
    transaction: &Path,
    index: usize,
    entry: PublicationEntry,
    fail_after_files: Option<usize>,
    expected_staged: Option<&migration_backup::replay_authority::Expected>,
    resolved: Option<&Path>,
) -> Result<bool> {
    validate_relative_collection_path(Path::new(&entry.destination))?;
    validate_relative_staging_path(Path::new(&entry.staged))?;
    let done = transaction.join(format!("files/{index:04}.done"));
    let mut proposed = Vec::new();
    let relative = transaction.strip_prefix(root).map_err(|_| {
        GardenError::Recovery("Publication transaction escaped the collection.".into())
    })?;
    migration_backup::open_nofollow(root, &relative.join(&entry.staged))?
        .read_to_end(&mut proposed)?;
    if let Some(expected) = expected_staged {
        expected.verify_bytes(&proposed)?;
    }
    let destination = if let Some(relative) = resolved {
        validate_relative_collection_path(relative)?;
        root.join(relative)
    } else {
        let Some(destination) = publication_destination(root, &entry, &proposed)? else {
            return Ok(false);
        };
        destination
    };
    let mut ancestor = destination.as_path();
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or_else(|| {
            GardenError::Invalid("A publication path has no collection ancestor.".into())
        })?;
    }
    if !fs::canonicalize(ancestor)?.starts_with(root) {
        return Err(GardenError::Invalid(
            "A publication destination escaped the collection through a link.".into(),
        ));
    }
    if done.exists() && fs::read(&destination).ok() == fs::read(&done).ok() {
        return Ok(true);
    }
    let proposed = if let Ok(text) = std::str::from_utf8(&proposed) {
        reconciliation::rebase_links(
            &entry.destination,
            &destination.strip_prefix(root).unwrap().to_string_lossy(),
            text,
        )
        .into_bytes()
    } else {
        proposed
    };
    let held = transaction.join(format!("files/{index:04}.held"));
    let mut desired = proposed.clone();
    // A crash after exchange leaves the displaced save here, durably named
    // by the manifest's entry index, before any cleanup can occur.
    if held.exists() {
        let displaced = fs::read(&held)?;
        if displaced != proposed && Some(&displaced) != entry.expected.as_ref() {
            desired = merge_owner_save(root, &desired, entry.expected.as_deref(), &displaced)?;
        }
    }
    let mut applied = false;
    for attempt in 0..8 {
        let current = fs::read(&destination).ok();
        if current.as_ref() == Some(&desired) {
            applied = true;
            break;
        }
        if let Some(current) = current
            .as_ref()
            .filter(|current| Some(*current) != entry.expected.as_ref())
        {
            desired = merge_owner_save(root, &desired, entry.expected.as_deref(), current)?;
        }
        // Persist each displaced save before reusing the exchange slot.
        if held.exists() {
            let displaced = fs::read(&held)?;
            if displaced != proposed && Some(&displaced) != entry.expected.as_ref() {
                retain_owner_save(root, &displaced)?;
            }
        }
        write_atomic(&held, &desired)?;
        fs::create_dir_all(destination.parent().unwrap())?;
        match exchange(&held, &destination) {
            Ok(()) => {
                sync_directory(held.parent().unwrap())?;
                sync_directory(destination.parent().unwrap())?;
                if fail_after_files == Some(usize::MAX) {
                    return interrupted();
                }
                let displaced = fs::read(&held)?;
                if Some(&displaced) != current.as_ref() {
                    desired =
                        merge_owner_save(root, &desired, entry.expected.as_deref(), &displaced)?;
                    if attempt == 7 {
                        return Err(GardenError::Invalid("External saves are still arriving; recoverable publication will retry automatically.".into()));
                    }
                    continue;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && current.is_none() => {
                // Hard-link creation is exclusive: a concurrent new file is
                // never overwritten by the missing-destination path.
                match fs::hard_link(&held, &destination) {
                    Ok(()) => sync_directory(destination.parent().unwrap())?,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => return Err(error.into()),
                }
            }
            Err(error) => return Err(error.into()),
        }
        applied = true;
        break;
    }
    if !applied {
        return Err(GardenError::Invalid("External saves are still arriving; publication remains pending for automatic recovery.".into()));
    }
    write_atomic(&done, &desired)?;
    if fail_after_files == Some(index + 1) {
        return interrupted();
    }
    Ok(true)
}

fn retain_removed_entry(root: &Path, relative: &Path) -> Result<()> {
    use rustix::fs::{mkdirat, openat, renameat_with, Mode, OFlags, RenameFlags};
    use rustix::io::Errno;
    validate_relative_collection_path(relative)?;
    let archive = PathBuf::from(".recovery/withdrawn-live").join(Uuid::new_v4().to_string());
    let (recovery, name) = replay_parent(root, &archive, true)?;
    mkdirat(&recovery, &name, Mode::from_bits_truncate(0o700)).map_err(std::io::Error::from)?;
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let directory = openat(&recovery, &name, flags, Mode::empty()).map_err(std::io::Error::from)?;
    let fd = openat(
        &directory,
        "path.json",
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_bits_truncate(0o600),
    )
    .map_err(std::io::Error::from)?;
    let mut file = File::from(fd);
    file.write_all(&serde_json::to_vec(relative)?)?;
    file.sync_all()?;
    File::from(directory.try_clone()?).sync_all()?;
    File::from(recovery).sync_all()?;
    match replay_parent(root, relative, false) {
        Ok((source, name)) => {
            match renameat_with(
                &source,
                &name,
                &directory,
                "content",
                RenameFlags::NOREPLACE,
            ) {
                Ok(()) => {
                    File::from(source).sync_all()?;
                }
                Err(Errno::NOENT) => (),
                Err(error) => return Err(std::io::Error::from(error).into()),
            }
        }
        Err(GardenError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
    File::from(directory).sync_all()?;
    Ok(())
}

fn publication_destination(
    root: &Path,
    entry: &PublicationEntry,
    proposed: &[u8],
) -> Result<Option<PathBuf>> {
    let fallback = root.join(&entry.destination);
    if !entry.destination.starts_with("pages/") {
        return Ok(Some(fallback));
    }
    let Some(header) = std::str::from_utf8(proposed)
        .ok()
        .and_then(|text| parse_knowledge_header(text).ok())
    else {
        return Ok(Some(fallback));
    };
    if trusted_pages::active(root)? {
        let matches = trusted_pages::knowledge_paths(root)?
            .into_iter()
            .filter(|path| {
                read_knowledge_header(path)
                    .is_ok_and(|candidate| candidate.page_id == header.page_id)
            })
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [path] => Ok(Some(path.clone())),
            [] if entry.expected.is_none() && !fallback.exists() => Ok(Some(fallback)),
            _ => Ok(None),
        };
    }
    let mut directories = vec![root.join("pages")];
    let mut matches = Vec::new();
    while let Some(directory) = directories.pop() {
        if !directory.is_dir() {
            continue;
        }
        for item in fs::read_dir(directory)? {
            let item = item?;
            let kind = item.file_type()?;
            if kind.is_dir() {
                directories.push(item.path());
            } else if kind.is_file()
                && item.path().extension().is_some_and(|ext| ext == "md")
                && read_knowledge_header(&item.path())
                    .is_ok_and(|candidate| candidate.page_id == header.page_id)
            {
                matches.push(item.path());
            }
        }
    }
    match matches.as_slice() {
        [path] => Ok(Some(path.clone())),
        [] if entry.expected.is_none() && !fallback.exists() => Ok(Some(fallback)),
        _ => Ok(None),
    }
}

fn interrupted<T>() -> Result<T> {
    Err(GardenError::Io(std::io::Error::new(
        std::io::ErrorKind::Interrupted,
        "Injected publication interruption after an atomic file replacement.",
    )))
}

fn retain_owner_save(root: &Path, bytes: &[u8]) -> Result<String> {
    let path = format!("owner-edits/{}.md", hex::encode(Sha256::digest(bytes)));
    if !root.join(&path).exists() {
        write_atomic(&root.join(&path), bytes)?;
    }
    Ok(path)
}

/// Preserve every conflicting save verbatim. Supported fact edits carry their
/// authority into the new source generation; all other differences stay explicit.
fn merge_owner_save(
    root: &Path,
    proposed: &[u8],
    base: Option<&[u8]>,
    owner: &[u8],
) -> Result<Vec<u8>> {
    let retained = retain_owner_save(root, owner)?;
    if trusted_pages::active(root)? {
        return Ok(proposed.to_vec());
    }
    let (Ok(proposed_text), Ok(owner_text)) =
        (std::str::from_utf8(proposed), std::str::from_utf8(owner))
    else {
        return Ok(owner.to_vec());
    };
    let Ok(mut next) = parse_knowledge_header(proposed_text) else {
        return merge_source_save(proposed_text, base, owner_text, &retained)
            .map(|value| value.unwrap_or_else(|| owner.to_vec()));
    };
    let Ok(previous) = parse_knowledge_header(owner_text) else {
        return Ok(owner.to_vec());
    };
    if previous.page_id != next.page_id {
        return Ok(owner.to_vec());
    }
    let baseline = base
        .and_then(|b| std::str::from_utf8(b).ok())
        .and_then(|s| parse_knowledge_header(s).ok());
    let mut body = proposed_text
        .split_once("\n---\n")
        .map(|(_, b)| b.to_owned())
        .unwrap_or_default();
    for fact in &previous.facts {
        let mut corrected = fact.manual_correction.as_ref().map(|c| c.value.clone());
        let lines: Vec<_> = owner_text.lines().collect();
        for pair in lines.windows(2) {
            if pair[1] != format!("  - Fact identity: `{}`", fact.fact_id) {
                continue;
            }
            let prefix = format!(
                "- **{}:** ",
                escape_markdown(&fact.property.replace('_', " "))
            );
            let suffix = fact
                .qualifier
                .as_deref()
                .map(|q| format!(" ({})", escape_markdown(q)))
                .unwrap_or_default();
            if let Some(value) = pair[0]
                .strip_prefix(&prefix)
                .and_then(|v| v.strip_suffix(&suffix))
                .and_then(decode_generated_fact_value)
            {
                if value != fact.value {
                    corrected = Some(value);
                }
            }
        }
        if let Some(value) = corrected {
            if let Some(updated) = next.facts.iter_mut().find(|f| f.fact_id == fact.fact_id) {
                let before = format!(
                    "- **{}:** {}",
                    escape_markdown(&updated.property.replace('_', " ")),
                    escape_markdown(&updated.value)
                );
                let after = format!(
                    "- **{}:** {}",
                    escape_markdown(&updated.property.replace('_', " ")),
                    escape_markdown(&value)
                );
                let identity = format!("\n  - Fact identity: `{}`", updated.fact_id);
                let suffix = updated
                    .qualifier
                    .as_deref()
                    .map(|q| format!(" ({})", escape_markdown(q)))
                    .unwrap_or_default();
                // Match the complete generated row and its durable identity;
                // equal values and value prefixes may belong to other facts.
                body = body.replace(
                    &format!("{before}{suffix}{identity}"),
                    &format!("{after}{suffix}{identity}"),
                );
                updated.value = value.clone();
                updated.manual_correction = Some(ManualFactCorrection { value });
                updated.origin = "manual_correction".into();
            } else {
                let mut corrected_fact = fact.clone();
                corrected_fact.value = value.clone();
                corrected_fact.manual_correction = Some(ManualFactCorrection { value });
                corrected_fact.origin = "manual_correction".into();
                body.push_str(&format!(
                    "\n- **{}:** {}{}\n  - Fact identity: `{}`\n",
                    escape_markdown(&corrected_fact.property.replace('_', " ")),
                    escape_markdown(&corrected_fact.value),
                    corrected_fact
                        .qualifier
                        .as_deref()
                        .map(|q| format!(" ({})", escape_markdown(q)))
                        .unwrap_or_default(),
                    corrected_fact.fact_id
                ));
                next.facts.push(corrected_fact);
            }
        }
    }
    if baseline
        .as_ref()
        .is_none_or(|base| base.title != previous.title)
    {
        next.title = previous.title.clone();
    }
    if let Some(base_text) = base.and_then(|b| std::str::from_utf8(b).ok()) {
        let baseline_lines = base_text.lines().collect::<HashSet<_>>();
        let owner_body = owner_text
            .split_once("\n---\n")
            .map(|(_, b)| b)
            .unwrap_or_default();
        for line in owner_body
            .lines()
            .filter(|line| !line.trim().is_empty() && !baseline_lines.contains(line))
        {
            if !line.starts_with(['-', ' ', '#', '|', '>'])
                && !next.owner_content.iter().any(|existing| existing == line)
            {
                next.owner_content.push(line.to_owned());
            }
        }
    }
    next.owner_metadata.extend(previous.owner_metadata);
    next.owner_content.extend(previous.owner_content);
    next.owner_content.sort();
    next.owner_content.dedup();
    next.uninterpreted_owner_edits
        .extend(previous.uninterpreted_owner_edits);
    next.uninterpreted_owner_edits.push(retained);
    next.uninterpreted_owner_edits.sort();
    next.uninterpreted_owner_edits.dedup();
    next.external_edit_status = Some("uncertain".into());
    // Retain owner prose visibly without promoting old generated rows to facts.
    body.push_str("\n## Concurrent owner save\n\nThe exact owner save is retained with uncertainty; supported fact corrections remain authoritative.\n");
    Ok(format!("---\n{}---\n{body}", serde_yaml_ng::to_string(&next)?).into_bytes())
}

fn merge_source_save(
    proposed: &str,
    base: Option<&[u8]>,
    owner: &str,
    retained: &str,
) -> Result<Option<Vec<u8>>> {
    let parse = |text: &str| -> Option<(SourceInfo, String)> {
        let (yaml, body) = text.strip_prefix("---\n")?.split_once("\n---\n")?;
        Some((serde_yaml_ng::from_str(yaml).ok()?, body.to_owned()))
    };
    let (Some((mut next, mut body)), Some((previous, owner_body))) =
        (parse(proposed), parse(owner))
    else {
        return Ok(None);
    };
    if next.source_id != previous.source_id {
        return Ok(None);
    }
    if let Some((baseline, baseline_body)) = base
        .and_then(|b| std::str::from_utf8(b).ok())
        .and_then(parse)
    {
        if previous.title != baseline.title {
            next.title = previous.title;
        }
        let lines = baseline_body.lines().collect::<HashSet<_>>();
        let additions = owner_body
            .lines()
            .filter(|line| !lines.contains(line))
            .collect::<Vec<_>>()
            .join("\n");
        if !additions.trim().is_empty() {
            body.push_str(&format!("\n## Owner notes\n\n{additions}\n"));
        }
    }
    next.owner_metadata.extend(previous.owner_metadata);
    next.owner_metadata.insert(
        "external_edit_status".into(),
        serde_yaml_ng::Value::String("uncertain".into()),
    );
    next.owner_metadata.insert(
        "concurrent_owner_save".into(),
        serde_yaml_ng::Value::String(retained.into()),
    );
    Ok(Some(serialize_page(&next, &body)?.into_bytes()))
}

#[cfg(target_os = "macos")]
fn exchange(left: &Path, right: &Path) -> std::io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    unsafe extern "C" {
        fn renamex_np(
            from: *const std::ffi::c_char,
            to: *const std::ffi::c_char,
            flags: u32,
        ) -> i32;
    }
    let left = CString::new(left.as_os_str().as_bytes())?;
    let right = CString::new(right.as_os_str().as_bytes())?;
    // RENAME_SWAP, declared by the macOS SDK's sys/stdio.h. Both paths are
    // synchronized ordinary files on the same collection volume.
    if unsafe { renamex_np(left.as_ptr(), right.as_ptr(), 2) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "macos"))]
fn exchange(_left: &Path, _right: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Conflict-safe publication currently requires the macOS release target.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_original_recovers_before_metadata_and_refuses_symlink_parent() {
        use std::os::unix::fs::symlink;
        let workspace = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(workspace.path()).unwrap();
        let directory = root.join("sources/a");
        fs::create_dir_all(&directory).unwrap();
        let candidate = root.join("candidate");
        let content = vec![b'x'; 3 * 1024 * 1024 + 1];
        fs::write(&candidate, &content).unwrap();
        let destination = directory.join("original-generation.txt");
        let metadata = directory.join("index.md");
        let make = || OriginalPublication {
            destination: destination.clone(),
            source: candidate.clone(),
            proposed_digest: hex::encode(Sha256::digest(&content)),
        };
        assert!(commit_publication_with_originals(
            &root,
            vec![(metadata.clone(), b"new metadata".to_vec())],
            vec![make()],
            &HashMap::new(),
            Some(usize::MAX)
        )
        .is_err());
        assert!(!metadata.exists());
        recover_publications(&root).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), content);
        assert_eq!(fs::read(&metadata).unwrap(), b"new metadata");
        fs::rename(&directory, root.join("sources/moved")).unwrap();
        let outside = root.join("outside");
        fs::create_dir(&outside).unwrap();
        symlink(&outside, &directory).unwrap();
        assert!(commit_publication_with_originals(
            &root,
            vec![],
            vec![make()],
            &HashMap::new(),
            None
        )
        .is_err());
        assert!(!outside.join("original-generation.txt").exists());
    }

    #[test]
    fn original_new_destination_does_not_overwrite_existing_file() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("sources/a/original.txt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"external").unwrap();
        let candidate = root.path().join("candidate");
        fs::write(&candidate, b"new").unwrap();
        let original = OriginalPublication {
            destination: path.clone(),
            source: candidate,
            proposed_digest: hex::encode(Sha256::digest(b"new")),
        };
        assert!(commit_publication_with_originals(
            root.path(),
            vec![],
            vec![original],
            &HashMap::new(),
            None
        )
        .is_err());
        assert_eq!(fs::read(path).unwrap(), b"external");
    }

    #[test]
    fn moved_page_replay_restarts_and_uses_verified_snapshot_identity() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let moved = root.join("pages/new/page.md");
        fs::create_dir_all(moved.parent().unwrap()).unwrap();
        let original = b"---\npage_id: page-1\n---\nold".to_vec();
        fs::write(&moved, &original).unwrap();
        let transaction = root.join(".staging/transactions/pending");
        fs::create_dir_all(transaction.join("files")).unwrap();
        let proposed = b"---\npage_id: page-1\n---\nnew";
        fs::write(transaction.join("files/0000"), proposed).unwrap();
        let manifest = PublicationManifest {
            entries: vec![PublicationEntry {
                destination: "pages/old/page.md".into(),
                staged: "files/0000".into(),
                expected: Some(original),
            }],
            removals: vec![],
        };
        fs::write(
            transaction.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        migration_backup::prepare_pre_recovery(&root).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::publication::tests::recovery_cursor_child",
            ])
            .env("KG_PUBLICATION_REPLAY_FIXTURE", &root)
            .env("KG_REPLAY_STOP_AFTER_ENTRY", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(84));
        assert_eq!(fs::read(&moved).unwrap(), proposed);
        let mut replay = RecoverySession::new();
        let mut done = false;
        for _ in 0..10000 {
            if replay.step(&root).unwrap() {
                done = true;
                break;
            }
        }
        assert!(done);
        assert_eq!(fs::read(&moved).unwrap(), proposed);
        assert!(!root.join("pages/old/page.md").exists());
    }
    #[test]
    fn new_page_replay_restarts_after_publication() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        fs::create_dir_all(root.join("pages")).unwrap();
        let transaction = root.join(".staging/transactions/pending");
        fs::create_dir_all(transaction.join("files")).unwrap();
        let proposed = b"---\npage_id: new-page\n---\nnew";
        fs::write(transaction.join("files/0000"), proposed).unwrap();
        let manifest = PublicationManifest {
            entries: vec![PublicationEntry {
                destination: "pages/new.md".into(),
                staged: "files/0000".into(),
                expected: None,
            }],
            removals: vec![],
        };
        fs::write(
            transaction.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        migration_backup::prepare_pre_recovery(&root).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::publication::tests::recovery_cursor_child",
            ])
            .env("KG_PUBLICATION_REPLAY_FIXTURE", &root)
            .env("KG_REPLAY_STOP_AFTER_ENTRY", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(84));
        let mut replay = RecoverySession::new();
        let mut done = false;
        for _ in 0..10000 {
            if replay.step(&root).unwrap() {
                done = true;
                break;
            }
        }
        assert!(done);
        assert_eq!(fs::read(root.join("pages/new.md")).unwrap(), proposed);
    }
    #[test]
    fn trusted_record_overlay_resumes_after_record_publication() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        fs::create_dir_all(root.join("pages")).unwrap();
        let page = root.join("pages/page.md");
        let original = b"---\npage_id: page-1\n---\nold".to_vec();
        let proposed = b"---\npage_id: page-1\n---\nnew".to_vec();
        fs::write(&page, &original).unwrap();
        trusted_pages::record(&root, Path::new("pages/page.md"), &original).unwrap();
        fs::create_dir_all(root.join(".recovery/migration")).unwrap();
        fs::write(
            root.join(".recovery/migration/state.json"),
            br#"{"phase":"authority_retired","backup":"backup-test"}"#,
        )
        .unwrap();
        let record_name = hex::encode(Sha256::digest(b"pages/page.md"));
        let record = format!(".recovery/trusted-pages/records/{record_name}");
        let old_record = fs::read(root.join(&record)).unwrap();
        let blob_name = hex::encode(Sha256::digest(&proposed));
        let blob = format!(".recovery/trusted-pages/blobs/{blob_name}");
        let new_record =
            serde_json::to_vec(&serde_json::json!({"path":"pages/page.md", "sha256":blob_name}))
                .unwrap();
        let transaction = root.join(".staging/transactions/pending");
        fs::create_dir_all(transaction.join("files")).unwrap();
        for (index, bytes) in [
            proposed.as_slice(),
            new_record.as_slice(),
            proposed.as_slice(),
        ]
        .into_iter()
        .enumerate()
        {
            fs::write(transaction.join(format!("files/{index:04}")), bytes).unwrap();
        }
        let entries = vec![
            PublicationEntry {
                destination: blob,
                staged: "files/0000".into(),
                expected: None,
            },
            PublicationEntry {
                destination: record,
                staged: "files/0001".into(),
                expected: Some(old_record),
            },
            PublicationEntry {
                destination: "pages/page.md".into(),
                staged: "files/0002".into(),
                expected: Some(original),
            },
        ];
        fs::write(
            transaction.join("manifest.json"),
            serde_json::to_vec(&PublicationManifest {
                entries,
                removals: vec![],
            })
            .unwrap(),
        )
        .unwrap();
        migration_backup::prepare_pre_recovery(&root).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::publication::tests::recovery_cursor_child",
            ])
            .env("KG_PUBLICATION_REPLAY_FIXTURE", &root)
            .env("KG_REPLAY_STOP_AFTER_ENTRY_INDEX", "2")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(84));
        let mut replay = RecoverySession::new();
        let mut done = false;
        for _ in 0..10000 {
            if replay.step(&root).unwrap() {
                done = true;
                break;
            }
        }
        assert!(done);
        assert_eq!(fs::read(&page).unwrap(), proposed);
    }
    #[test]
    fn archived_record_overlay_rebuilds_after_process_exit() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        fs::create_dir_all(root.join("pages")).unwrap();
        let page = root.join("pages/page.md");
        let original = b"---\npage_id: page-1\n---\nold".to_vec();
        let proposed = b"---\npage_id: page-1\n---\nnew".to_vec();
        fs::write(&page, &original).unwrap();
        trusted_pages::record(&root, Path::new("pages/page.md"), &original).unwrap();
        fs::create_dir_all(root.join(".recovery/migration")).unwrap();
        fs::write(
            root.join(".recovery/migration/state.json"),
            br#"{"phase":"authority_retired","backup":"backup-test"}"#,
        )
        .unwrap();
        let record_name = hex::encode(Sha256::digest(b"pages/page.md"));
        let record = format!(".recovery/trusted-pages/records/{record_name}");
        let old_record = fs::read(root.join(&record)).unwrap();
        let blob_name = hex::encode(Sha256::digest(&proposed));
        let blob = format!(".recovery/trusted-pages/blobs/{blob_name}");
        let new_record =
            serde_json::to_vec(&serde_json::json!({"path":"pages/page.md", "sha256":blob_name}))
                .unwrap();
        let first = root.join(".staging/transactions/a");
        let second = root.join(".staging/transactions/b");
        fs::create_dir_all(first.join("files")).unwrap();
        fs::create_dir_all(second.join("files")).unwrap();
        fs::write(first.join("files/0000"), &proposed).unwrap();
        fs::write(first.join("files/0001"), &new_record).unwrap();
        fs::write(second.join("files/0000"), &proposed).unwrap();
        let first_entries = vec![
            PublicationEntry {
                destination: blob,
                staged: "files/0000".into(),
                expected: None,
            },
            PublicationEntry {
                destination: record,
                staged: "files/0001".into(),
                expected: Some(old_record),
            },
        ];
        let second_entries = vec![PublicationEntry {
            destination: "pages/page.md".into(),
            staged: "files/0000".into(),
            expected: Some(original),
        }];
        for (directory, entries) in [(&first, first_entries), (&second, second_entries)] {
            fs::write(
                directory.join("manifest.json"),
                serde_json::to_vec(&PublicationManifest {
                    entries,
                    removals: vec![],
                })
                .unwrap(),
            )
            .unwrap();
        }
        migration_backup::prepare_pre_recovery(&root).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::publication::tests::recovery_cursor_child",
            ])
            .env("KG_PUBLICATION_REPLAY_FIXTURE", &root)
            .env("KG_REPLAY_STOP_AFTER_ARCHIVE", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(85));
        assert!(!first.exists());
        let backup: String = serde_json::from_slice(
            &fs::read(root.join(".recovery/migration/pre-recovery.json")).unwrap(),
        )
        .unwrap();
        let archived_record = root
            .join(".recovery/migration/publication-history")
            .join(backup)
            .join("pre-work-unassociated/a/files/0001");
        fs::write(&archived_record, b"substituted").unwrap();
        let mut rejected = RecoverySession::new();
        let mut blocked = false;
        for _ in 0..30000 {
            match rejected.step(&root) {
                Err(_) => {
                    blocked = true;
                    break;
                }
                Ok(true) => break,
                Ok(false) => (),
            }
        }
        assert!(blocked);
        assert_eq!(fs::read(&page).unwrap(), b"---\npage_id: page-1\n---\nold");
        fs::write(&archived_record, &new_record).unwrap();
        let receipt = archived_record.with_file_name("0001.done");
        fs::write(&receipt, b"forged receipt").unwrap();
        let mut forged = RecoverySession::new();
        let mut blocked = false;
        for _ in 0..30000 {
            match forged.step(&root) {
                Err(_) => {
                    blocked = true;
                    break;
                }
                Ok(true) => break,
                Ok(false) => (),
            }
        }
        assert!(blocked);
        assert_eq!(fs::read(&page).unwrap(), b"---\npage_id: page-1\n---\nold");
        fs::write(&receipt, &new_record).unwrap();
        let live_record = root
            .join(".recovery/trusted-pages/records")
            .join(&record_name);
        fs::write(&live_record, b"external").unwrap();
        let mut conflict = RecoverySession::new();
        let mut blocked = false;
        for _ in 0..30000 {
            match conflict.step(&root) {
                Err(_) => {
                    blocked = true;
                    break;
                }
                Ok(true) => break,
                Ok(false) => (),
            }
        }
        assert!(blocked);
        assert_eq!(fs::read(&live_record).unwrap(), b"external");
        assert_eq!(fs::read(&page).unwrap(), b"---\npage_id: page-1\n---\nold");
        fs::write(&live_record, &new_record).unwrap();
        let mut replay = RecoverySession::new();
        let mut done = false;
        for _ in 0..30000 {
            if replay.step(&root).unwrap() {
                done = true;
                break;
            }
        }
        assert!(done);
        assert_eq!(fs::read(&page).unwrap(), proposed);
    }
    fn large_replay_fixture(root: &Path) -> PathBuf {
        fs::create_dir_all(root.join("notes")).unwrap();
        let target = root.join("notes/large.bin");
        assert!(commit_publication(
            root,
            vec![(target, vec![11; 5 * 1024 * 1024])],
            &HashMap::new(),
            Some(0)
        )
        .is_err());
        let saved = migration_backup::backup(root).unwrap();
        let id = saved.file_name().unwrap().to_str().unwrap();
        fs::write(
            root.join(".recovery/migration/pre-recovery.json"),
            serde_json::to_vec(id).unwrap(),
        )
        .unwrap();
        fs::read_dir(root.join(".staging/transactions"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
            .join("files/0000")
    }
    fn large_existing_fixture(root: &Path) -> PathBuf {
        fs::create_dir_all(root.join("notes")).unwrap();
        let target = root.join("notes/large.bin");
        let original = vec![10; 5 * 1024 * 1024];
        fs::write(&target, &original).unwrap();
        assert!(commit_publication(
            root,
            vec![(target.clone(), vec![11; 5 * 1024 * 1024])],
            &HashMap::from([(target.clone(), original)]),
            Some(0)
        )
        .is_err());
        let saved = migration_backup::backup(root).unwrap();
        let id = saved.file_name().unwrap().to_str().unwrap();
        fs::write(
            root.join(".recovery/migration/pre-recovery.json"),
            serde_json::to_vec(id).unwrap(),
        )
        .unwrap();
        target
    }
    #[test]
    fn large_existing_publication_resumes_after_atomic_exchange() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let target = large_existing_fixture(&root);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::publication::tests::recovery_cursor_child",
            ])
            .env("KG_PUBLICATION_REPLAY_FIXTURE", &root)
            .env("KG_LARGE_REPLAY_STOP_AFTER_LINK", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(81));
        let mut replay = RecoverySession::new();
        let mut complete = false;
        for _ in 0..10000 {
            if replay.step(&root).unwrap() {
                complete = true;
                break;
            }
        }
        assert!(complete);
        assert_eq!(fs::read(target).unwrap(), vec![11; 5 * 1024 * 1024]);
    }
    #[test]
    fn large_existing_publication_preserves_external_save_after_interruption() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let target = large_existing_fixture(&root);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::publication::tests::recovery_cursor_child",
            ])
            .env("KG_PUBLICATION_REPLAY_FIXTURE", &root)
            .env("KG_LARGE_REPLAY_STOP_BEFORE_LINK", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(80));
        fs::write(&target, vec![12; 5 * 1024 * 1024]).unwrap();
        let mut replay = RecoverySession::new();
        let mut rejected = false;
        for _ in 0..10000 {
            match replay.step(&root) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(true) => break,
                Ok(false) => (),
            }
        }
        assert!(rejected);
        assert_eq!(fs::read(target).unwrap(), vec![12; 5 * 1024 * 1024]);
    }
    #[test]
    fn large_new_publication_restarts_after_link_and_detaches_work_copy() {
        use std::os::unix::fs::MetadataExt;
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        large_replay_fixture(&root);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::publication::tests::recovery_cursor_child",
            ])
            .env("KG_PUBLICATION_REPLAY_FIXTURE", &root)
            .env("KG_LARGE_REPLAY_STOP_AFTER_LINK", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(81));
        let mut replay = RecoverySession::new();
        let mut complete = false;
        for _ in 0..10000 {
            if replay.step(&root).unwrap() {
                complete = true;
                break;
            }
        }
        assert!(complete);
        let target = root.join("notes/large.bin");
        assert_eq!(fs::read(&target).unwrap(), vec![11; 5 * 1024 * 1024]);
        assert_eq!(fs::metadata(target).unwrap().nlink(), 1);
    }
    #[test]
    fn large_new_publication_rejects_same_metadata_mutation_after_interruption() {
        use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let staged = large_replay_fixture(&root);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::publication::tests::recovery_cursor_child",
            ])
            .env("KG_PUBLICATION_REPLAY_FIXTURE", &root)
            .env("KG_LARGE_REPLAY_STOP_BEFORE_LINK", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(80));
        let before = fs::metadata(&staged).unwrap();
        fs::write(&staged, vec![12; 5 * 1024 * 1024]).unwrap();
        let name = std::ffi::CString::new(staged.as_os_str().as_bytes()).unwrap();
        let times = [
            libc::timespec {
                tv_sec: before.atime(),
                tv_nsec: before.atime_nsec(),
            },
            libc::timespec {
                tv_sec: before.mtime(),
                tv_nsec: before.mtime_nsec(),
            },
        ];
        assert_eq!(
            unsafe { libc::utimensat(libc::AT_FDCWD, name.as_ptr(), times.as_ptr(), 0) },
            0
        );
        let mut replay = RecoverySession::new();
        let mut rejected = false;
        for _ in 0..10000 {
            match replay.step(&root) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(true) => break,
                Ok(false) => (),
            }
        }
        assert!(rejected);
        assert!(!root.join("notes/large.bin").exists());
        assert_eq!(fs::read(staged).unwrap(), vec![12; 5 * 1024 * 1024]);
    }
    #[test]
    fn staged_content_is_rechecked_after_verification_before_publication() {
        use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        fs::create_dir_all(root.join("notes")).unwrap();
        assert!(commit_publication(
            &root,
            vec![(root.join("notes/page"), b"planned".to_vec())],
            &HashMap::new(),
            Some(0)
        )
        .is_err());
        let saved = migration_backup::backup(&root).unwrap();
        let id = saved.file_name().unwrap().to_str().unwrap();
        fs::write(
            root.join(".recovery/migration/pre-recovery.json"),
            serde_json::to_vec(id).unwrap(),
        )
        .unwrap();
        let transaction = fs::read_dir(root.join(".staging/transactions"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let staged = transaction.join("files/0000");
        let mut replay = RecoverySession::new();
        for _ in 0..10000 {
            assert!(!replay.step(&root).unwrap());
            if replay
                .active
                .as_ref()
                .and_then(|active| active.pending.as_ref())
                .is_some_and(|pending| pending.verified)
            {
                break;
            }
        }
        assert!(replay
            .active
            .as_ref()
            .and_then(|active| active.pending.as_ref())
            .is_some_and(|pending| pending.verified));
        let before = fs::metadata(&staged).unwrap();
        fs::write(&staged, b"mangled").unwrap();
        let name = std::ffi::CString::new(staged.as_os_str().as_bytes()).unwrap();
        let times = [
            libc::timespec {
                tv_sec: before.atime(),
                tv_nsec: before.atime_nsec(),
            },
            libc::timespec {
                tv_sec: before.mtime(),
                tv_nsec: before.mtime_nsec(),
            },
        ];
        assert_eq!(
            unsafe { libc::utimensat(libc::AT_FDCWD, name.as_ptr(), times.as_ptr(), 0) },
            0
        );
        assert!(replay.step(&root).is_err());
        assert!(!root.join("notes/page").exists());
        assert_eq!(fs::read(staged).unwrap(), b"mangled");
    }
    #[test]
    fn replay_uses_legacy_snapshot_digest() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        fs::create_dir_all(root.join("notes")).unwrap();
        assert!(commit_publication(
            &root,
            vec![(root.join("notes/page"), b"planned".to_vec())],
            &HashMap::new(),
            Some(0)
        )
        .is_err());
        let saved = migration_backup::backup(&root).unwrap();
        let id = saved.file_name().unwrap().to_str().unwrap();
        fs::write(
            root.join(".recovery/migration/pre-recovery.json"),
            serde_json::to_vec(id).unwrap(),
        )
        .unwrap();
        let mut replay = RecoverySession::new();
        let mut complete = false;
        for _ in 0..1000 {
            if replay.step(&root).unwrap() {
                complete = true;
                break;
            }
        }
        assert!(complete);
        assert_eq!(fs::read(root.join("notes/page")).unwrap(), b"planned");
    }
    #[test]
    fn restarted_removal_preserves_an_external_save_at_the_same_path() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        fs::create_dir_all(root.join("notes")).unwrap();
        let target = root.join("notes/page");
        fs::write(&target, b"original").unwrap();
        assert!(commit_publication_with_removals(
            &root,
            Vec::new(),
            vec![target.clone()],
            &HashMap::new(),
            Some(0)
        )
        .is_err());
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::publication::tests::recovery_cursor_child",
            ])
            .env("KG_PUBLICATION_REPLAY_FIXTURE", &root)
            .env("KG_REPLAY_STOP_AFTER_REMOVAL", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(83));
        fs::write(&target, b"external save").unwrap();
        let mut replay = RecoverySession::new();
        let mut done = false;
        for _ in 0..1000 {
            if replay.step(&root).unwrap() {
                done = true;
                break;
            }
        }
        assert!(done);
        let saves: Vec<Vec<u8>> = fs::read_dir(root.join(".recovery/withdrawn-live"))
            .unwrap()
            .map(|entry| fs::read(entry.unwrap().path().join("content")).unwrap())
            .collect();
        assert!(saves.contains(&b"original".to_vec()));
        assert!(saves.contains(&b"external save".to_vec()));
    }
    #[test]
    fn interrupted_multi_entry_transaction_resumes_without_skipping() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        fs::create_dir_all(root.join("notes")).unwrap();
        let files = (0..4)
            .map(|n| (root.join(format!("notes/{n}")), vec![n]))
            .collect();
        assert!(commit_publication(&root, files, &HashMap::new(), Some(0)).is_err());
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::publication::tests::recovery_cursor_child",
            ])
            .env("KG_PUBLICATION_REPLAY_FIXTURE", &root)
            .env("KG_REPLAY_STOP_AFTER_ENTRY", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(84));
        assert_eq!(fs::read_dir(root.join("notes")).unwrap().count(), 1);
        let mut replay = RecoverySession::new();
        let mut done = false;
        for _ in 0..1000 {
            if replay.step(&root).unwrap() {
                done = true;
                break;
            }
        }
        assert!(done);
        for n in 0..4 {
            assert_eq!(fs::read(root.join(format!("notes/{n}"))).unwrap(), vec![n]);
        }
    }
    #[test]
    fn replay_history_keeps_a_displaced_external_save() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        fs::create_dir_all(root.join("notes")).unwrap();
        let target = root.join("notes/page");
        fs::write(&target, b"baseline").unwrap();
        assert!(commit_publication(
            &root,
            vec![(target.clone(), b"planned".to_vec())],
            &HashMap::from([(target.clone(), b"baseline".to_vec())]),
            Some(0)
        )
        .is_err());
        fs::write(&target, b"external owner bytes").unwrap();
        let mut replay = RecoverySession::new();
        for _ in 0..100 {
            if replay.step(&root).unwrap() {
                break;
            }
        }
        let history =
            root.join(".recovery/migration/publication-history/unassociated/pre-work-unassociated");
        let archive = fs::read_dir(history)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            fs::read(archive.join("files/0000.held")).unwrap(),
            b"external owner bytes"
        );
        let owner = root.join("owner-edits").join(format!(
            "{}.md",
            hex::encode(Sha256::digest(b"external owner bytes"))
        ));
        assert_eq!(fs::read(owner).unwrap(), b"external owner bytes");
    }
    #[test]
    fn replay_rejects_symlinked_transaction_directory() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let outside = root.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("sentinel"), b"preserved").unwrap();
        let transactions = root.join(".staging/transactions");
        fs::create_dir_all(&transactions).unwrap();
        symlink(&outside, transactions.join("hostile")).unwrap();
        let mut replay = RecoverySession::new();
        let mut rejected = false;
        for _ in 0..100 {
            match replay.step(&root) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(true) => break,
                Ok(false) => (),
            }
        }
        assert!(rejected);
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"preserved");
    }
    #[test]
    fn replay_refuses_a_symlinked_staged_file() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        fs::create_dir_all(root.join("notes")).unwrap();
        assert!(commit_publication(
            &root,
            vec![(root.join("notes/page"), b"planned".to_vec())],
            &HashMap::new(),
            Some(0)
        )
        .is_err());
        let transaction = fs::read_dir(root.join(".staging/transactions"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let staged = transaction.join("files/0000");
        fs::remove_file(&staged).unwrap();
        let outside = temp.path().join("outside");
        fs::write(&outside, b"outside").unwrap();
        symlink(&outside, &staged).unwrap();
        let mut replay = RecoverySession::new();
        let mut rejected = false;
        for _ in 0..100 {
            match replay.step(&root) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(true) => break,
                Ok(false) => (),
            }
        }
        assert!(rejected);
        assert_eq!(fs::read(outside).unwrap(), b"outside");
        assert!(!root.join("notes/page").exists());
    }
    #[test]
    fn recovery_cursor_child() {
        let Some(root) = std::env::var_os("KG_PUBLICATION_REPLAY_FIXTURE") else {
            return;
        };
        let root = PathBuf::from(root);
        let mut replay = RecoverySession::new();
        for _ in 0..1000 {
            if replay.step(&root).unwrap() {
                panic!("Replay finished before interruption.");
            }
            if !replay.after.is_empty() {
                std::process::exit(86);
            }
        }
        panic!("Replay did not advance.");
    }
    #[test]
    fn interrupted_replay_rediscovers_unprocessed_transactions() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        fs::create_dir_all(root.join("notes")).unwrap();
        for n in 0..4 {
            let path = root.join(format!("notes/{n}"));
            assert!(
                commit_publication(&root, vec![(path, vec![n])], &HashMap::new(), Some(0)).is_err()
            );
        }
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::publication::tests::recovery_cursor_child",
            ])
            .env("KG_PUBLICATION_REPLAY_FIXTURE", &root)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(86));
        assert_eq!(fs::read_dir(root.join("notes")).unwrap().count(), 1);
        let mut replay = RecoverySession::new();
        let mut done = false;
        for _ in 0..1000 {
            if replay.step(&root).unwrap() {
                done = true;
                break;
            }
        }
        assert!(done);
        for n in 0..4 {
            assert_eq!(fs::read(root.join(format!("notes/{n}"))).unwrap(), vec![n]);
        }
        assert_eq!(
            fs::read_dir(root.join(".staging/transactions"))
                .unwrap()
                .count(),
            0
        );
        let history =
            root.join(".recovery/migration/publication-history/unassociated/pre-work-unassociated");
        let archives: Vec<_> = fs::read_dir(history)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(archives.len(), 4);
        assert!(archives
            .iter()
            .all(|path| path.join("manifest.json").is_file() && path.join("files/0000").is_file()));
    }

    #[test]
    fn concurrent_correction_updates_only_the_identified_complete_fact_row() {
        let root = tempfile::tempdir().unwrap();
        let evidence = serde_json::json!({"quote":"count", "byte_start":0,"byte_end":5,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let facts = [("fact-one", "1"), ("fact-prefix", "12"), ("fact-equal", "1")].map(|(id, value)| serde_json::json!({"fact_id":id,"subject_page_id":"page-test","property":"count","value":value,"qualifier":null,"origin":"source","evidence":evidence}));
        let mut header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({"schema":1,"page_id":"page-test","source_id":"source-test","title":"Test","kind":"event","evidence":evidence,"facts":facts})).unwrap();
        let body = "\n# Test\n\n- **count:** 1\n  - Fact identity: `fact-one`\n- **count:** 12\n  - Fact identity: `fact-prefix`\n- **count:** 1\n  - Fact identity: `fact-equal`\n";
        let proposed = format!(
            "---\n{}---\n{body}",
            serde_yaml_ng::to_string(&header).unwrap()
        );
        header.facts[0].manual_correction = Some(ManualFactCorrection { value: "9".into() });
        let owner = format!(
            "---\n{}---\n{body}",
            serde_yaml_ng::to_string(&header).unwrap()
        );
        let merged = merge_owner_save(
            root.path(),
            proposed.as_bytes(),
            Some(proposed.as_bytes()),
            owner.as_bytes(),
        )
        .unwrap();
        let merged = String::from_utf8(merged).unwrap();
        assert!(merged.contains("- **count:** 9\n  - Fact identity: `fact-one`"));
        assert!(merged.contains("- **count:** 12\n  - Fact identity: `fact-prefix`"));
        assert!(merged.contains("- **count:** 1\n  - Fact identity: `fact-equal`"));
        let header = parse_knowledge_header(&merged).unwrap();
        assert_eq!(header.facts[0].value, "9");
        assert_eq!(header.facts[1].value, "12");
        assert_eq!(header.facts[2].value, "1");
    }
}

// Hold each parent directory open and refuse symlinks at every traversal.
// Replacing a path ancestor cannot redirect removal to a different tree.
fn remove_collection_entry(root: &Path, relative: &Path) -> std::io::Result<()> {
    use rustix::fs::{openat, Mode, OFlags, CWD};
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut directory = openat(CWD, root, flags, Mode::empty())?;
    for component in relative.parent().unwrap_or(Path::new("")).components() {
        match openat(&directory, component.as_os_str(), flags, Mode::empty()) {
            Ok(next) => directory = next,
            Err(rustix::io::Errno::NOENT) => return Ok(()),
            Err(error) => return Err(error.into()),
        }
    }
    let name = std::ffi::CString::new(relative.file_name().unwrap().as_encoded_bytes())?;
    remove_entry_at(&directory, &name, 0)?;
    File::from(directory).sync_all()
}

fn remove_entry_at(
    directory: &impl std::os::fd::AsFd,
    name: &std::ffi::CStr,
    depth: usize,
) -> std::io::Result<()> {
    use rustix::fs::{openat, unlinkat, AtFlags, Dir, Mode, OFlags};
    if depth > 256 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Removal directory nesting exceeds its recovery bound.",
        ));
    }
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    match openat(directory, name, flags, Mode::empty()) {
        Ok(child) => {
            let mut entries = Dir::read_from(&child)?;
            while let Some(entry) = entries.read() {
                let entry = entry?;
                let name = entry.file_name();
                if name.to_bytes() != b"." && name.to_bytes() != b".." {
                    remove_entry_at(&child, name, depth + 1)?;
                }
            }
            unlinkat(directory, name, AtFlags::REMOVEDIR)?;
        }
        Err(rustix::io::Errno::NOENT) => {}
        Err(rustix::io::Errno::NOTDIR | rustix::io::Errno::LOOP) => {
            match unlinkat(directory, name, AtFlags::empty()) {
                Ok(()) | Err(rustix::io::Errno::NOENT) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

pub(super) fn validate_real_collection_ancestors(root: &Path, destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| GardenError::Invalid("Missing collection parent.".into()))?;
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| GardenError::Invalid("Path escaped collection.".into()))?;
    let mut path = root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err(GardenError::Invalid("Invalid collection ancestor.".into()));
        }
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => {},
            Ok(_) => return Err(GardenError::Invalid("A collection ancestor is a symlink or is not a directory; publication remains pending.".into())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod publication_removal_tests {
    use super::*;
    #[test]
    fn removal_keeps_its_open_parent_when_an_ancestor_is_swapped() {
        use std::os::unix::fs::symlink;
        let workspace = tempfile::tempdir().unwrap();
        let parent = workspace.path().join("sources");
        let outside = workspace.path().join("outside");
        for directory in [&parent, &outside] {
            fs::create_dir_all(directory.join("victim")).unwrap();
            fs::write(directory.join("victim/sentinel"), "retain outside").unwrap();
        }
        let held = File::open(&parent).unwrap();
        fs::rename(&parent, workspace.path().join("moved-sources")).unwrap();
        symlink(&outside, &parent).unwrap();
        remove_entry_at(&held, &std::ffi::CString::new("victim").unwrap(), 0).unwrap();
        assert!(outside.join("victim/sentinel").is_file());
        assert!(!workspace.path().join("moved-sources/victim").exists());
    }
}
