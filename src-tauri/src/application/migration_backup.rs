pub(crate) mod verified_index;
use super::*;

mod archive_prune;
mod authored_stage;
mod bounded_copy;
mod cutover_plan;
pub(crate) mod final_verify;
mod incremental_verify;
mod label_catalog;
pub(crate) mod legacy_adopt;
mod legacy_hash;
mod legacy_manifest;
mod legacy_verify;
mod manifest_cursor;
mod manifest_records;
pub(crate) mod precutover;
pub(super) mod replay_authority;
mod retirement;
mod retirement_flow;
mod retirement_verify;
mod snapshot_worker;
pub(crate) mod startup_coordinator;
mod traversal;
mod trusted_retirement;

pub(super) fn read_bounded_nofollow(root: &Path, relative: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    bounded_copy::source_file(root, relative)?
        .take(limit + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > limit {
        return Err(GardenError::Recovery(
            "Recovery record exceeds its size limit.".into(),
        ));
    }
    Ok(data)
}

pub(super) fn open_nofollow(root: &Path, relative: &Path) -> Result<File> {
    bounded_copy::source_file(root, relative)
}
fn replay_copy_directories(root: &Path, journal: &Path, create: bool) -> Result<()> {
    use rustix::fs::{mkdirat, openat, Mode, OFlags, CWD};
    use rustix::io::Errno;
    use std::os::unix::fs::MetadataExt;
    let generation = journal
        .parent()
        .and_then(|path| path.strip_prefix(".recovery/migration/replay-large").ok())
        .ok_or_else(|| GardenError::Recovery("Replay copy path is invalid.".into()))?;
    if generation.components().count() != 1
        || !generation
            .to_str()
            .is_some_and(|value| value.starts_with("pre-work-") && value.len() <= 128)
    {
        return Err(GardenError::Recovery(
            "Replay copy generation is invalid.".into(),
        ));
    }
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let root_fd = openat(CWD, root, flags, Mode::empty()).map_err(std::io::Error::from)?;
    let recovery =
        openat(&root_fd, ".recovery", flags, Mode::empty()).map_err(std::io::Error::from)?;
    let mut parent =
        openat(&recovery, "migration", flags, Mode::empty()).map_err(std::io::Error::from)?;
    for part in [
        std::ffi::OsStr::new("replay-large"),
        generation.as_os_str(),
        std::ffi::OsStr::new("temps"),
    ] {
        if create {
            match mkdirat(&parent, part, Mode::from_bits_truncate(0o700)) {
                Ok(()) | Err(Errno::EXIST) => (),
                Err(error) => return Err(std::io::Error::from(error).into()),
            }
        }
        let child = openat(&parent, part, flags, Mode::empty()).map_err(std::io::Error::from)?;
        let metadata = File::from(child.try_clone()?).metadata()?;
        if metadata.mode() & 0o077 != 0 || metadata.uid() != unsafe { libc::geteuid() } {
            return Err(GardenError::Recovery(
                "Replay copy work directory is not private.".into(),
            ));
        }
        if create {
            File::from(parent).sync_all()?;
        } else {
            drop(parent);
        }
        parent = child;
    }
    if create {
        File::from(parent).sync_all()?;
    }
    Ok(())
}
pub(super) fn begin_replay_copy(root: &Path, journal: &Path, source: &Path) -> Result<()> {
    replay_copy_directories(root, journal, true)?;
    let temp = journal
        .parent()
        .ok_or_else(|| GardenError::Recovery("Replay copy journal has no parent.".into()))?
        .join("temps")
        .join(Uuid::new_v4().to_string());
    bounded_copy::begin(root, journal, source, &temp)
}
pub(super) fn step_replay_copy(root: &Path, journal: &Path, source: &Path) -> Result<bool> {
    replay_copy_destination(root, journal)?;
    if bounded_copy::source(root, journal)? != source {
        return Err(GardenError::Recovery(
            "Replay copy source identity changed.".into(),
        ));
    }
    bounded_copy::step(root, journal, 4 * 1024 * 1024)
}
pub(super) fn replay_copy_destination(root: &Path, journal: &Path) -> Result<PathBuf> {
    replay_copy_directories(root, journal, false)?;
    let destination = bounded_copy::destination(root, journal)?;
    let directory = journal
        .parent()
        .ok_or_else(|| GardenError::Recovery("Replay copy journal has no parent.".into()))?
        .join("temps");
    let name = destination.strip_prefix(&directory).map_err(|_| {
        GardenError::Recovery("Replay copy escaped its private work directory.".into())
    })?;
    if name.components().count() != 1
        || name
            .to_str()
            .and_then(|name| Uuid::parse_str(name).ok())
            .is_none()
    {
        return Err(GardenError::Recovery(
            "Replay copy identity is invalid.".into(),
        ));
    }
    Ok(destination)
}

#[cfg(test)]
type RecoveryPause = (PathBuf, Arc<std::sync::Barrier>, Arc<std::sync::Barrier>);
#[cfg(test)]
static PAUSE_PRE_RECOVERY: std::sync::Mutex<Option<RecoveryPause>> = std::sync::Mutex::new(None);

#[derive(Serialize, Deserialize)]
struct MigrationState {
    backup: String,
    phase: String,
}

// The caller holds the collection lock. Pending publication journals may be
// captured before replay. Derived indexes are rebuildable and may have a WAL.
pub(super) fn backup(root: &Path) -> Result<PathBuf> {
    let recovery = root.join(".recovery/migration");
    fs::create_dir_all(&recovery)?;
    let staging = tempfile::tempdir_in(&recovery)?;
    let before = inventory(root)?;
    let files = staging.path().join("files");
    fs::create_dir(&files)?;
    for relative in &before {
        let source = root.join(relative);
        let target = files.join(relative);
        if fs::symlink_metadata(&source)?.is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            fs::create_dir_all(target.parent().unwrap())?;
            fs::copy(&source, &target)?;
            File::open(&target)?.sync_all()?;
        }
    }
    if inventory(root)? != before {
        return Err(GardenError::Invalid(
            "Collection changed during migration backup; retry when external edits have stopped."
                .into(),
        ));
    }
    if manifest(root, &before)? != manifest(&files, &before)? || inventory(root)? != before {
        return Err(GardenError::Invalid(
            "Collection changed during migration backup; no migration was started.".into(),
        ));
    }
    for relative in before.iter().rev() {
        let directory = files.join(relative);
        if directory.is_dir() {
            sync_directory(&directory)?;
        }
    }
    sync_directory(&files)?;
    let manifest = manifest(&files, &before)?;
    write_atomic(
        &staging.path().join("manifest.json"),
        &serde_json::to_vec(&manifest)?,
    )?;
    sync_directory(staging.path())?;
    let destination = recovery.join(format!("backup-{}", Uuid::new_v4()));
    fs::rename(staging.path(), &destination)?;
    sync_directory(&recovery)?;
    Ok(destination)
}

// Capture the exact pre-recovery collection, including pending transaction
// journals, before replay is allowed to change any authored file.
pub(super) fn prepare_pre_recovery(root: &Path) -> Result<()> {
    #[cfg(test)]
    {
        let pause =
            PAUSE_PRE_RECOVERY
                .lock()
                .unwrap()
                .as_ref()
                .and_then(|(path, entered, release)| {
                    (path == root).then(|| (Arc::clone(entered), Arc::clone(release)))
                });
        if let Some((entered, release)) = pause {
            entered.wait();
            release.wait();
        }
    }
    let journal = root.join(".recovery/migration/pre-recovery.json");
    validate_real_collection_ancestors(root, &journal)?;
    match fs::symlink_metadata(&journal) {
        Ok(metadata) if metadata.is_file() => {
            let name: String = serde_json::from_str(&read_bounded_text(&journal, 4096)?)?;
            if !name.starts_with("backup-") || Path::new(&name).components().count() != 1 {
                return Err(GardenError::Invalid(
                    "Pre-recovery backup identity is invalid; originals remain preserved.".into(),
                ));
            }
            let history = root
                .join(".recovery/migration/pre-recovery-history")
                .join(format!("{name}.json"));
            validate_real_collection_ancestors(root, &history)?;
            match fs::symlink_metadata(&history) {
                Ok(_) => return Err(GardenError::Invalid("Pre-recovery completion conflicts with an active journal; both remain preserved.".into())),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
            }
            return verify_backup(&root.join(".recovery/migration").join(name));
        }
        Ok(_) => {
            return Err(GardenError::Invalid(
                "Pre-recovery backup journal is not a regular file.".into(),
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    let saved = backup(root)?;
    verify_backup(&saved)?;
    let name = saved.file_name().unwrap().to_string_lossy().into_owned();
    write_atomic(&journal, &serde_json::to_vec(&name)?)?;
    #[cfg(test)]
    if std::env::var_os("KG_STOP_AFTER_PRE_RECOVERY_BACKUP").is_some() {
        std::process::exit(77);
    }
    Ok(())
}

// An active pre-recovery snapshot defines the exact intent set that may be
// consumed. A removed or newly inserted intent remains pending, rather than
// being mistaken for a completed exclusion.
pub(super) fn pre_recovery_has_integrity_intents(root: &Path) -> Result<bool> {
    let journal = root.join(".recovery/migration/pre-recovery.json");
    match fs::symlink_metadata(&journal) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Ok(metadata) if metadata.is_file() => (),
        Ok(_) => {
            return Err(GardenError::Recovery(
                "Pre-recovery journal is not a regular file.".into(),
            ))
        }
        Err(error) => return Err(error.into()),
    }
    let name: String = serde_json::from_str(&read_bounded_text(&journal, 4096)?)?;
    if !name.starts_with("backup-") || Path::new(&name).components().count() != 1 {
        return Err(GardenError::Recovery(
            "Pre-recovery backup identity is invalid.".into(),
        ));
    }
    let saved = root
        .join(".recovery/migration")
        .join(name)
        .join("files/.recovery/integrity-exclusions");
    validate_real_collection_ancestors(root, &saved.join("entry"))?;
    match fs::read_dir(saved) {
        Ok(mut entries) => Ok(entries.next().transpose()?.is_some()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn missing_integrity_intents(root: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    fn entries(directory: &Path) -> Result<std::collections::BTreeMap<String, Vec<u8>>> {
        let mut result = std::collections::BTreeMap::new();
        let items = match fs::read_dir(directory) {
            Ok(items) => items,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(result),
            Err(error) => return Err(error.into()),
        };
        for item in items {
            let item = item?;
            if !item.file_type()?.is_file() {
                return Err(GardenError::Recovery(
                    "Integrity intent storage contains a non-regular entry; it remains preserved."
                        .into(),
                ));
            }
            let name = item.file_name().into_string().map_err(|_| {
                GardenError::Recovery(
                    "Integrity intent name is not readable; it remains preserved.".into(),
                )
            })?;
            let mut bytes = Vec::new();
            File::open(item.path())?
                .take(4097)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 4096 {
                return Err(GardenError::Recovery(
                    "Integrity intent is too large; it remains preserved.".into(),
                ));
            }
            result.insert(name, bytes);
        }
        Ok(result)
    }
    let journal = root.join(".recovery/migration/pre-recovery.json");
    let name: String = serde_json::from_str(&read_bounded_text(&journal, 4096)?)?;
    if !name.starts_with("backup-") || Path::new(&name).components().count() != 1 {
        return Err(GardenError::Recovery(
            "Pre-recovery backup identity is invalid.".into(),
        ));
    }
    let saved = root
        .join(".recovery/migration")
        .join(name)
        .join("files/.recovery/integrity-exclusions");
    let live = root.join(".recovery/integrity-exclusions");
    validate_real_collection_ancestors(root, &saved.join("entry"))?;
    validate_real_collection_ancestors(root, &live.join("entry"))?;
    let saved = entries(&saved)?;
    let live = entries(&live)?;
    if live
        .iter()
        .any(|(name, bytes)| saved.get(name) != Some(bytes))
    {
        return Err(GardenError::Recovery("Integrity intents changed since the verified backup; inspect the retained snapshot before retrying.".into()));
    }
    Ok(saved
        .into_iter()
        .filter(|(name, _)| !live.contains_key(name))
        .collect())
}

fn consumed_integrity_name(root: &Path, name: &str, bytes: &[u8]) -> Result<String> {
    let generation: String = serde_json::from_str(&read_bounded_text(
        &root.join(".recovery/migration/pre-recovery.json"),
        4096,
    )?)?;
    let mut hash = Sha256::new();
    for part in [generation.as_bytes(), name.as_bytes(), bytes] {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part);
    }
    Ok(format!("{}.json", hex::encode(hash.finalize())))
}

pub(super) fn consumed_integrity_matches(root: &Path, name: &str, expected: &[u8]) -> Result<bool> {
    use rustix::fs::{openat, Mode, OFlags, CWD};
    use rustix::io::Errno;
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let root_fd = openat(CWD, root, flags, Mode::empty()).map_err(std::io::Error::from)?;
    let recovery =
        openat(&root_fd, ".recovery", flags, Mode::empty()).map_err(std::io::Error::from)?;
    let consumed = match openat(&recovery, "integrity-consumed", flags, Mode::empty()) {
        Ok(fd) => fd,
        Err(Errno::NOENT) => return Ok(false),
        Err(error) => return Err(std::io::Error::from(error).into()),
    };
    let id = consumed_integrity_name(root, name, expected)?;
    let fd = match openat(
        &consumed,
        id.as_str(),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(Errno::NOENT) => return Ok(false),
        Err(error) => return Err(std::io::Error::from(error).into()),
    };
    let file = File::from(fd);
    if !file.metadata()?.is_file() {
        return Err(GardenError::Recovery(
            "Consumed integrity record is not a regular file.".into(),
        ));
    }
    let mut actual = Vec::new();
    file.take(4097).read_to_end(&mut actual)?;
    Ok(actual == expected)
}

pub(super) fn consume_integrity_intent(root: &Path, name: &str, expected: &[u8]) -> Result<()> {
    use rustix::fs::{mkdirat, openat, renameat_with, Mode, OFlags, RenameFlags, CWD};
    use rustix::io::Errno;
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let root_fd = openat(CWD, root, flags, Mode::empty()).map_err(std::io::Error::from)?;
    let recovery =
        openat(&root_fd, ".recovery", flags, Mode::empty()).map_err(std::io::Error::from)?;
    let intents = openat(&recovery, "integrity-exclusions", flags, Mode::empty())
        .map_err(std::io::Error::from)?;
    match mkdirat(
        &recovery,
        "integrity-consumed",
        Mode::from_bits_truncate(0o700),
    ) {
        Ok(()) | Err(Errno::EXIST) => (),
        Err(error) => return Err(std::io::Error::from(error).into()),
    }
    let consumed = openat(&recovery, "integrity-consumed", flags, Mode::empty())
        .map_err(std::io::Error::from)?;
    let id = consumed_integrity_name(root, name, expected)?;
    renameat_with(
        &intents,
        name,
        &consumed,
        id.as_str(),
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    File::from(intents).sync_all()?;
    File::from(consumed).sync_all()?;
    if !consumed_integrity_matches(root, name, expected)? {
        return Err(GardenError::Recovery(
            "Integrity intent changed during consumption; its bytes remain preserved.".into(),
        ));
    }
    Ok(())
}

pub(super) fn finish_pre_recovery(root: &Path) -> Result<()> {
    finish_pre_recovery_with_hook(root, None, || {})
}
pub(super) fn finish_pre_recovery_verified(root: &Path, proof: &final_verify::Proof) -> Result<()> {
    finish_pre_recovery_with_hook(root, Some(proof.pre()), || {})
}
fn finish_pre_recovery_with_hook(
    root: &Path,
    verified: Option<&str>,
    before_rename: impl FnOnce(),
) -> Result<()> {
    use rustix::fs::{mkdirat, openat, renameat_with, Mode, OFlags, RenameFlags, CWD};
    use rustix::io::Errno;
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let root_fd = openat(CWD, root, flags, Mode::empty()).map_err(std::io::Error::from)?;
    let recovery =
        openat(&root_fd, ".recovery", flags, Mode::empty()).map_err(std::io::Error::from)?;
    let migration =
        openat(&recovery, "migration", flags, Mode::empty()).map_err(std::io::Error::from)?;
    let active = openat(
        &migration,
        "pre-recovery.json",
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)?;
    let active = File::from(active);
    if !active.metadata()?.is_file() {
        return Err(GardenError::Invalid(
            "Pre-recovery journal is not a regular file.".into(),
        ));
    }
    let mut bytes = Vec::new();
    active.take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(GardenError::Invalid(
            "Pre-recovery journal is too large.".into(),
        ));
    }
    let name: String = serde_json::from_slice(&bytes)?;
    if !name.starts_with("backup-") || Path::new(&name).components().count() != 1 {
        return Err(GardenError::Invalid(
            "Pre-recovery backup identity is invalid; originals remain preserved.".into(),
        ));
    }
    match verified {
        Some(id) if id == name => (),
        Some(_) => {
            return Err(GardenError::Recovery(
                "Verified pre-recovery identity changed.".into(),
            ))
        }
        None => verify_backup(&root.join(".recovery/migration").join(&name))?,
    }
    match mkdirat(
        &migration,
        "pre-recovery-history",
        Mode::from_bits_truncate(0o700),
    ) {
        Ok(()) | Err(Errno::EXIST) => (),
        Err(error) => return Err(std::io::Error::from(error).into()),
    }
    let history = openat(&migration, "pre-recovery-history", flags, Mode::empty())
        .map_err(std::io::Error::from)?;
    let mut completed = format!("{name}.json");
    before_rename();
    let result = renameat_with(
        &migration,
        "pre-recovery.json",
        &history,
        completed.as_str(),
        RenameFlags::NOREPLACE,
    );
    if result == Err(Errno::EXIST) && verified.is_some() {
        let previous = openat(
            &history,
            completed.as_str(),
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(std::io::Error::from)?;
        let previous = File::from(previous);
        if !previous.metadata()?.is_file() {
            return Err(GardenError::Recovery(
                "Completed recovery history is not a regular file.".into(),
            ));
        }
        let mut previous_bytes = Vec::new();
        previous.take(4097).read_to_end(&mut previous_bytes)?;
        if previous_bytes != bytes {
            return Err(GardenError::Recovery(
                "Completed recovery history conflicts with the active journal.".into(),
            ));
        }
        let coordinator: serde_json::Value = serde_json::from_slice(&read_bounded_nofollow(
            root,
            Path::new(".recovery/migration/startup-coordinator.json"),
            16384,
        )?)?;
        if coordinator.get("reused").and_then(|value| value.as_bool()) != Some(true)
            || coordinator.get("phase").and_then(|value| value.as_str()) != Some("complete")
            || coordinator
                .get("pre_backup")
                .and_then(|value| value.as_str())
                != Some(name.as_str())
        {
            return Err(GardenError::Recovery(
                "Reused recovery completion is ambiguous.".into(),
            ));
        }
        let work = coordinator
            .get("pre_work")
            .and_then(|value| value.as_str())
            .ok_or_else(|| GardenError::Recovery("Recovery generation is missing.".into()))?;
        let work = Path::new(work);
        validate_relative_collection_path(work)?;
        let generation = work
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| GardenError::Recovery("Recovery generation is invalid.".into()))?;
        if work.parent() != Some(Path::new(".recovery/migration"))
            || !generation.starts_with("pre-work-")
            || generation.len() > 128
        {
            return Err(GardenError::Recovery(
                "Recovery generation is invalid.".into(),
            ));
        }
        completed = format!("{name}-{generation}.json");
        renameat_with(
            &migration,
            "pre-recovery.json",
            &history,
            completed.as_str(),
            RenameFlags::NOREPLACE,
        )
        .map_err(std::io::Error::from)?;
    } else {
        result.map_err(std::io::Error::from)?;
    }
    #[cfg(test)]
    if std::env::var_os("KG_STOP_AFTER_PRE_RECOVERY_COMPLETION").is_some() {
        std::process::exit(78);
    }
    let saved_journal = openat(
        &history,
        completed.as_str(),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)?;
    let mut saved_bytes = Vec::new();
    File::from(saved_journal)
        .take(4097)
        .read_to_end(&mut saved_bytes)?;
    if saved_bytes != bytes {
        return Err(GardenError::Invalid("Pre-recovery journal changed during completion; the changed bytes remain preserved in history.".into()));
    }
    File::from(history).sync_all()?;
    File::from(migration).sync_all()?;
    Ok(())
}

pub(super) fn prepare(root: &Path) -> Result<()> {
    let state_path = root.join(".recovery/migration/state.json");
    validate_real_collection_ancestors(root, &state_path)?;
    let state_exists = match fs::symlink_metadata(&state_path) {
        Ok(metadata) if metadata.is_file() => true,
        Ok(_) => {
            return Err(GardenError::Invalid(
                "The migration state is not a regular file.".into(),
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    if state_exists {
        let state: MigrationState = serde_json::from_slice(&fs::read(&state_path)?)?;
        if ![
            "backup_verified",
            "evidence_imported",
            "cutover_preparing",
            "authority_retired",
        ]
        .contains(&state.phase.as_str())
            || !state.backup.starts_with("backup-")
            || Path::new(&state.backup).components().count() != 1
        {
            return Err(GardenError::Invalid(
                "The migration state is invalid; original collection files were retained.".into(),
            ));
        }
        verify_backup(&root.join(".recovery/migration").join(state.backup))?;
        return Ok(());
    }
    let transactions = root.join(".staging/transactions");
    for entry in fs::read_dir(transactions)? {
        if entry?.path().join("manifest.json").exists() {
            return Err(GardenError::Invalid(
                "A publication must be recovered before migration can begin.".into(),
            ));
        }
    }
    let saved = backup(root)?;
    verify_backup(&saved)?;
    let state = MigrationState {
        backup: saved.file_name().unwrap().to_string_lossy().into_owned(),
        phase: "backup_verified".into(),
    };
    write_atomic(&state_path, &serde_json::to_vec(&state)?)
}

pub(super) fn diagnostics(root: &Path) -> Result<Vec<reconciliation::CollectionDiagnostic>> {
    let path = root.join(".recovery/migration/state.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let state: MigrationState = serde_json::from_slice(&fs::read(path)?)?;
    if !state.backup.starts_with("backup-") || Path::new(&state.backup).components().count() != 1 {
        return Err(GardenError::Invalid(
            "Migration recovery identity is invalid.".into(),
        ));
    }
    let saved = root.join(".recovery/migration").join(state.backup);
    let staged = saved.join("authored-material.json");
    if !staged.exists() {
        return Ok(Vec::new());
    }
    let materials: Vec<AuthoredMaterial> = serde_json::from_slice(&fs::read(staged)?)?;
    let mut paths = Vec::new();
    for material in materials {
        for opaque in material.opaque_saves {
            let relative = Path::new(&opaque.path);
            validate_relative_collection_path(relative)?;
            let recovery_path = saved.join("files").join(relative);
            paths.push(
                recovery_path
                    .strip_prefix(root)
                    .map_err(|_| {
                        GardenError::Invalid(
                            "Migration recovery path escaped the collection.".into(),
                        )
                    })?
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    paths.sort();
    paths.dedup();
    let diagnostics = if paths.is_empty() {
        Vec::new()
    } else {
        vec![reconciliation::CollectionDiagnostic {
            kind: "migration_ambiguous_owner_save".into(),
            paths,
            detail: "Previous page snapshots and uncertain saves may mix owner and generated material. Their exact bytes remain in migration recovery; the mixed snapshots were not promoted into knowledge.".into(),
        }]
    };
    Ok(diagnostics)
}

pub(super) fn import_pending_authored_sources(app: &mut Application) -> Result<()> {
    let state_path = app.root.join(".recovery/migration/state.json");
    if !state_path.exists() {
        return Ok(());
    }
    let mut state: MigrationState = serde_json::from_slice(&fs::read(&state_path)?)?;
    if [
        "evidence_imported",
        "cutover_preparing",
        "authority_retired",
    ]
    .contains(&state.phase.as_str())
    {
        return Ok(());
    }
    if state.phase != "backup_verified" {
        return Err(GardenError::Invalid("Invalid migration phase.".into()));
    }
    let saved = app.root.join(".recovery/migration").join(&state.backup);
    import_authored_sources(app, &saved)?;
    state.phase = "evidence_imported".into();
    write_atomic(&state_path, &serde_json::to_vec(&state)?)
}

pub(super) fn retire_manual_authority(app: &mut Application) -> Result<()> {
    let state_path = app.root.join(".recovery/migration/state.json");
    if !state_path.exists() {
        return Ok(());
    }
    let mut state: MigrationState = serde_json::from_slice(&fs::read(&state_path)?)?;
    if state.phase == "authority_retired" {
        return Ok(());
    }
    if !["evidence_imported", "cutover_preparing"].contains(&state.phase.as_str()) {
        return Err(GardenError::Invalid(
            "Migration is not ready for the authority transition.".into(),
        ));
    }
    let saved = app.root.join(".recovery/migration").join(&state.backup);
    verify_backup(&saved)?;
    let files = saved.join("files");
    let paths = inventory(&files)?;
    if state.phase == "evidence_imported" {
        let intents: Vec<AuthoredImportIntent> =
            serde_json::from_slice(&fs::read(stage_import_journal(&saved)?)?)?;
        ensure_unrelated_collection_files_match(&app.root, &saved, &intents)?;
        for intent in &intents {
            let Some(expected) = &intent.expected_bundle_sha256 else {
                return Err(GardenError::Invalid(
                    "Authored source import is not committed.".into(),
                ));
            };
            if bundle_digest(&app.source_dir(&intent.source_id)?)? != *expected {
                return Err(GardenError::Invalid(
                    "Authored source bundle changed before cutover.".into(),
                ));
            }
        }
        for relative in &paths {
            let source = files.join(relative);
            if !fs::symlink_metadata(&source)?.is_file() {
                continue;
            }
            let tracked = (relative.starts_with("pages")
                && relative.extension().is_some_and(|ext| ext == "md"))
                || (relative.starts_with("sources")
                    && (relative.file_name().is_some_and(|name| name == "index.md")
                        || (relative
                            .parent()
                            .and_then(Path::file_name)
                            .is_some_and(|name| name == "acquisitions")
                            && relative.extension().is_some_and(|ext| ext == "md"))));
            if tracked && fs::metadata(&source)?.len() <= MAX_PAGE_BYTES {
                trusted_pages::record(&app.root, relative, &fs::read(source)?)?;
            }
        }
        for intent in &intents {
            let path = app.source_dir(&intent.source_id)?.join("index.md");
            let relative = path
                .strip_prefix(&app.root)
                .map_err(|_| GardenError::Invalid("Authored source escaped collection.".into()))?;
            trusted_pages::record(&app.root, relative, &fs::read(&path)?)?;
        }
        state.phase = "cutover_preparing".into();
        write_atomic(&state_path, &serde_json::to_vec(&state)?)?;
    }
    for relative in paths
        .iter()
        .filter(|path| path.starts_with("pages") && path.extension().is_some_and(|ext| ext == "md"))
    {
        let snapshot = files.join(relative);
        if !fs::symlink_metadata(&snapshot)?.is_file() {
            continue;
        }
        if fs::metadata(&snapshot)?.len() > MAX_PAGE_BYTES {
            continue;
        }
        let raw = fs::read(&snapshot)?;
        let Ok(text) = std::str::from_utf8(&raw) else {
            continue;
        };
        let Ok(mut header) = parse_knowledge_header(text) else {
            continue;
        };
        let mut facts = Vec::new();
        for mut fact in header.facts {
            if fact.manual_correction.take().is_some() {
                let Some(support) = app.latest_support(&fact.supports)?.cloned() else {
                    continue;
                };
                fact.value = support.value;
                fact.qualifier = support.qualifier;
                fact.origin = support.origin;
                fact.evidence = support.evidence;
            }
            facts.push(fact);
        }
        header.facts = facts;
        header.owner_content.clear();
        header.owner_metadata.clear();
        header.uninterpreted_owner_edits.clear();
        header.external_edit_status = None;
        let updated = app.render_knowledge_page(header, &HashMap::new())?;
        let destination = app.root.join(relative);
        commit_publication(
            &app.root,
            vec![(destination.clone(), updated.into_bytes())],
            &HashMap::from([(destination, raw)]),
            None,
        )?;
    }
    state.phase = "authority_retired".into();
    write_atomic(&state_path, &serde_json::to_vec(&state)?)
}

#[derive(Serialize, Deserialize)]
struct ManifestV2 {
    version: u32,
    entries: Vec<(PathBuf, Option<String>)>,
}

fn verify_manifest(root: &Path, bytes: &[u8]) -> Result<Vec<PathBuf>> {
    let paths = inventory(root)?;
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    if value.is_array() {
        let expected: Vec<(PathBuf, Option<String>)> = serde_json::from_value(value)?;
        if manifest(root, &paths)? != expected {
            return Err(GardenError::Invalid(
                "Migration backup failed integrity verification.".into(),
            ));
        }
    } else {
        let expected: ManifestV2 = serde_json::from_value(value)?;
        if expected.version != 2 || expected.entries.len() != paths.len() {
            return Err(GardenError::Invalid(
                "Migration backup manifest is invalid.".into(),
            ));
        }
        for (path, (expected_path, expected_digest)) in paths.iter().zip(&expected.entries) {
            validate_relative_collection_path(expected_path)?;
            if path != expected_path {
                return Err(GardenError::Invalid(
                    "Migration backup paths differ from the manifest.".into(),
                ));
            }
            let full = root.join(path);
            let kind = fs::symlink_metadata(&full)?.file_type();
            let digest = if kind.is_dir() {
                None
            } else if kind.is_file() {
                Some(manifest_cursor::digest_file(root, path)?)
            } else {
                return Err(GardenError::Invalid(
                    "Migration backup contains a non-regular entry.".into(),
                ));
            };
            if &digest != expected_digest {
                return Err(GardenError::Invalid(
                    "Migration backup failed integrity verification.".into(),
                ));
            }
        }
    }
    Ok(paths)
}

fn verify_v3(files: &Path, database: &Path) -> Result<()> {
    let work = tempfile::tempdir()?;
    incremental_verify::begin(files, database, work.path())?;
    let mut session = incremental_verify::Session::new();
    while !incremental_verify::step(files, database, work.path(), &mut session)? {}
    Ok(())
}

fn verify_backup(saved: &Path) -> Result<()> {
    let v3 = saved.join("manifest.sqlite");
    if fs::symlink_metadata(&v3).is_ok() {
        if !fs::symlink_metadata(saved)?.is_dir()
            || !fs::symlink_metadata(saved.join("files"))?.is_dir()
            || !fs::symlink_metadata(&v3)?.is_file()
        {
            return Err(GardenError::Invalid(
                "Migration backup contains a non-regular entry.".into(),
            ));
        }
        return verify_v3(&saved.join("files"), &v3);
    }
    if !fs::symlink_metadata(saved)?.is_dir()
        || !fs::symlink_metadata(saved.join("files"))?.is_dir()
        || !fs::symlink_metadata(saved.join("manifest.json"))?.is_file()
    {
        return Err(GardenError::Invalid(
            "Migration backup contains a non-regular entry.".into(),
        ));
    }
    verify_manifest(
        &saved.join("files"),
        &fs::read(saved.join("manifest.json"))?,
    )?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct AuthoredCorrection {
    fact_id: String,
    property: String,
    value: String,
    qualifier: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct OpaqueOwnerSave {
    path: String,
    bytes: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct AuthoredMaterial {
    page_id: String,
    corrections: Vec<AuthoredCorrection>,
    prose: Vec<String>,
    // These saves can contain generated text. They are archival evidence,
    // not independent claims to feed back into semantic processing.
    opaque_saves: Vec<OpaqueOwnerSave>,
}

fn extract_authored_material(
    snapshot: &Path,
    header: &KnowledgePageHeader,
) -> Result<AuthoredMaterial> {
    let corrections = header
        .facts
        .iter()
        .filter_map(|fact| {
            fact.manual_correction
                .as_ref()
                .map(|correction| AuthoredCorrection {
                    fact_id: fact.fact_id.clone(),
                    property: fact.property.clone(),
                    value: correction.value.clone(),
                    qualifier: fact.qualifier.clone(),
                })
        })
        .collect();
    let mut opaque_saves = Vec::new();
    for path in &header.uninterpreted_owner_edits {
        let relative = Path::new(path);
        validate_relative_collection_path(relative)?;
        if relative.components().next()
            != Some(std::path::Component::Normal("owner-edits".as_ref()))
        {
            return Err(GardenError::Invalid(
                "An owner save points outside the retained owner-edit directory.".into(),
            ));
        }
        let full = snapshot.join(relative);
        validate_real_collection_ancestors(snapshot, &full)?;
        if !fs::symlink_metadata(&full)?.is_file() {
            return Err(GardenError::Invalid(
                "An owner save is not a regular file.".into(),
            ));
        }
        opaque_saves.push(OpaqueOwnerSave {
            path: path.clone(),
            bytes: fs::read(full)?,
        });
    }
    Ok(AuthoredMaterial {
        page_id: header.page_id.clone(),
        corrections,
        prose: header.owner_content.clone(),
        opaque_saves,
    })
}

fn capture_snapshot_fact_edits(header: &mut KnowledgePageHeader, markdown: &str) {
    let Some((_, body)) = markdown.split_once("\n---\n") else {
        return;
    };
    let mut pending = None::<(String, String)>;
    for line in body.lines() {
        if let Some(fact_line) = line.strip_prefix("- **") {
            pending = fact_line
                .split_once(":** ")
                .map(|(property, value)| (property.to_owned(), value.to_owned()));
            continue;
        }
        let Some(identity) = line
            .strip_prefix("  - Fact identity: `")
            .and_then(|value| value.strip_suffix('`'))
        else {
            continue;
        };
        let Some((property, value)) = pending.take() else {
            continue;
        };
        let Some(fact) = header
            .facts
            .iter_mut()
            .find(|fact| fact.fact_id == identity)
        else {
            continue;
        };
        if property != escape_markdown(&fact.property.replace('_', " ")) {
            continue;
        }
        let suffix = fact
            .qualifier
            .as_deref()
            .map(|value| format!(" ({})", escape_markdown(value)))
            .unwrap_or_default();
        if value == format!("{}{}", escape_markdown(&fact.value), suffix) {
            continue;
        }
        if let Some(value) = value
            .strip_suffix(&suffix)
            .and_then(decode_generated_fact_value)
        {
            fact.manual_correction = Some(ManualFactCorrection { value });
        }
    }
}

fn stage_authored_material(saved: &Path) -> Result<PathBuf> {
    verify_backup(saved)?;
    let files = saved.join("files");
    let mut authored = Vec::new();
    for relative in inventory(&files)? {
        if !relative.starts_with("pages") || relative.extension().is_none_or(|ext| ext != "md") {
            continue;
        }
        let path = files.join(&relative);
        if !fs::symlink_metadata(&path)?.is_file() {
            continue;
        }
        if fs::metadata(&path)?.len() > MAX_PAGE_BYTES {
            authored.push(AuthoredMaterial {
                page_id: format!("unparsed:{}", relative.display()),
                corrections: Vec::new(),
                prose: Vec::new(),
                opaque_saves: vec![OpaqueOwnerSave {
                    path: relative.to_string_lossy().into_owned(),
                    bytes: Vec::new(),
                }],
            });
            continue;
        }
        let raw = fs::read(&path)?;
        let parsed = std::str::from_utf8(&raw).ok().and_then(|text| {
            parse_knowledge_header(text)
                .ok()
                .map(|header| (text, header))
        });
        let material = if let Some((markdown, mut header)) = parsed {
            capture_snapshot_fact_edits(&mut header, markdown);
            let mut material = extract_authored_material(&files, &header)?;
            // A page body can mix generated prose with uncaptured owner text.
            // Preserve its exact bytes in the verified backup, without treating
            // the mixed body as independent semantic evidence.
            material.opaque_saves.push(OpaqueOwnerSave {
                path: relative.to_string_lossy().into_owned(),
                bytes: Vec::new(),
            });
            material
        } else {
            AuthoredMaterial {
                page_id: format!("unparsed:{}", relative.display()),
                corrections: Vec::new(),
                prose: Vec::new(),
                opaque_saves: vec![OpaqueOwnerSave {
                    path: relative.to_string_lossy().into_owned(),
                    bytes: raw,
                }],
            }
        };
        if !material.corrections.is_empty()
            || !material.prose.is_empty()
            || !material.opaque_saves.is_empty()
        {
            authored.push(material);
        }
    }
    let bytes = serde_json::to_vec(&authored)?;
    let path = saved.join("authored-material.json");
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() && fs::read(&path)? == bytes => (),
        Ok(_) => {
            return Err(GardenError::Invalid(
                "The staged authored evidence changed; original backup remains intact.".into(),
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => write_atomic(&path, &bytes)?,
        Err(error) => return Err(error.into()),
    }
    Ok(path)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AuthoredImportIntent {
    source_id: String,
    source_file: PathBuf,
    expected_original_sha256: String,
    expected_bundle_sha256: Option<String>,
}

fn plan_authored_imports(saved: &Path) -> Result<Vec<AuthoredImportIntent>> {
    let mut intents = Vec::new();
    for source in stage_authored_sources(saved)? {
        let digest = hex::encode(digest_file(&source)?);
        let scope = format!("migration-authored:{digest}");
        let source_id = format!(
            "source-{}",
            hex::encode(Sha256::digest(
                format!("repository-source:txt:{scope}").as_bytes()
            ))
        );
        intents.push(AuthoredImportIntent {
            source_id,
            source_file: source
                .strip_prefix(saved)
                .map_err(|_| GardenError::Invalid("Authored evidence escaped its backup.".into()))?
                .to_path_buf(),
            expected_original_sha256: digest,
            expected_bundle_sha256: None,
        });
    }
    Ok(intents)
}

fn stage_import_journal(saved: &Path) -> Result<PathBuf> {
    let intents = plan_authored_imports(saved)?;
    let bytes = serde_json::to_vec(&intents)?;
    let path = saved.join("authored-imports.json");
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => {
            let existing: Vec<AuthoredImportIntent> = serde_json::from_slice(&fs::read(&path)?)?;
            if existing.len() != intents.len()
                || existing.iter().zip(&intents).any(|(a, b)| {
                    a.source_id != b.source_id
                        || a.source_file != b.source_file
                        || a.expected_original_sha256 != b.expected_original_sha256
                })
            {
                return Err(GardenError::Invalid(
                    "The authored import journal changed; migration remains pending.".into(),
                ));
            }
        }
        Ok(_) => {
            return Err(GardenError::Invalid(
                "The authored import journal is not a regular file.".into(),
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => write_atomic(&path, &bytes)?,
        Err(error) => return Err(error.into()),
    }
    Ok(path)
}

fn ensure_unrelated_collection_files_match(
    root: &Path,
    saved: &Path,
    intents: &[AuthoredImportIntent],
) -> Result<()> {
    verify_backup(saved)?;
    let database = saved.join("manifest.sqlite");
    let v3 = database.exists();
    let expected: Vec<(PathBuf, Option<String>)> = if v3 {
        let mut entries = Vec::new();
        let mut cursor = None;
        while let Some((key, entry)) = manifest_records::next(&database, cursor.as_deref())? {
            entries.push((entry.path, entry.digest));
            cursor = Some(key);
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    } else {
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(saved.join("manifest.json"))?)?;
        if value.is_array() {
            serde_json::from_value(value)?
        } else {
            serde_json::from_value::<ManifestV2>(value)?.entries
        }
    };
    let migration_directories = intents
        .iter()
        .map(|intent| {
            let digest = intent.source_id.strip_prefix("source-").unwrap_or("");
            PathBuf::from("sources").join(digest)
        })
        .collect::<Vec<_>>();
    if expected.iter().any(|(path, _)| {
        migration_directories
            .iter()
            .any(|directory| path.starts_with(directory))
    }) {
        return Err(GardenError::Invalid(
            "A migration source identity collides with the backup.".into(),
        ));
    }
    let current: Vec<_> = inventory(root)?
        .into_iter()
        .filter(|path| {
            !migration_directories
                .iter()
                .any(|directory| path.starts_with(directory))
        })
        .collect();
    let actual = if v3 {
        current
            .iter()
            .map(|path| {
                let metadata = fs::symlink_metadata(root.join(path))?;
                let digest = if metadata.is_dir() {
                    None
                } else if metadata.is_file() {
                    Some(manifest_cursor::digest_file(root, path)?)
                } else {
                    return Err(GardenError::Recovery(
                        "Collection member changed type during migration.".into(),
                    ));
                };
                Ok((path.clone(), digest))
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        manifest(root, &current)?
    };
    if actual != expected {
        let changed = actual
            .iter()
            .find(|entry| !expected.contains(entry))
            .or_else(|| expected.iter().find(|entry| !actual.contains(entry)))
            .map(|entry| entry.0.display().to_string())
            .unwrap_or_default();
        return Err(GardenError::Invalid(format!("Collection changed during migration import at {changed}; original files are retained and migration remains pending.")));
    }
    Ok(())
}

fn import_authored_sources(app: &mut Application, saved: &Path) -> Result<()> {
    import_authored_sources_with_hook(app, saved, || {})
}

fn import_authored_sources_with_hook(
    app: &mut Application,
    saved: &Path,
    mut after_import: impl FnMut(),
) -> Result<()> {
    let journal = stage_import_journal(saved)?;
    let mut intents: Vec<AuthoredImportIntent> = serde_json::from_slice(&fs::read(&journal)?)?;
    for index in 0..intents.len() {
        ensure_unrelated_collection_files_match(&app.root, saved, &intents)?;
        let intent = &intents[index];
        let directory = app.source_dir(&intent.source_id)?;
        if let Some(expected) = &intent.expected_bundle_sha256 {
            if bundle_digest(&directory)? != *expected {
                return Err(GardenError::Invalid(
                    "An imported authored source changed; migration remains pending.".into(),
                ));
            }
            continue;
        }
        let source = saved.join(&intent.source_file);
        validate_relative_collection_path(&intent.source_file)?;
        if hex::encode(digest_file(&source)?) != intent.expected_original_sha256 {
            return Err(GardenError::Invalid(
                "Authored source evidence changed before import.".into(),
            ));
        }
        if directory.exists() {
            validate_unrecorded_import(&directory, &source, intent)?;
            ensure_unrelated_collection_files_match(&app.root, saved, &intents)?;
            intents[index].expected_bundle_sha256 = Some(bundle_digest(&directory)?);
            write_atomic(&journal, &serde_json::to_vec(&intents)?)?;
            continue;
        }
        let scope = format!("migration-authored:{}", intent.expected_original_sha256);
        app.migration_import = true;
        let result = app.import_source_with_identity(
            &source,
            AcquisitionMethod::Picker,
            None,
            None,
            Some(scope),
            None,
        );
        app.migration_import = false;
        let imported = result?;
        after_import();
        if imported.info.source_id != intent.source_id
            || imported.info.sha256 != intent.expected_original_sha256
        {
            return Err(GardenError::Invalid(
                "The imported authored source did not match the journal.".into(),
            ));
        }
        ensure_unrelated_collection_files_match(&app.root, saved, &intents)?;
        intents[index].expected_bundle_sha256 = Some(bundle_digest(&directory)?);
        write_atomic(&journal, &serde_json::to_vec(&intents)?)?;
    }
    Ok(())
}

fn validate_unrecorded_import(
    directory: &Path,
    source: &Path,
    intent: &AuthoredImportIntent,
) -> Result<()> {
    let paths = inventory(directory)?;
    let plain = vec![PathBuf::from("index.md"), PathBuf::from("original.txt")];
    let seeded = vec![
        PathBuf::from(".trusted-index"),
        PathBuf::from("index.md"),
        PathBuf::from("original.txt"),
    ];
    if paths != plain && paths != seeded {
        return Err(GardenError::Invalid(
            "Unrecorded authored source has unexpected files; migration remains pending.".into(),
        ));
    }
    if paths == seeded {
        let index = read_bounded_text(&directory.join("index.md"), MAX_PAGE_BYTES)?;
        trusted_pages::validate_import_seed(&directory.join(".trusted-index"), index.as_bytes())?;
    }
    let page = read_page(&directory.join("index.md"))?;
    let info = &page.info;
    let expected_path = source.to_string_lossy().into_owned();
    let bytes = fs::read(source)?;
    if info.source_id != intent.source_id
        || info.sha256 != intent.expected_original_sha256
        || info.format != "txt"
        || info.asset != "original.txt"
        || info.bytes != bytes.len() as u64
        || info.acquisitions.len() != 1
        || info.acquisitions[0].path != expected_path
        || info.acquisitions[0].method != AcquisitionMethod::Picker
        || info.versions_seen.len() != 1
        || info.versions_seen[0].sha256 != info.sha256
        || info.versions_seen[0].source_version_id != info.sha256
        || info.versions_seen[0].asset != info.asset
        || !info.owner_metadata.is_empty()
        || !info.knowledge_pages.is_empty()
        || !info.semantic_decisions.is_empty()
        || digest_file(&directory.join("original.txt"))? != digest_file(source)?
    {
        return Err(GardenError::Invalid("Unrecorded authored source does not match the migration intent; migration remains pending.".into()));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| GardenError::Invalid("Authored source is not valid UTF-8.".into()))?;
    if page.markdown != serialize_page(info, &source_body(info, text, None))? {
        return Err(GardenError::Invalid(
            "Unrecorded authored source page changed; migration remains pending.".into(),
        ));
    }
    Ok(())
}

fn bundle_digest(directory: &Path) -> Result<String> {
    let paths = inventory(directory)?;
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(&manifest(
        directory, &paths,
    )?)?)))
}

fn stage_authored_sources(saved: &Path) -> Result<Vec<PathBuf>> {
    let staged = stage_authored_material(saved)?;
    let materials: Vec<AuthoredMaterial> = serde_json::from_slice(&fs::read(staged)?)?;
    let mut paths = Vec::new();
    let directory = saved.join("authored-sources");
    fs::create_dir_all(&directory)?;
    if !fs::symlink_metadata(&directory)?.is_dir() {
        return Err(GardenError::Invalid(
            "Staged source directory is not a regular directory.".into(),
        ));
    }
    for material in materials {
        if material.corrections.is_empty() && material.prose.is_empty() {
            continue;
        }
        let bytes = serde_json::to_vec_pretty(&serde_json::json!({
            "qualification": "Historical owner-authored statements retained during migration. These statements are not verified and may be outdated.",
            "legacy_page_id": material.page_id,
            "corrections": material.corrections,
            "owner_prose": material.prose,
        }))?;
        let path = directory.join(format!("{}.txt", hex::encode(Sha256::digest(&bytes))));
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() && fs::read(&path)? == bytes => (),
            Ok(_) => {
                return Err(GardenError::Invalid(
                    "Staged authored source evidence changed.".into(),
                ))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                write_atomic(&path, &bytes)?
            }
            Err(error) => return Err(error.into()),
        }
        paths.push(path);
    }
    sync_directory(&directory)?;
    Ok(paths)
}

// A later stage must use this guard before modifying pages, then retain any
// saves displaced during publication. External editors do not hold our lock.
#[cfg(test)]
fn ensure_collection_matches_backup(root: &Path, saved: &Path) -> Result<()> {
    verify_backup(saved)?;
    if saved.join("manifest.sqlite").exists() {
        verify_v3(root, &saved.join("manifest.sqlite"))?;
    } else {
        verify_manifest(root, &fs::read(saved.join("manifest.json"))?).map_err(|_| GardenError::Invalid("Collection changed after migration backup; create a fresh verified snapshot before modifying pages.".into()))?;
    }
    Ok(())
}

// Restore into a new directory so a failed recovery cannot overwrite a live collection.
pub(super) fn restore_to_new_directory(saved: &Path, destination: &Path) -> Result<()> {
    let parent = destination.parent().ok_or_else(|| {
        GardenError::Invalid("Recovery destination has no parent directory.".into())
    })?;
    if fs::canonicalize(parent)?.starts_with(fs::canonicalize(saved)?) {
        return Err(GardenError::Invalid(
            "Recovery destination must be outside the backup.".into(),
        ));
    }
    let files = saved.join("files");
    let v3 = saved.join("manifest.sqlite");
    let manifest_bytes = if v3.exists() {
        verify_v3(&files, &v3)?;
        None
    } else {
        Some(fs::read(saved.join("manifest.json"))?)
    };
    let paths = if let Some(bytes) = &manifest_bytes {
        verify_manifest(&files, bytes)?
    } else {
        inventory(&files)?
    };
    fs::create_dir(destination)?;
    for relative in &paths {
        let source = files.join(relative);
        let target = destination.join(relative);
        if fs::symlink_metadata(&source)?.is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            fs::create_dir_all(target.parent().unwrap())?;
            fs::copy(&source, &target)?;
            File::open(&target)?.sync_all()?;
        }
    }
    if inventory(&files)? != paths || inventory(destination)? != paths {
        return Err(GardenError::Invalid(
            "Backup changed during recovery; the restored directory is incomplete.".into(),
        ));
    }
    for relative in &paths {
        let source = files.join(relative);
        if fs::symlink_metadata(&source)?.is_file()
            && digest_file(&source)? != digest_file(&destination.join(relative))?
        {
            return Err(GardenError::Invalid(
                "Recovery verification failed; the restored directory is incomplete.".into(),
            ));
        }
    }
    for relative in paths.iter().rev() {
        let directory = destination.join(relative);
        if directory.is_dir() {
            sync_directory(&directory)?;
        }
    }
    if let Some(bytes) = &manifest_bytes {
        verify_manifest(destination, bytes).map_err(|_| {
            GardenError::Invalid(
                "Recovery verification failed; the restored directory is incomplete.".into(),
            )
        })?;
    } else {
        verify_v3(destination, &v3)?;
    }
    sync_directory(destination)?;
    sync_directory(destination.parent().unwrap())?;
    Ok(())
}

fn manifest(root: &Path, paths: &[PathBuf]) -> Result<Vec<(PathBuf, Option<String>)>> {
    paths
        .iter()
        .map(|path| {
            let full = root.join(path);
            let kind = fs::symlink_metadata(&full)?.file_type();
            let digest = if kind.is_file() {
                Some(hex::encode(digest_file(&full)?))
            } else if kind.is_dir() {
                None
            } else {
                return Err(GardenError::Invalid(format!(
                    "Cannot back up non-regular collection entry {}.",
                    path.display()
                )));
            };
            Ok((path.clone(), digest))
        })
        .collect()
}

fn inventory(root: &Path) -> Result<Vec<PathBuf>> {
    let mut pending = vec![PathBuf::new()];
    let mut paths = Vec::new();
    while let Some(relative) = pending.pop() {
        for entry in fs::read_dir(root.join(&relative))? {
            let entry = entry?;
            let path = relative.join(entry.file_name());
            if (relative.as_os_str().is_empty()
                && [".derived", ".collection.lock"]
                    .iter()
                    .any(|name| path == Path::new(name)))
                || path == Path::new(".recovery/migration")
            {
                continue;
            }
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(path.clone());
            } else if !kind.is_file() {
                return Err(GardenError::Invalid(format!(
                    "Cannot back up non-regular collection entry {}.",
                    path.display()
                )));
            }
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn digest_file(path: &Path) -> Result<Vec<u8>> {
    let mut input = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut bytes = [0; 64 * 1024];
    loop {
        let count = input.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        hasher.update(&bytes[..count]);
    }
    Ok(hasher.finalize().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_constructor_gates_collection_before_backup_starts() {
        let workspace = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(workspace.path()).unwrap();
        let entered = Arc::new(std::sync::Barrier::new(2));
        let release = Arc::new(std::sync::Barrier::new(2));
        *PAUSE_PRE_RECOVERY.lock().unwrap() =
            Some((root.clone(), Arc::clone(&entered), Arc::clone(&release)));
        let mut app = Application::open_pending_with_all_providers_and_meaning_assets(
            &root,
            Arc::new(UnavailableProvider),
            Arc::new(LocalExtractor),
            Arc::new(NativeAudioProcessor),
            None::<PathBuf>,
        )
        .unwrap();
        assert!(app.startup_recovery_pending);
        assert!(app.require_integrity_recovered().is_err());
        let status = app.collection_status();
        assert!(!status.complete);
        assert!(status
            .diagnostics
            .iter()
            .any(|item| item.kind == "startup_recovery_pending"));
        let worker = std::thread::spawn(move || app.resume_pending_startup());
        entered.wait();
        assert!(!root.join(".derived/lookup.sqlite").exists());
        release.wait();
        *PAUSE_PRE_RECOVERY.lock().unwrap() = None;
        worker.join().unwrap().unwrap();
    }

    #[test]
    fn pending_constructor_restart_keeps_originals_and_gates_reads() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        fs::write(root.join("owner.txt"), b"owner content").unwrap();
        let open = || {
            Application::open_pending_with_all_providers_and_meaning_assets(
                root,
                Arc::new(UnavailableProvider),
                Arc::new(LocalExtractor),
                Arc::new(NativeAudioProcessor),
                None::<PathBuf>,
            )
            .unwrap()
        };
        let mut app = open();
        assert!(!app.collection_status().complete);
        assert!(app.require_integrity_recovered().is_err());
        assert!(app.list_sources(0).is_err());
        assert!(app
            .import_source(root.join("owner.txt"), AcquisitionMethod::Picker)
            .is_err());
        drop(app);
        assert_eq!(fs::read(root.join("owner.txt")).unwrap(), b"owner content");
        let mut app = open();
        assert!(app.require_integrity_recovered().is_err());
        app.resume_pending_startup().unwrap();
        assert!(app.collection_status().complete);
        assert_eq!(fs::read(root.join("owner.txt")).unwrap(), b"owner content");
    }

    #[test]
    fn backup_preserves_sources_and_owner_bytes_without_indexing_recovery() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        fs::create_dir_all(root.join("sources/a")).unwrap();
        fs::create_dir_all(root.join("pages")).unwrap();
        fs::create_dir_all(root.join(".derived")).unwrap();
        fs::create_dir_all(root.join(".recovery/removed")).unwrap();
        fs::write(
            root.join(".recovery/removed/original"),
            b"recoverable source",
        )
        .unwrap();
        fs::write(root.join("sources/a/original"), b"original\0bytes").unwrap();
        fs::write(root.join("pages/owner.md"), b"owner correction").unwrap();
        fs::write(root.join(".derived/index"), b"disposable").unwrap();
        let saved = backup(root).unwrap();
        ensure_collection_matches_backup(root, &saved).unwrap();
        fs::write(root.join("pages/owner.md"), b"changed").unwrap();
        assert!(ensure_collection_matches_backup(root, &saved).is_err());
        assert_eq!(
            fs::read(saved.join("files/sources/a/original")).unwrap(),
            b"original\0bytes"
        );
        assert_eq!(
            fs::read(saved.join("files/pages/owner.md")).unwrap(),
            b"owner correction"
        );
        assert!(!saved.join("files/.derived").exists());
        assert_eq!(
            fs::read(saved.join("files/.recovery/removed/original")).unwrap(),
            b"recoverable source"
        );
        let restored = root.join("restored");
        restore_to_new_directory(&saved, &restored).unwrap();
        assert_eq!(
            fs::read(restored.join("pages/owner.md")).unwrap(),
            b"owner correction"
        );
        assert_eq!(
            fs::read(restored.join("sources/a/original")).unwrap(),
            b"original\0bytes"
        );
        assert!(restore_to_new_directory(&saved, &restored).is_err());
        fs::write(saved.join("files/pages/owner.md"), b"corrupt").unwrap();
        let rejected = root.join("rejected");
        assert!(restore_to_new_directory(&saved, &rejected).is_err());
        assert!(!rejected.exists());
    }

    #[test]
    fn restored_collection_opens_with_original_source() {
        let workspace = tempfile::tempdir().unwrap();
        let collection = workspace.path().join("collection");
        let source = workspace.path().join("source.txt");
        fs::write(&source, b"The original source remains readable.").unwrap();
        let mut app = Application::open(&collection).unwrap();
        let imported = app
            .import_source(&source, AcquisitionMethod::Picker)
            .unwrap();
        let saved = backup(&collection).unwrap();
        drop(app);
        let restored = workspace.path().join("restored");
        restore_to_new_directory(&saved, &restored).unwrap();
        let app = Application::open(&restored).unwrap();
        assert_eq!(
            fs::read(app.original_path(&imported.info.source_id).unwrap()).unwrap(),
            b"The original source remains readable."
        );
    }

    #[test]
    fn existing_manual_value_becomes_evidence_and_supported_value_remains_active() {
        let workspace = tempfile::tempdir().unwrap();
        let collection = workspace.path().join("collection");
        let source_path = workspace.path().join("input.txt");
        fs::write(&source_path, b"Count was 10.").unwrap();
        let mut app = Application::open(&collection).unwrap();
        let source = app
            .import_source(&source_path, AcquisitionMethod::Picker)
            .unwrap();
        let evidence = serde_json::json!({"quote":"Count was 10", "byte_start":0,"byte_end":12,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({
            "schema":1,"page_id":"page-legacy","source_id":source.info.source_id,"title":"Legacy count","kind":"topic","evidence":evidence,
            "facts":[{"fact_id":"fact-count","subject_page_id":"page-legacy","property":"count","value":"99","qualifier":null,"origin":"manual_correction","evidence":evidence,
                "manual_correction":{"value":"99"},"supports":[{"source_id":source.info.source_id,"source_version_id":source.info.sha256,"value":"10","qualifier":null,"origin":"source","evidence":evidence}]}]
        })).unwrap();
        fs::create_dir_all(collection.join("pages")).unwrap();
        let page = collection.join("pages/legacy.md");
        fs::write(&page, format!("---\n{}---\n\n# Legacy count\n\n- **count:** 99\n  - Fact identity: `fact-count`\n", serde_yaml_ng::to_string(&header).unwrap())).unwrap();
        drop(app);
        fs::remove_dir_all(collection.join(".recovery")).unwrap();
        let staging = collection.join(".recovery/verified-original-staging");
        fs::create_dir_all(&staging).unwrap();
        fs::write(
            staging.join(".staged-legacy"),
            b"pre-migration staged bytes",
        )
        .unwrap();
        let app = Application::open(&collection).unwrap();
        let current = app.open_knowledge_page("page-legacy").unwrap();
        assert!(current.markdown.contains("- **count:** 10"));
        assert!(!current.markdown.contains("- **count:** 99"));
        let state: MigrationState = serde_json::from_slice(
            &fs::read(collection.join(".recovery/migration/state.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(state.phase, "authority_retired");
        let saved = collection.join(".recovery/migration").join(state.backup);
        assert_eq!(
            fs::read(saved.join("files/.recovery/verified-original-staging/.staged-legacy"))
                .unwrap(),
            b"pre-migration staged bytes"
        );
        assert!(
            fs::read_dir(collection.join(".recovery/abandoned-verified-originals"))
                .unwrap()
                .any(
                    |entry| fs::read(entry.unwrap().path().join("content")).unwrap()
                        == b"pre-migration staged bytes"
                )
        );
        let journal: Vec<AuthoredImportIntent> =
            serde_json::from_slice(&fs::read(saved.join("authored-imports.json")).unwrap())
                .unwrap();
        let authored = app.open_source(&journal[0].source_id).unwrap();
        assert!(authored.body.contains("99"));
        fs::write(&page, b"external replacement").unwrap();
        assert!(app
            .open_knowledge_page("page-legacy")
            .unwrap()
            .markdown
            .contains("- **count:** 10"));
        drop(app);
        let app = Application::open(&collection).unwrap();
        assert!(app
            .open_knowledge_page("page-legacy")
            .unwrap()
            .markdown
            .contains("- **count:** 10"));
    }

    #[test]
    fn malformed_page_is_archived_without_a_semantic_source() {
        let workspace = tempfile::tempdir().unwrap();
        fs::create_dir(workspace.path().join("pages")).unwrap();
        fs::write(
            workspace.path().join("pages/broken.md"),
            b"\xffowner text\0",
        )
        .unwrap();
        let saved = backup(workspace.path()).unwrap();
        let staged = stage_authored_material(&saved).unwrap();
        let material: Vec<AuthoredMaterial> =
            serde_json::from_slice(&fs::read(staged).unwrap()).unwrap();
        assert_eq!(material.len(), 1);
        assert!(material[0].page_id.starts_with("unparsed:"));
        assert_eq!(material[0].opaque_saves[0].bytes, b"\xffowner text\0");
        assert!(stage_authored_sources(&saved).unwrap().is_empty());
    }

    #[test]
    fn migrated_ambiguous_save_has_inspectable_recovery_diagnostic() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        fs::create_dir_all(root.join("pages")).unwrap();
        fs::write(root.join("pages/broken.md"), b"\xffmixed owner material\0").unwrap();
        let app = Application::open(&root).unwrap();
        let diagnostics = app.collection_status().diagnostics;
        let diagnostic = diagnostics
            .iter()
            .find(|item| item.kind == "migration_ambiguous_owner_save")
            .unwrap();
        assert_eq!(
            fs::read(root.join(&diagnostic.paths[0])).unwrap(),
            b"\xffmixed owner material\0"
        );
        assert_eq!(
            fs::read(app.collection_file_path(&diagnostic.paths[0]).unwrap()).unwrap(),
            b"\xffmixed owner material\0"
        );
        assert!(app.list_sources(0).unwrap().sources.is_empty());
        drop(app);
        let app = Application::open(&root).unwrap();
        assert!(app
            .collection_status()
            .diagnostics
            .iter()
            .any(|item| item.kind == "migration_ambiguous_owner_save"));
    }

    #[test]
    fn authored_import_records_bundle_and_rejects_changed_bundle() {
        let workspace = tempfile::tempdir().unwrap();
        let collection = workspace.path().join("collection");
        let source = workspace.path().join("source.txt");
        fs::write(&source, b"Original material.").unwrap();
        let mut app = Application::open(&collection).unwrap();
        let original = app
            .import_source(&source, AcquisitionMethod::Picker)
            .unwrap();
        fs::create_dir_all(collection.join("pages")).unwrap();
        let evidence = serde_json::json!({"quote":"Original", "byte_start":0,"byte_end":8,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({
            "schema":1,"page_id":"authored-page","source_id":original.info.source_id,"title":"A page","kind":"topic","evidence":evidence,"owner_content":["A personal note."]
        })).unwrap();
        fs::write(
            collection.join("pages/authored.md"),
            format!(
                "---\n{}---\n\n# A page\n",
                serde_yaml_ng::to_string(&header).unwrap()
            ),
        )
        .unwrap();
        fs::remove_dir_all(collection.join(".recovery")).unwrap();
        let saved = backup(&collection).unwrap();
        import_authored_sources(&mut app, &saved).unwrap();
        let journal: Vec<AuthoredImportIntent> =
            serde_json::from_slice(&fs::read(saved.join("authored-imports.json")).unwrap())
                .unwrap();
        assert_eq!(journal.len(), 1);
        assert!(journal[0].expected_bundle_sha256.is_some());
        ensure_unrelated_collection_files_match(&collection, &saved, &journal).unwrap();
        let edited_page = collection.join("pages/authored.md");
        let before_edit = fs::read(&edited_page).unwrap();
        fs::write(&edited_page, b"external edit during migration").unwrap();
        assert!(ensure_unrelated_collection_files_match(&collection, &saved, &journal).is_err());
        fs::write(&edited_page, before_edit).unwrap();
        let original_path = app.original_path(&journal[0].source_id).unwrap();
        assert!(fs::read_to_string(original_path)
            .unwrap()
            .contains("A personal note."));
        import_authored_sources(&mut app, &saved).unwrap();
        let mut interrupted = journal.clone();
        interrupted[0].expected_bundle_sha256 = None;
        fs::write(
            saved.join("authored-imports.json"),
            serde_json::to_vec(&interrupted).unwrap(),
        )
        .unwrap();
        import_authored_sources(&mut app, &saved).unwrap();
        let recovered: Vec<AuthoredImportIntent> =
            serde_json::from_slice(&fs::read(saved.join("authored-imports.json")).unwrap())
                .unwrap();
        assert_eq!(recovered, journal);
        fs::write(
            app.source_dir(&journal[0].source_id)
                .unwrap()
                .join("unexpected"),
            b"changed",
        )
        .unwrap();
        assert!(import_authored_sources(&mut app, &saved).is_err());
    }

    #[test]
    fn imported_authored_source_is_searchable_and_supports_qualified_knowledge() {
        struct RecordedOwnerProvider;
        impl SemanticProvider for RecordedOwnerProvider {
            fn form_knowledge(
                &self,
                text: &str,
            ) -> std::result::Result<KnowledgeDraft, ProviderError> {
                let quote = "A personal note.";
                let Some(start) = text.find(quote) else {
                    return Err(ProviderError::unavailable(
                        "No owner note in this fixture.".into(),
                        crate::semantic::ProviderAvailability::CapabilityUnavailable,
                    ));
                };
                let evidence = EvidenceDraft {
                    quote: quote.into(),
                    byte_start: start,
                    byte_end: start + quote.len(),
                    origin: "source".into(),
                    qualifier: Some("Historical owner statement, unverified".into()),
                    offset_basis: None,
                    source_location: None,
                };
                Ok(KnowledgeDraft {
                    entities: vec![EntityDraft {
                        kind: "topic".into(),
                        label: "Personal note".into(),
                        evidence: evidence.clone(),
                    }],
                    facts: vec![crate::semantic::FactDraft {
                        subject: "Personal note".into(),
                        property: "statement".into(),
                        value: quote.into(),
                        evidence,
                        record_key: None,
                    }],
                    ..Default::default()
                })
            }
        }
        let workspace = tempfile::tempdir().unwrap();
        let collection = workspace.path().join("collection");
        let source = workspace.path().join("source.txt");
        fs::write(&source, b"Original material.").unwrap();
        let mut app =
            Application::open_with_semantic_provider(&collection, Arc::new(RecordedOwnerProvider))
                .unwrap();
        let original = app
            .import_source(&source, AcquisitionMethod::Picker)
            .unwrap();
        fs::create_dir_all(collection.join("pages")).unwrap();
        let evidence = serde_json::json!({"quote":"Original", "byte_start":0,"byte_end":8,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({
            "schema":1,"page_id":"authored-page","source_id":original.info.source_id,"title":"A page","kind":"topic","evidence":evidence,"owner_content":["A personal note."]
        })).unwrap();
        fs::write(
            collection.join("pages/authored.md"),
            format!(
                "---\n{}---\n\n# A page\n",
                serde_yaml_ng::to_string(&header).unwrap()
            ),
        )
        .unwrap();
        fs::remove_dir_all(collection.join(".recovery")).unwrap();
        let saved = backup(&collection).unwrap();
        import_authored_sources(&mut app, &saved).unwrap();
        let journal: Vec<AuthoredImportIntent> =
            serde_json::from_slice(&fs::read(saved.join("authored-imports.json")).unwrap())
                .unwrap();
        let owner_source = app.open_source(&journal[0].source_id).unwrap();
        assert_eq!(owner_source.info.extraction, ExtractionState::TextPreserved);
        assert!(owner_source
            .body
            .contains("Historical owner-authored statements"));
        app.resume_due_semantic_jobs().unwrap();
        let owner_source = app.open_source(&journal[0].source_id).unwrap();
        assert!(!owner_source.knowledge_pages.is_empty());
        let page = app
            .open_knowledge_page(&owner_source.knowledge_pages[0].page_id)
            .unwrap();
        assert!(page.markdown.contains("A personal note."));
        assert!(page
            .markdown
            .contains("Historical owner statement, unverified"));
        let results = app
            .search_pages(PageSearchRequest {
                query: "personal note".into(),
                ..Default::default()
            })
            .unwrap();
        assert!(results
            .pages
            .iter()
            .any(|result| result.source_id == journal[0].source_id));
    }

    #[test]
    fn external_save_during_import_is_preserved_and_migration_stays_pending() {
        let workspace = tempfile::tempdir().unwrap();
        let collection = workspace.path().join("collection");
        let source = workspace.path().join("source.txt");
        fs::write(&source, b"Original material.").unwrap();
        let mut app = Application::open(&collection).unwrap();
        let original = app
            .import_source(&source, AcquisitionMethod::Picker)
            .unwrap();
        fs::create_dir_all(collection.join("pages")).unwrap();
        let evidence = serde_json::json!({"quote":"Original", "byte_start":0,"byte_end":8,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({
            "schema":1,"page_id":"authored-page","source_id":original.info.source_id,"title":"A page","kind":"topic","evidence":evidence,"owner_content":["A personal note."]
        })).unwrap();
        let page = collection.join("pages/authored.md");
        let before = format!(
            "---\n{}---\n\n# A page\n",
            serde_yaml_ng::to_string(&header).unwrap()
        );
        fs::write(&page, &before).unwrap();
        fs::remove_dir_all(collection.join(".recovery")).unwrap();
        let saved = backup(&collection).unwrap();
        let result = import_authored_sources_with_hook(&mut app, &saved, || {
            fs::write(&page, b"concurrent save").unwrap()
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&page).unwrap(), b"concurrent save");
        assert_eq!(
            fs::read(saved.join("files/pages/authored.md")).unwrap(),
            before.as_bytes()
        );
        let journal: Vec<AuthoredImportIntent> =
            serde_json::from_slice(&fs::read(saved.join("authored-imports.json")).unwrap())
                .unwrap();
        assert!(journal
            .iter()
            .all(|intent| intent.expected_bundle_sha256.is_none()));
        assert!(import_authored_sources(&mut app, &saved).is_err());
    }

    #[test]
    fn startup_records_and_rechecks_backup_across_restart() {
        let workspace = tempfile::tempdir().unwrap();
        let collection = workspace.path().join("collection");
        let source = workspace.path().join("source.txt");
        fs::write(&source, b"The source survives migration preparation.").unwrap();
        let mut app = Application::open(&collection).unwrap();
        app.import_source(&source, AcquisitionMethod::Picker)
            .unwrap();
        drop(app);
        let app = Application::open(&collection).unwrap();
        let state_path = collection.join(".recovery/migration/state.json");
        let initial = fs::read(&state_path).unwrap();
        let state: MigrationState = serde_json::from_slice(&initial).unwrap();
        assert_eq!(state.phase, "authority_retired");
        drop(app);
        let app = Application::open(&collection).unwrap();
        assert_eq!(fs::read(&state_path).unwrap(), initial);
        drop(app);
        let saved = collection.join(".recovery/migration").join(state.backup);
        fs::write(saved.join("manifest.json"), b"corrupt").unwrap();
        assert!(Application::open(&collection).is_err());
        assert!(source.exists());
    }

    #[test]
    fn authored_material_keeps_corrections_and_opaque_saves_without_generated_facts() {
        let workspace = tempfile::tempdir().unwrap();
        fs::create_dir(workspace.path().join("owner-edits")).unwrap();
        fs::write(
            workspace.path().join("owner-edits/save.md"),
            b"mixed owner and generated bytes\0",
        )
        .unwrap();
        let evidence = serde_json::json!({"quote":"source", "byte_start":0,"byte_end":6,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({
            "schema": 1, "page_id":"page-1", "source_id":"source-1", "title":"Generated context", "kind":"event", "evidence":evidence,
            "facts":[
                {"fact_id":"fact-1", "subject_page_id":"page-1", "property":"duration", "value":"generated value", "qualifier":"approximate", "origin":"manual_correction", "evidence":evidence, "manual_correction":{"value":"owner's exact value"}},
                {"fact_id":"fact-2", "subject_page_id":"page-1", "property":"count", "value":"generated only", "qualifier":null, "origin":"source", "evidence":evidence}
            ],
            "owner_content":["owner's exact prose  "], "uninterpreted_owner_edits":["owner-edits/save.md"]
        })).unwrap();
        let mut edited = header.clone();
        capture_snapshot_fact_edits(
            &mut edited,
            "---\nmetadata\n---\n- **count:** 23\n  - Fact identity: `fact-2`\n",
        );
        assert_eq!(
            edited.facts[1].manual_correction.as_ref().unwrap().value,
            "23"
        );
        capture_snapshot_fact_edits(
            &mut edited,
            "---\nmetadata\n---\n- **count:** 24\n  - Fact identity: `unknown`\n",
        );
        assert_eq!(
            edited.facts[1].manual_correction.as_ref().unwrap().value,
            "23"
        );
        let material = extract_authored_material(workspace.path(), &header).unwrap();
        assert_eq!(
            material.corrections,
            vec![AuthoredCorrection {
                fact_id: "fact-1".into(),
                property: "duration".into(),
                value: "owner's exact value".into(),
                qualifier: Some("approximate".into())
            }]
        );
        assert_eq!(material.prose, vec!["owner's exact prose  "]);
        assert_eq!(
            material.opaque_saves[0].bytes,
            b"mixed owner and generated bytes\0"
        );
        assert!(!format!("{material:?}").contains("generated only"));
        assert!(!format!("{material:?}").contains("generated value"));
        fs::create_dir(workspace.path().join("pages")).unwrap();
        fs::write(
            workspace.path().join("pages/page.md"),
            format!(
                "---\n{}---\n- **count:** 23\n  - Fact identity: `fact-2`\n",
                serde_yaml_ng::to_string(&header).unwrap()
            ),
        )
        .unwrap();
        let saved = backup(workspace.path()).unwrap();
        let staged = stage_authored_material(&saved).unwrap();
        let first = fs::read(&staged).unwrap();
        assert_eq!(stage_authored_material(&saved).unwrap(), staged);
        assert_eq!(fs::read(&staged).unwrap(), first);
        let decoded: Vec<AuthoredMaterial> = serde_json::from_slice(&first).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].prose, material.prose);
        assert!(decoded[0]
            .opaque_saves
            .iter()
            .any(|item| item == &material.opaque_saves[0]));
        assert!(decoded[0]
            .opaque_saves
            .iter()
            .any(|item| item.path == "pages/page.md"));
        assert_eq!(decoded[0].corrections.len(), 2);
        assert_eq!(decoded[0].corrections[1].value, "23");
        let sources = stage_authored_sources(&saved).unwrap();
        assert_eq!(sources.len(), 1);
        let source = fs::read_to_string(&sources[0]).unwrap();
        assert!(source.contains("owner's exact value"));
        assert!(source.contains("owner's exact prose"));
        assert!(source.contains("23"));
        assert!(!source.contains("generated only"));
        assert!(!source.contains("mixed owner and generated bytes"));
        assert_eq!(stage_authored_sources(&saved).unwrap(), sources);
        let intents = plan_authored_imports(&saved).unwrap();
        assert_eq!(intents.len(), 1);
        assert_eq!(
            intents[0].expected_original_sha256,
            hex::encode(Sha256::digest(source.as_bytes()))
        );
        assert_eq!(plan_authored_imports(&saved).unwrap(), intents);
        assert_eq!(intents[0].expected_bundle_sha256, None);
        let journal = stage_import_journal(&saved).unwrap();
        let saved_intents: Vec<AuthoredImportIntent> =
            serde_json::from_slice(&fs::read(&journal).unwrap()).unwrap();
        assert_eq!(saved_intents, intents);
        assert_eq!(stage_import_journal(&saved).unwrap(), journal);
        fs::write(&staged, b"tampered").unwrap();
        assert!(stage_authored_material(&saved).is_err());
        let mut escaped = header.clone();
        escaped.uninterpreted_owner_edits = vec!["../outside".into()];
        assert!(extract_authored_material(workspace.path(), &escaped).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn abrupt_pre_recovery_snapshot_child() {
        let Some(root) = std::env::var_os("KG_PRE_RECOVERY_TEST_ROOT") else {
            return;
        };
        let _ = Application::open(root);
        panic!("pre-recovery exit hook did not run");
    }

    #[test]
    fn abrupt_exit_before_actual_journal_replay_preserves_and_restores_both_states() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let transaction = root.join(".staging/transactions/pending");
        fs::create_dir_all(transaction.join("files")).unwrap();
        fs::create_dir_all(root.join("owner-edits")).unwrap();
        fs::write(root.join("owner-edits/owner.txt"), b"before replay").unwrap();
        fs::write(transaction.join("files/0000"), b"after replay").unwrap();
        let manifest = serde_json::json!({"entries": [{"destination":"owner-edits/owner.txt", "staged":"files/0000", "expected": b"before replay".to_vec()}]});
        fs::write(
            transaction.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::tests::abrupt_pre_recovery_snapshot_child",
            ])
            .env("KG_PRE_RECOVERY_TEST_ROOT", &root)
            .env("KG_STOP_AFTER_PRE_RECOVERY_BACKUP", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(77));
        assert_eq!(
            fs::read(root.join("owner-edits/owner.txt")).unwrap(),
            b"before replay"
        );
        assert!(transaction.join("manifest.json").is_file());
        let pre: String = serde_json::from_slice(
            &fs::read(root.join(".recovery/migration/pre-recovery.json")).unwrap(),
        )
        .unwrap();
        let pre_backup = root.join(".recovery/migration").join(pre);
        let before_restore = workspace.path().join("restore-before");
        restore_to_new_directory(&pre_backup, &before_restore).unwrap();
        assert_eq!(
            fs::read(before_restore.join("owner-edits/owner.txt")).unwrap(),
            b"before replay"
        );
        assert!(before_restore
            .join(".staging/transactions/pending/manifest.json")
            .is_file());
        drop(Application::open(&root).unwrap());
        assert_eq!(
            fs::read(root.join("owner-edits/owner.txt")).unwrap(),
            b"after replay"
        );
        assert!(!transaction.exists());
        let state: MigrationState =
            serde_json::from_slice(&fs::read(root.join(".recovery/migration/state.json")).unwrap())
                .unwrap();
        let after_restore = workspace.path().join("restore-after");
        restore_to_new_directory(
            &root.join(".recovery/migration").join(state.backup),
            &after_restore,
        )
        .unwrap();
        assert_eq!(
            fs::read(after_restore.join("owner-edits/owner.txt")).unwrap(),
            b"after replay"
        );
    }

    #[test]
    fn abrupt_pre_recovery_completion_child() {
        let Some(root) = std::env::var_os("KG_PRE_RECOVERY_COMPLETION_ROOT") else {
            return;
        };
        let _ = Application::open(root);
        panic!("completion exit hook did not run");
    }

    #[test]
    fn abrupt_exit_after_pre_recovery_completion_keeps_prior_snapshot() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let transaction = root.join(".staging/transactions/pending");
        fs::create_dir_all(transaction.join("files")).unwrap();
        fs::create_dir_all(root.join("owner-edits")).unwrap();
        fs::write(root.join("owner-edits/owner.txt"), b"before replay").unwrap();
        fs::write(transaction.join("files/0000"), b"after replay").unwrap();
        let manifest = serde_json::json!({"entries": [{"destination":"owner-edits/owner.txt", "staged":"files/0000", "expected": b"before replay".to_vec()}]});
        fs::write(
            transaction.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::tests::abrupt_pre_recovery_completion_child",
            ])
            .env("KG_PRE_RECOVERY_COMPLETION_ROOT", &root)
            .env("KG_STOP_AFTER_PRE_RECOVERY_COMPLETION", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(78));
        assert_eq!(
            fs::read(root.join("owner-edits/owner.txt")).unwrap(),
            b"after replay"
        );
        let history = root.join(".recovery/migration/pre-recovery-history");
        let first = fs::read_dir(&history).unwrap().next().unwrap().unwrap();
        let name: String = serde_json::from_slice(&fs::read(first.path()).unwrap()).unwrap();
        assert_eq!(
            fs::read(
                root.join(".recovery/migration")
                    .join(name)
                    .join("files/owner-edits/owner.txt")
            )
            .unwrap(),
            b"before replay"
        );
        drop(Application::open(&root).unwrap());
        assert_eq!(
            fs::read(root.join("owner-edits/owner.txt")).unwrap(),
            b"after replay"
        );
        assert_eq!(fs::read_dir(history).unwrap().count(), 2);
    }

    #[test]
    fn changed_pre_recovery_journal_is_retained_and_reported() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        fs::create_dir_all(&root).unwrap();
        prepare_pre_recovery(&root).unwrap();
        let journal = root.join(".recovery/migration/pre-recovery.json");
        let name: String = serde_json::from_slice(&fs::read(&journal).unwrap()).unwrap();
        let result = finish_pre_recovery_with_hook(&root, None, || {
            fs::write(&journal, b"concurrent journal bytes").unwrap()
        });
        assert!(result.is_err());
        let saved = root
            .join(".recovery/migration/pre-recovery-history")
            .join(format!("{name}.json"));
        assert_eq!(fs::read(saved).unwrap(), b"concurrent journal bytes");
        assert!(root.join(".recovery/migration").join(name).is_dir());
    }

    #[test]
    fn pre_recovery_completion_never_overwrites_a_conflicting_history_entry() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        fs::create_dir_all(&root).unwrap();
        prepare_pre_recovery(&root).unwrap();
        let journal = root.join(".recovery/migration/pre-recovery.json");
        let name: String = serde_json::from_slice(&fs::read(&journal).unwrap()).unwrap();
        let history = root.join(".recovery/migration/pre-recovery-history");
        fs::create_dir_all(&history).unwrap();
        let conflict = history.join(format!("{name}.json"));
        fs::write(&conflict, b"concurrent owner bytes").unwrap();
        assert!(finish_pre_recovery(&root).is_err());
        assert_eq!(fs::read(conflict).unwrap(), b"concurrent owner bytes");
        assert!(journal.is_file());
    }

    #[test]
    fn completed_migration_takes_new_pre_recovery_generation_and_resumes_after_exit() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        drop(Application::open(&root).unwrap());
        let history = root.join(".recovery/migration/pre-recovery-history");
        let first = fs::read_dir(&history)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(first.len(), 1);
        let transaction = root.join(".staging/transactions/second");
        fs::create_dir_all(transaction.join("files")).unwrap();
        fs::create_dir_all(root.join("owner-edits")).unwrap();
        fs::write(root.join("owner-edits/owner.txt"), b"second before").unwrap();
        fs::write(transaction.join("files/0000"), b"second after").unwrap();
        let manifest = serde_json::json!({"entries": [{"destination":"owner-edits/owner.txt", "staged":"files/0000", "expected": b"second before".to_vec()}]});
        fs::write(
            transaction.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::tests::abrupt_pre_recovery_snapshot_child",
            ])
            .env("KG_PRE_RECOVERY_TEST_ROOT", &root)
            .env("KG_STOP_AFTER_PRE_RECOVERY_BACKUP", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(77));
        assert_eq!(
            fs::read(root.join("owner-edits/owner.txt")).unwrap(),
            b"second before"
        );
        let active: String = serde_json::from_slice(
            &fs::read(root.join(".recovery/migration/pre-recovery.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            fs::read(
                root.join(".recovery/migration")
                    .join(&active)
                    .join("files/owner-edits/owner.txt")
            )
            .unwrap(),
            b"second before"
        );
        assert!(root
            .join(".recovery/migration")
            .join(&active)
            .join("files/.staging/transactions/second/manifest.json")
            .is_file());
        drop(Application::open(&root).unwrap());
        assert_eq!(
            fs::read(root.join("owner-edits/owner.txt")).unwrap(),
            b"second after"
        );
        assert!(!root.join(".recovery/migration/pre-recovery.json").exists());
        let history_entries = fs::read_dir(&history)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(history_entries.len(), 2);
        assert!(history_entries.contains(&first[0]));
        assert!(history.join(format!("{active}.json")).is_file());
    }

    #[test]
    fn damaged_pre_recovery_snapshot_blocks_journal_replay() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let transaction = root.join(".staging/transactions/pending");
        fs::create_dir_all(transaction.join("files")).unwrap();
        fs::create_dir_all(root.join("owner-edits")).unwrap();
        fs::write(root.join("owner-edits/owner.txt"), b"before replay").unwrap();
        fs::write(transaction.join("files/0000"), b"after replay").unwrap();
        let manifest = serde_json::json!({"entries": [{"destination":"owner-edits/owner.txt", "staged":"files/0000", "expected": b"before replay".to_vec()}]});
        fs::write(
            transaction.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        prepare_pre_recovery(&root).unwrap();
        let name: String = serde_json::from_slice(
            &fs::read(root.join(".recovery/migration/pre-recovery.json")).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join(".recovery/migration")
                .join(name)
                .join("files/owner-edits/owner.txt"),
            b"damaged backup",
        )
        .unwrap();
        assert!(Application::open(&root).is_err());
        assert_eq!(
            fs::read(root.join("owner-edits/owner.txt")).unwrap(),
            b"before replay"
        );
        assert!(transaction.join("manifest.json").is_file());
    }

    #[test]
    fn pre_recovery_and_migration_snapshots_restore_both_sides_of_replay() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        fs::create_dir_all(root.join("pages")).unwrap();
        let transaction = root.join(".staging/transactions/pending");
        fs::create_dir_all(&transaction).unwrap();
        fs::write(root.join("pages/owner.md"), b"owner before replay").unwrap();
        fs::write(
            transaction.join("manifest.json"),
            b"pending transaction bytes",
        )
        .unwrap();
        prepare_pre_recovery(&root).unwrap();
        let pre: String = serde_json::from_slice(
            &fs::read(root.join(".recovery/migration/pre-recovery.json")).unwrap(),
        )
        .unwrap();
        let pre_backup = root.join(".recovery/migration").join(&pre);
        assert_eq!(
            fs::read(pre_backup.join("files/.staging/transactions/pending/manifest.json")).unwrap(),
            b"pending transaction bytes"
        );
        // Simulate interruption after replay has begun; resuming must keep the
        // original pre-recovery backup, not replace it with the new state.
        fs::write(root.join("pages/owner.md"), b"owner after replay").unwrap();
        fs::remove_dir_all(&transaction).unwrap();
        prepare_pre_recovery(&root).unwrap();
        let repeated: String = serde_json::from_slice(
            &fs::read(root.join(".recovery/migration/pre-recovery.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(pre, repeated);
        prepare(&root).unwrap();
        let state: MigrationState =
            serde_json::from_slice(&fs::read(root.join(".recovery/migration/state.json")).unwrap())
                .unwrap();
        let post_backup = root.join(".recovery/migration").join(state.backup);
        let before_restore = workspace.path().join("restore-before");
        let after_restore = workspace.path().join("restore-after");
        restore_to_new_directory(&pre_backup, &before_restore).unwrap();
        restore_to_new_directory(&post_backup, &after_restore).unwrap();
        assert_eq!(
            fs::read(before_restore.join("pages/owner.md")).unwrap(),
            b"owner before replay"
        );
        assert_eq!(
            fs::read(before_restore.join(".staging/transactions/pending/manifest.json")).unwrap(),
            b"pending transaction bytes"
        );
        assert_eq!(
            fs::read(after_restore.join("pages/owner.md")).unwrap(),
            b"owner after replay"
        );
    }

    #[test]
    fn version_two_backup_restores_and_detects_corruption() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("nested/owner"), vec![9; 150_000]).unwrap();
        let saved = backup(&root).unwrap();
        let files = saved.join("files");
        let entries = inventory(&files)
            .unwrap()
            .into_iter()
            .map(|path| {
                let digest = if files.join(&path).is_file() {
                    Some(manifest_cursor::digest_file(&files, &path).unwrap())
                } else {
                    None
                };
                (path, digest)
            })
            .collect();
        fs::write(
            saved.join("manifest.json"),
            serde_json::to_vec(&ManifestV2 {
                version: 2,
                entries,
            })
            .unwrap(),
        )
        .unwrap();
        let restored = workspace.path().join("restored");
        restore_to_new_directory(&saved, &restored).unwrap();
        assert_eq!(
            fs::read(restored.join("nested/owner")).unwrap(),
            vec![9; 150_000]
        );
        fs::write(files.join("nested/owner"), vec![8; 150_000]).unwrap();
        assert!(restore_to_new_directory(&saved, &workspace.path().join("rejected")).is_err());
    }

    #[test]
    fn backup_refuses_symlinks_instead_of_following_or_discarding_them() {
        let workspace = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("outside", workspace.path().join("link")).unwrap();
        assert!(backup(workspace.path()).is_err());
        assert!(workspace
            .path()
            .join("link")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());
    }
}
