//! Durable coordination of the pre-replay and post-replay snapshots. This is
//! separate from the application mutex so a caller can schedule bounded turns.
use super::*;
#[derive(Serialize, Deserialize)]
struct State {
    phase: String,
    pre_work: PathBuf,
    post_work: PathBuf,
    pre_backup: Option<String>,
    #[serde(default)]
    reused: bool,
}
enum Verification {
    Legacy {
        path: PathBuf,
        session: legacy_verify::Session,
    },
    V3 {
        path: PathBuf,
        work: tempfile::TempDir,
        session: incremental_verify::Session,
    },
}
pub(crate) struct Session {
    snapshot: snapshot_worker::Session,
    verification: Option<Verification>,
    replay: crate::application::publication::RecoverySession,
    replay_initialized: bool,
    replay_failure: Option<String>,
    prune: Option<archive_prune::Session>,
    prune_initialized: bool,
    live_comparison: Option<(tempfile::TempDir, incremental_verify::Session)>,
    verified_index: Option<(PathBuf, std::sync::Arc<verified_index::VerifiedIndex>)>,
}
impl Session {
    pub(crate) fn new() -> Self {
        Self {
            snapshot: snapshot_worker::Session::new(),
            verification: None,
            replay: crate::application::publication::RecoverySession::new(),
            replay_initialized: false,
            replay_failure: None,
            prune: None,
            prune_initialized: false,
            live_comparison: None,
            verified_index: None,
        }
    }
}
fn verified(saved: &Path, session: &mut Session) -> Result<bool> {
    if session.verification.is_none() {
        let verification = if saved.join("manifest.sqlite").exists() {
            let work = tempfile::tempdir()?;
            incremental_verify::begin(
                &saved.join("files"),
                &saved.join("manifest.sqlite"),
                work.path(),
            )?;
            let mut cursor = incremental_verify::Session::new();
            cursor.capture()?;
            Verification::V3 {
                path: saved.to_owned(),
                work,
                session: cursor,
            }
        } else {
            let mut cursor = legacy_verify::Session::new(saved)?;
            cursor.capture()?;
            Verification::Legacy {
                path: saved.to_owned(),
                session: cursor,
            }
        };
        session.verification = Some(verification);
    }
    let complete = match session.verification.as_mut().unwrap() {
        Verification::Legacy { path, session } if path == saved => {
            legacy_verify::step(saved, session)?
        }
        Verification::V3 {
            path,
            work,
            session,
        } if path == saved => incremental_verify::step(
            &saved.join("files"),
            &saved.join("manifest.sqlite"),
            work.path(),
            session,
        )?,
        _ => {
            return Err(GardenError::Recovery(
                "Verification target changed during startup.".into(),
            ))
        }
    };
    if complete {
        let (path, index) = match session.verification.as_mut().unwrap() {
            Verification::V3 {
                path,
                session: cursor,
                ..
            } => (path.clone(), cursor.take_index()),
            Verification::Legacy {
                path,
                session: cursor,
            } => (path.clone(), cursor.take_index()),
        };
        let index = index
            .ok_or_else(|| GardenError::Recovery("Verified manifest index is missing.".into()))?;
        session.verified_index = Some((path, std::sync::Arc::new(index)));
        session.verification = None;
    }
    Ok(complete)
}
const JOURNAL: &str = ".recovery/migration/startup-coordinator.json";
fn load(root: &Path) -> Result<State> {
    let mut bytes = Vec::new();
    bounded_copy::source_file(root, Path::new(JOURNAL))?
        .take(16385)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 16384 {
        return Err(GardenError::Recovery(
            "Startup coordinator is too large.".into(),
        ));
    }
    let state: State = serde_json::from_slice(&bytes)?;
    for (path, prefix) in [
        (&state.pre_work, "pre-work-"),
        (&state.post_work, "post-work-"),
    ] {
        validate_relative_collection_path(path)?;
        if path.parent() != Some(Path::new(".recovery/migration"))
            || !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix) && name.len() <= 128)
        {
            return Err(GardenError::Recovery(
                "Startup work escaped recovery.".into(),
            ));
        }
    }
    if let Some(id) = &state.pre_backup {
        if !id.starts_with("backup-") || Path::new(id).components().count() != 1 {
            return Err(GardenError::Recovery(
                "Pre-recovery identity is invalid.".into(),
            ));
        }
    }
    Ok(state)
}
fn save(root: &Path, state: &State) -> Result<()> {
    write_atomic(&root.join(JOURNAL), &serde_json::to_vec(state)?)
}
fn create_once(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap();
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(path)
        .map_err(|e| GardenError::Io(e.error))?;
    sync_directory(parent)
}
fn archive_interrupted_reuse(root: &Path, generation: &str) -> Result<()> {
    use rustix::fs::{openat, renameat_with, Mode, OFlags, RenameFlags, CWD};
    if !generation.starts_with("pre-work-")
        || Path::new(generation).components().count() != 1
        || generation.len() > 128
    {
        return Err(GardenError::Recovery(
            "Recovery generation is invalid.".into(),
        ));
    }
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let root_fd = openat(CWD, root, flags, Mode::empty()).map_err(std::io::Error::from)?;
    let recovery =
        openat(&root_fd, ".recovery", flags, Mode::empty()).map_err(std::io::Error::from)?;
    let migration =
        openat(&recovery, "migration", flags, Mode::empty()).map_err(std::io::Error::from)?;
    let target = format!("interrupted-reuse-{generation}.json");
    renameat_with(
        &migration,
        "pre-recovery.json",
        &migration,
        target.as_str(),
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    File::from(migration).sync_all()?;
    Ok(())
}
pub(crate) fn begin(root: &Path) -> Result<()> {
    let path = root.join(JOURNAL);
    validate_real_collection_ancestors(root, &path)?;
    fs::create_dir_all(path.parent().unwrap())?;
    let mut candidate = None;
    if fs::symlink_metadata(&path).is_ok() {
        let mut existing = load(root)?;
        let active =
            fs::symlink_metadata(root.join(".recovery/migration/pre-recovery.json")).is_ok();
        if existing.reused && (existing.phase != "complete" || active) {
            let generation = existing
                .pre_work
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| GardenError::Recovery("Recovery generation is invalid.".into()))?;
            let archived = root
                .join(".recovery/migration")
                .join(format!("interrupted-reuse-{generation}.json"));
            if active {
                archive_interrupted_reuse(root, generation)?;
            }
            if active || fs::symlink_metadata(&archived).is_ok() {
                let fresh = Uuid::new_v4().to_string();
                existing.pre_work = PathBuf::from(format!(".recovery/migration/pre-work-{fresh}"));
                existing.post_work =
                    PathBuf::from(format!(".recovery/migration/post-work-{fresh}"));
                existing.pre_backup = None;
                existing.phase = "pre".into();
                existing.reused = false;
            } else if [
                "reuse_verify",
                "reuse_compare",
                "reuse_verify_again",
                "reuse_compare_final",
            ]
            .contains(&existing.phase.as_str())
            {
                existing.phase = "reuse_verify".into();
            } else {
                return Err(GardenError::Recovery(
                    "Reused recovery state has no active journal.".into(),
                ));
            }
            save(root, &existing)?;
            return Ok(());
        }
        if existing.phase != "complete" || active {
            if active && existing.phase != "pre" {
                existing.phase = "verify_pre".into();
                save(root, &existing)?;
            }
            return Ok(());
        }
        let name = existing.pre_backup.ok_or_else(|| {
            GardenError::Recovery("Completed coordinator has no pre-recovery identity.".into())
        })?;
        let history = root
            .join(".recovery/migration/pre-recovery-history")
            .join(format!("{name}.json"));
        let relative = history.strip_prefix(root).map_err(|_| {
            GardenError::Recovery("Recovery history escaped the collection.".into())
        })?;
        let completed: String =
            serde_json::from_slice(&read_bounded_nofollow(root, relative, 4096)?)?;
        if completed != name {
            return Err(GardenError::Recovery(
                "Completed recovery history is ambiguous.".into(),
            ));
        }
        match fs::symlink_metadata(
            root.join(".recovery/migration")
                .join(&name)
                .join("manifest.sqlite"),
        ) {
            Ok(metadata) if metadata.file_type().is_file() => candidate = Some(name),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Completed recovery manifest is not a regular file.".into(),
                ))
            }
        }
    }
    let generation = Uuid::new_v4().to_string();
    let pre = root.join(".recovery/migration/pre-recovery.json");
    let (phase, pre_backup) = match fs::symlink_metadata(&pre) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if candidate.is_some() {
                ("reuse_verify", candidate)
            } else {
                ("pre", None)
            }
        }
        Err(e) => return Err(e.into()),
        Ok(m) if m.is_file() => {
            let id: String = serde_json::from_str(&read_bounded_text(&pre, 4096)?)?;
            if !id.starts_with("backup-") || Path::new(&id).components().count() != 1 {
                return Err(GardenError::Recovery(
                    "Pre-recovery identity is invalid.".into(),
                ));
            }
            let history = root
                .join(".recovery/migration/pre-recovery-history")
                .join(format!("{id}.json"));
            if fs::symlink_metadata(history).is_ok() {
                return Err(GardenError::Recovery(
                    "Active recovery conflicts with completion history.".into(),
                ));
            }
            ("verify_pre", Some(id))
        }
        Ok(_) => {
            return Err(GardenError::Recovery(
                "Pre-recovery journal is not a regular file.".into(),
            ))
        }
    };
    let state = State {
        phase: phase.into(),
        pre_work: PathBuf::from(format!(".recovery/migration/pre-work-{generation}")),
        post_work: PathBuf::from(format!(".recovery/migration/post-work-{generation}")),
        pre_backup,
        reused: phase == "reuse_verify",
    };
    if fs::symlink_metadata(&path).is_ok() {
        save(root, &state)
    } else {
        create_once(&path, &serde_json::to_vec(&state)?)
    }
}
pub(crate) fn description(root: &Path, session: &Session) -> Result<String> {
    let state = load(root)?;
    let message = match state.phase.as_str() {
        "pre" => snapshot_worker::progress(root, &state.pre_work)?
            .unwrap_or_else(|| "Creating and verifying a backup before recovery.".into()),
        "verify_pre" => "Verifying the existing recovery backup.".into(),
        "reuse_verify" | "reuse_verify_again" => "Checking the retained recovery backup.".into(),
        "reuse_compare" | "reuse_compare_final" => {
            "Checking the library for changes since the retained backup.".into()
        }
        "replay" => session.replay.progress(),
        "prune" => "Checking retained publication records.".into(),
        "post" => snapshot_worker::progress(root, &state.post_work)?
            .unwrap_or_else(|| "Creating and verifying a backup after recovery.".into()),
        "verify_post" => "Verifying the existing migration backup.".into(),
        "complete" => "Preparing library indexes and recovery records.".into(),
        _ => {
            return Err(GardenError::Recovery(
                "Startup coordinator phase is invalid.".into(),
            ))
        }
    };
    Ok(message)
}
fn snapshot(
    root: &Path,
    work: &Path,
    session: &mut snapshot_worker::Session,
) -> Result<Option<PathBuf>> {
    match fs::symlink_metadata(root.join(work).join("state.json")) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            snapshot_worker::begin(root, work)?;
            Ok(None)
        }
        Err(e) => Err(e.into()),
        Ok(m) if m.is_file() => snapshot_worker::step(root, work, session),
        Ok(_) => Err(GardenError::Recovery(
            "Snapshot state is not a regular file.".into(),
        )),
    }
}
fn name(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|v| v.to_str())
        .filter(|v| v.starts_with("backup-"))
        .map(str::to_owned)
        .ok_or_else(|| GardenError::Recovery("Snapshot identity is invalid.".into()))
}
/// `true` means the pre-replay snapshot and the migration snapshot have been
/// durably published. The application cutover still has to run afterward.
pub(crate) fn step(root: &Path, session: &mut Session) -> Result<bool> {
    let mut state = load(root)?;
    if ![
        "pre",
        "reuse_verify",
        "reuse_compare",
        "reuse_verify_again",
        "reuse_compare_final",
    ]
    .contains(&state.phase.as_str())
    {
        let expected = state
            .pre_backup
            .as_deref()
            .ok_or_else(|| GardenError::Recovery("Pre-recovery identity is missing.".into()))?;
        let journal = root.join(".recovery/migration/pre-recovery.json");
        validate_real_collection_ancestors(root, &journal)?;
        if !fs::symlink_metadata(&journal)?.is_file() {
            return Err(GardenError::Recovery(
                "Pre-recovery journal changed type.".into(),
            ));
        }
        let actual: String = serde_json::from_str(&read_bounded_text(&journal, 4096)?)?;
        if actual != expected {
            return Err(GardenError::Recovery(
                "Pre-recovery journal changed during startup.".into(),
            ));
        }
    }
    match state.phase.as_str() {
        "reuse_verify" => {
            let id = state.pre_backup.as_deref().ok_or_else(|| {
                GardenError::Recovery("Retained backup identity is missing.".into())
            })?;
            if verified(&root.join(".recovery/migration").join(id), session)? {
                state.phase = "reuse_compare".into();
                save(root, &state)?;
            }
            Ok(false)
        }
        "reuse_compare" | "reuse_compare_final" => {
            let id = state.pre_backup.as_deref().ok_or_else(|| {
                GardenError::Recovery("Retained backup identity is missing.".into())
            })?;
            let db = root
                .join(".recovery/migration")
                .join(id)
                .join("manifest.sqlite");
            if session.live_comparison.is_none() {
                let work = tempfile::tempdir()?;
                incremental_verify::begin(root, &db, work.path())?;
                session.live_comparison = Some((work, incremental_verify::Session::new()));
                return Ok(false);
            }
            let (work, cursor) = session.live_comparison.as_mut().unwrap();
            match incremental_verify::step(root, &db, work.path(), cursor) {
                Ok(false) => Ok(false),
                Ok(true) => {
                    session.live_comparison = None;
                    if state.phase == "reuse_compare_final" {
                        create_once(
                            &root.join(".recovery/migration/pre-recovery.json"),
                            &serde_json::to_vec(id)?,
                        )?;
                        #[cfg(test)]
                        if std::env::var_os("KG_REUSE_STOP_AFTER_JOURNAL").is_some() {
                            std::process::exit(79);
                        }
                        state.phase = "replay".into();
                    } else {
                        state.phase = "reuse_verify_again".into();
                    }
                    save(root, &state)?;
                    Ok(false)
                }
                Err(_) => {
                    session.live_comparison = None;
                    session.snapshot = snapshot_worker::Session::new();
                    state.pre_backup = None;
                    state.phase = "pre".into();
                    state.reused = false;
                    save(root, &state)?;
                    Ok(false)
                }
            }
        }
        "reuse_verify_again" => {
            let id = state.pre_backup.as_deref().ok_or_else(|| {
                GardenError::Recovery("Retained backup identity is missing.".into())
            })?;
            if !verified(&root.join(".recovery/migration").join(id), session)? {
                return Ok(false);
            }
            state.phase = "reuse_compare_final".into();
            save(root, &state)?;
            Ok(false)
        }
        "pre" => {
            let Some(saved) = snapshot(root, &state.pre_work, &mut session.snapshot)? else {
                return Ok(false);
            };
            if !verified(&saved, session)? {
                return Ok(false);
            }
            let id = name(&saved)?;
            let path = root.join(".recovery/migration/pre-recovery.json");
            match fs::symlink_metadata(&path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    create_once(&path, &serde_json::to_vec(&id)?)?
                }
                Err(e) => return Err(e.into()),
                Ok(m) if m.is_file() => {
                    let existing: String = serde_json::from_str(&read_bounded_text(&path, 4096)?)?;
                    if existing != id {
                        return Err(GardenError::Recovery(
                            "Pre-recovery journal conflicts with the verified snapshot.".into(),
                        ));
                    }
                }
                Ok(_) => {
                    return Err(GardenError::Recovery(
                        "Pre-recovery journal is not a regular file.".into(),
                    ))
                }
            }
            #[cfg(test)]
            if std::env::var_os("KG_COORDINATOR_STOP_AFTER_PRE_JOURNAL").is_some() {
                std::process::exit(90);
            }
            state.pre_backup = Some(id);
            state.phase = "replay".into();
            save(root, &state)?;
            session.snapshot = snapshot_worker::Session::new();
            Ok(false)
        }
        "verify_pre" => {
            let id = state
                .pre_backup
                .as_deref()
                .ok_or_else(|| GardenError::Recovery("Pre-recovery identity is missing.".into()))?;
            if verified(&root.join(".recovery/migration").join(id), session)? {
                state.phase = "replay".into();
                save(root, &state)?;
            }
            Ok(false)
        }
        "verify_post" => {
            let path = root.join(".recovery/migration/state.json");
            let existing: MigrationState = serde_json::from_str(&read_bounded_text(&path, 4096)?)?;
            if ![
                "backup_verified",
                "evidence_imported",
                "cutover_preparing",
                "authority_retired",
            ]
            .contains(&existing.phase.as_str())
                || !existing.backup.starts_with("backup-")
                || Path::new(&existing.backup).components().count() != 1
            {
                return Err(GardenError::Recovery(
                    "Existing migration state is invalid.".into(),
                ));
            }
            if verified(
                &root.join(".recovery/migration").join(&existing.backup),
                session,
            )? {
                state.phase = "complete".into();
                save(root, &state)?;
                return Ok(true);
            }
            Ok(false)
        }
        "replay" => {
            if let Some(detail) = &session.replay_failure {
                return Err(GardenError::Recovery(detail.clone()));
            }
            fs::create_dir_all(root.join("sources"))?;
            fs::create_dir_all(root.join(".staging"))?;
            if !session.replay_initialized {
                let generation = state
                    .pre_work
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| GardenError::Recovery("Replay generation is invalid.".into()))?;
                session.replay =
                    crate::application::publication::RecoverySession::for_generation(generation)?;
                let (path, index) = session.verified_index.as_ref().ok_or_else(|| {
                    GardenError::Recovery("Replay has no held verified manifest index.".into())
                })?;
                let expected = root
                    .join(".recovery/migration")
                    .join(state.pre_backup.as_ref().unwrap());
                if path != &expected {
                    return Err(GardenError::Recovery(
                        "Verified manifest belongs to another backup.".into(),
                    ));
                }
                session.replay.set_verified_index(index.clone());
                session.replay_initialized = true;
            }
            match session.replay.step(root) {
                Ok(false) => return Ok(false),
                Ok(true) => (),
                Err(error) => {
                    session.replay_failure = Some(error.to_string());
                    return Err(error);
                }
            }
            state.phase = "prune".into();
            save(root, &state)?;
            Ok(false)
        }
        "prune" => {
            if !session.prune_initialized {
                let id = state.pre_backup.as_deref().ok_or_else(|| {
                    GardenError::Recovery("Pruning backup identity is missing.".into())
                })?;
                let generation = state
                    .pre_work
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| {
                        GardenError::Recovery("Pruning generation is invalid.".into())
                    })?;
                session.prune = archive_prune::Session::new(root, id, generation)?;
                session.prune_initialized = true;
            }
            if let Some(prune) = session.prune.as_mut() {
                if !prune.step()? {
                    return Ok(false);
                }
            }
            state.phase = "post".into();
            save(root, &state)?;
            Ok(false)
        }
        "post" => {
            let path = root.join(".recovery/migration/state.json");
            if fs::symlink_metadata(&path).is_ok() {
                state.phase = "verify_post".into();
                save(root, &state)?;
                return Ok(false);
            }
            let Some(saved) = snapshot(root, &state.post_work, &mut session.snapshot)? else {
                return Ok(false);
            };
            if !verified(&saved, session)? {
                return Ok(false);
            }
            let migration = MigrationState {
                backup: name(&saved)?,
                phase: "backup_verified".into(),
            };
            match create_once(&path, &serde_json::to_vec(&migration)?) {
                Ok(()) => (),
                Err(GardenError::Io(ref e)) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let existing: MigrationState =
                        serde_json::from_str(&read_bounded_text(&path, 4096)?)?;
                    if existing.backup != migration.backup || existing.phase != migration.phase {
                        return Err(GardenError::Recovery(
                            "Migration state conflicts with the verified snapshot.".into(),
                        ));
                    }
                }
                Err(e) => return Err(e),
            }
            #[cfg(test)]
            if std::env::var_os("KG_COORDINATOR_STOP_AFTER_POST_JOURNAL").is_some() {
                std::process::exit(91);
            }
            state.phase = "complete".into();
            save(root, &state)?;
            Ok(true)
        }
        "complete" => Ok(true),
        _ => Err(GardenError::Recovery(
            "Startup coordinator phase is invalid.".into(),
        )),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coordinator_rejects_a_nested_forged_work_generation() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        begin(&root).unwrap();
        let path = root.join(".recovery/migration/startup-coordinator.json");
        let mut state: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        state["pre_work"] = serde_json::json!(".recovery/migration/other/pre-work-forged");
        fs::write(path, serde_json::to_vec(&state).unwrap()).unwrap();
        assert!(begin(&root).is_err());
    }
    fn complete_coordinator(root: &Path) -> String {
        begin(root).unwrap();
        let mut session = Session::new();
        let mut complete = false;
        for _ in 0..30000 {
            if step(root, &mut session).unwrap() {
                complete = true;
                break;
            }
        }
        assert!(complete);
        serde_json::from_slice(
            &fs::read(root.join(".recovery/migration/pre-recovery.json")).unwrap(),
        )
        .unwrap()
    }
    fn stable_reuse_fixture(root: &Path) -> String {
        fs::create_dir_all(root).unwrap();
        fs::write(root.join("owner"), b"original").unwrap();
        complete_coordinator(root);
        finish_pre_recovery(root).unwrap();
        let stable = complete_coordinator(root);
        finish_pre_recovery(root).unwrap();
        stable
    }
    #[test]
    fn unchanged_collection_reuses_verified_backup_and_external_change_gets_a_new_one() {
        for changed in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("collection");
            fs::create_dir(&root).unwrap();
            let root = fs::canonicalize(root).unwrap();
            let stable = stable_reuse_fixture(&root);
            if changed {
                fs::write(root.join("owner"), b"external owner edit").unwrap();
            }
            let current = complete_coordinator(&root);
            if changed {
                assert_ne!(current, stable);
                assert_eq!(
                    fs::read(
                        root.join(".recovery/migration")
                            .join(&current)
                            .join("files/owner")
                    )
                    .unwrap(),
                    b"external owner edit"
                );
            } else {
                assert_eq!(current, stable);
                let state: MigrationState = serde_json::from_slice(
                    &fs::read(root.join(".recovery/migration/state.json")).unwrap(),
                )
                .unwrap();
                fs::write(
                    root.join(".recovery/migration/state.json"),
                    serde_json::to_vec(&MigrationState {
                        backup: state.backup,
                        phase: "authority_retired".into(),
                    })
                    .unwrap(),
                )
                .unwrap();
                let mut verify = final_verify::Session::new(&root, false).unwrap();
                let proof = loop {
                    if let Some(proof) = verify.step().unwrap() {
                        break proof;
                    }
                };
                finish_pre_recovery_verified(&root, &proof).unwrap();
                assert!(!root.join(".recovery/migration/pre-recovery.json").exists());
            }
        }
    }
    #[test]
    fn interrupted_reuse_takes_a_fresh_snapshot_before_replay() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let stable = stable_reuse_fixture(&root);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::startup_coordinator::tests::crash_child",
            ])
            .env("KG_COORDINATOR_ROOT", &root)
            .env("KG_REUSE_STOP_AFTER_JOURNAL", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(79));
        fs::write(root.join("owner"), b"external after exit").unwrap();
        let fresh = complete_coordinator(&root);
        assert_ne!(fresh, stable);
        assert_eq!(
            fs::read(
                root.join(".recovery/migration")
                    .join(&fresh)
                    .join("files/owner")
            )
            .unwrap(),
            b"external after exit"
        );
        assert_eq!(
            fs::read(
                root.join(".recovery/migration")
                    .join(&stable)
                    .join("files/owner")
            )
            .unwrap(),
            b"original"
        );
    }
    #[test]
    fn reuse_rechecks_a_same_metadata_edit_after_the_first_comparison() {
        use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let stable = stable_reuse_fixture(&root);
        begin(&root).unwrap();
        let mut session = Session::new();
        for _ in 0..30000 {
            if load(&root).unwrap().phase == "reuse_compare_final" {
                break;
            }
            assert!(!step(&root, &mut session).unwrap());
        }
        assert_eq!(load(&root).unwrap().phase, "reuse_compare_final");
        let path = root.join("owner");
        let before = fs::metadata(&path).unwrap();
        fs::write(&path, b"altered!").unwrap();
        let name = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
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
        let mut complete = false;
        for _ in 0..30000 {
            if step(&root, &mut session).unwrap() {
                complete = true;
                break;
            }
        }
        assert!(complete);
        let fresh: String = serde_json::from_slice(
            &fs::read(root.join(".recovery/migration/pre-recovery.json")).unwrap(),
        )
        .unwrap();
        assert_ne!(fresh, stable);
        assert_eq!(
            fs::read(
                root.join(".recovery/migration")
                    .join(fresh)
                    .join("files/owner")
            )
            .unwrap(),
            b"altered!"
        );
    }
    #[test]
    fn staged_mutation_after_process_exit_blocks_replay() {
        use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        let transaction = root.join(".staging/transactions/pending");
        fs::create_dir_all(transaction.join("files")).unwrap();
        fs::create_dir_all(root.join("notes")).unwrap();
        let staged = transaction.join("files/0000");
        fs::write(&staged, b"planned").unwrap();
        fs::write(transaction.join("manifest.json"), br#"{"entries":[{"destination":"notes/page","staged":"files/0000","expected":null}],"removals":[]}"#).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::startup_coordinator::tests::crash_child",
            ])
            .env("KG_COORDINATOR_ROOT", &root)
            .env("KG_REPLAY_STOP_AFTER_STAGE_VERIFY", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(82));
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
        begin(&root).unwrap();
        let mut session = Session::new();
        let mut rejected = false;
        for _ in 0..10000 {
            match step(&root, &mut session) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(true) => break,
                Ok(false) => (),
            }
        }
        assert!(rejected);
        assert!(!root.join("notes/page").exists());
        assert_eq!(fs::read(staged).unwrap(), b"mangled");
    }
    #[test]
    fn replay_rejects_manifest_mutation_with_restored_metadata() {
        use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        let transaction = root.join(".staging/transactions/pending");
        fs::create_dir_all(transaction.join("files")).unwrap();
        fs::create_dir_all(root.join("notes")).unwrap();
        fs::write(transaction.join("files/0000"), b"planned").unwrap();
        let manifest = transaction.join("manifest.json");
        let bytes = br#"{"entries":[{"destination":"notes/a","staged":"files/0000","expected":null}],"removals":[]}"#;
        fs::write(&manifest, bytes).unwrap();
        let root = fs::canonicalize(root).unwrap();
        begin(&root).unwrap();
        let mut session = Session::new();
        for _ in 0..10000 {
            if load(&root).unwrap().phase == "replay" {
                break;
            }
            assert!(!step(&root, &mut session).unwrap());
        }
        assert_eq!(load(&root).unwrap().phase, "replay");
        let before = fs::metadata(&manifest).unwrap();
        let changed = String::from_utf8(bytes.to_vec())
            .unwrap()
            .replace("notes/a", "notes/b");
        assert_eq!(changed.len(), bytes.len());
        fs::write(&manifest, changed.as_bytes()).unwrap();
        let name = std::ffi::CString::new(manifest.as_os_str().as_bytes()).unwrap();
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
        let after = fs::metadata(&manifest).unwrap();
        assert_eq!(
            (after.ino(), after.len(), after.mtime(), after.mtime_nsec()),
            (
                before.ino(),
                before.len(),
                before.mtime(),
                before.mtime_nsec()
            )
        );
        let mut rejected = false;
        for _ in 0..10000 {
            match step(&root, &mut session) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(true) => break,
                Ok(false) => (),
            }
        }
        assert!(rejected);
        let scratch = root.join(".recovery/migration/replay-scratch");
        let retained = fs::read_dir(&scratch).unwrap().count();
        for _ in 0..5 {
            assert!(step(&root, &mut session).is_err());
        }
        assert_eq!(fs::read_dir(&scratch).unwrap().count(), retained);
        assert!(!root.join("notes/a").exists());
        assert!(!root.join("notes/b").exists());
        assert_eq!(fs::read(&manifest).unwrap(), changed.as_bytes());
    }
    #[test]
    fn large_manifest_replay_resumes_inside_a_transaction() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        let transaction = root.join(".staging/transactions/pending");
        fs::create_dir_all(transaction.join("files")).unwrap();
        fs::create_dir_all(root.join("notes")).unwrap();
        let mut entries = Vec::new();
        for index in 0..32 {
            let staged = format!("files/{index:04}");
            fs::write(transaction.join(&staged), format!("value-{index}")).unwrap();
            entries.push(serde_json::json!({"destination":format!("notes/{index}"),"staged":staged,"expected":vec![7u8;4096]}));
        }
        fs::write(
            transaction.join("manifest.json"),
            serde_json::to_vec(&serde_json::json!({"entries":entries,"removals":[]})).unwrap(),
        )
        .unwrap();
        let root = fs::canonicalize(root).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::startup_coordinator::tests::crash_child",
            ])
            .env("KG_COORDINATOR_ROOT", &root)
            .env("KG_REPLAY_STOP_AFTER_ENTRY", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(84));
        assert_eq!(fs::read_dir(root.join("notes")).unwrap().count(), 1);
        begin(&root).unwrap();
        let mut session = Session::new();
        let mut complete = false;
        for _ in 0..30000 {
            if step(&root, &mut session).unwrap() {
                complete = true;
                break;
            }
        }
        assert!(complete);
        for index in 0..32 {
            assert_eq!(
                fs::read_to_string(root.join(format!("notes/{index}"))).unwrap(),
                format!("value-{index}")
            );
        }
    }
    #[test]
    fn adopts_verified_legacy_pre_recovery_and_migration_state() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        fs::write(root.join("owner"), b"owner").unwrap();
        fs::create_dir_all(root.join(".staging/transactions")).unwrap();
        let saved = backup(&root).unwrap();
        let id = name(&saved).unwrap();
        fs::write(
            root.join(".recovery/migration/pre-recovery.json"),
            serde_json::to_vec(&id).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join(".recovery/migration/state.json"),
            serde_json::to_vec(&MigrationState {
                backup: id.clone(),
                phase: "authority_retired".into(),
            })
            .unwrap(),
        )
        .unwrap();
        begin(&root).unwrap();
        let mut session = Session::new();
        let mut done = false;
        for _ in 0..500 {
            if step(&root, &mut session).unwrap() {
                done = true;
                break;
            }
        }
        assert!(done);
        let retained: MigrationState =
            serde_json::from_slice(&fs::read(root.join(".recovery/migration/state.json")).unwrap())
                .unwrap();
        assert_eq!(retained.backup, id);
        assert_eq!(retained.phase, "authority_retired");
    }
    #[test]
    fn crash_child() {
        let Some(root) = std::env::var_os("KG_COORDINATOR_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        begin(&root).unwrap();
        let mut session = Session::new();
        loop {
            if step(&root, &mut session).unwrap() {
                panic!("crash hook did not run");
            }
        }
    }
    #[test]
    fn modified_snapshot_after_crash_blocks_replay() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        fs::write(root.join("owner"), b"untouched").unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::startup_coordinator::tests::crash_child",
            ])
            .env("KG_COORDINATOR_ROOT", &root)
            .env("KG_COORDINATOR_STOP_AFTER_PRE_JOURNAL", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(90));
        let id: String = serde_json::from_slice(
            &fs::read(root.join(".recovery/migration/pre-recovery.json")).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join(".recovery/migration")
                .join(id)
                .join("files/owner"),
            b"damaged",
        )
        .unwrap();
        begin(&root).unwrap();
        let mut session = Session::new();
        let mut rejected = false;
        for _ in 0..500 {
            match step(&root, &mut session) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(true) => break,
                Ok(false) => (),
            }
        }
        assert!(rejected);
        assert_eq!(fs::read(root.join("owner")).unwrap(), b"untouched");
    }
    #[test]
    fn replay_uses_verified_rows_after_live_manifest_tampering() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        let transaction = root.join(".staging/transactions/pending");
        fs::create_dir_all(transaction.join("files")).unwrap();
        fs::write(transaction.join("files/0000"), b"original").unwrap();
        fs::write(transaction.join("manifest.json"), serde_json::to_vec(&serde_json::json!({"entries":[{"destination":"notes/value","staged":"files/0000","expected":null}],"removals":[]})).unwrap()).unwrap();
        let root = fs::canonicalize(root).unwrap();
        begin(&root).unwrap();
        let mut session = Session::new();
        let mut ready = false;
        for _ in 0..10000 {
            step(&root, &mut session).unwrap();
            if load(&root).unwrap().phase == "replay" {
                ready = true;
                break;
            }
        }
        assert!(ready);
        let id: String = serde_json::from_slice(
            &fs::read(root.join(".recovery/migration/pre-recovery.json")).unwrap(),
        )
        .unwrap();
        fs::write(transaction.join("files/0000"), b"tampered").unwrap();
        let digest = manifest_cursor::digest_file(
            &root,
            Path::new(".staging/transactions/pending/files/0000"),
        )
        .unwrap();
        manifest_records::set(
            &root
                .join(".recovery/migration")
                .join(id)
                .join("manifest.sqlite"),
            Path::new(".staging/transactions/pending/files/0000"),
            Some(&digest),
        )
        .unwrap();
        let mut rejected = false;
        for _ in 0..10000 {
            match step(&root, &mut session) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(true) => break,
                Ok(false) => (),
            }
        }
        assert!(rejected);
        assert!(!root.join("notes/value").exists());
    }
    #[test]
    fn modified_manifest_after_process_exit_blocks_replay() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        fs::write(root.join("owner"), b"untouched").unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::startup_coordinator::tests::crash_child",
            ])
            .env("KG_COORDINATOR_ROOT", &root)
            .env("KG_COORDINATOR_STOP_AFTER_PRE_JOURNAL", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(90));
        let id: String = serde_json::from_slice(
            &fs::read(root.join(".recovery/migration/pre-recovery.json")).unwrap(),
        )
        .unwrap();
        let db = root
            .join(".recovery/migration")
            .join(id)
            .join("manifest.sqlite");
        manifest_records::set(&db, Path::new("owner"), Some(&"f".repeat(64))).unwrap();
        begin(&root).unwrap();
        let mut session = Session::new();
        let mut rejected = false;
        for _ in 0..500 {
            match step(&root, &mut session) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(true) => break,
                Ok(false) => (),
            }
        }
        assert!(rejected);
        assert_eq!(fs::read(root.join("owner")).unwrap(), b"untouched");
    }
    #[test]
    fn pre_and_post_journal_crashes_resume_and_preserve_both_states() {
        for hook in [
            "KG_COORDINATOR_STOP_AFTER_PRE_JOURNAL",
            "KG_COORDINATOR_STOP_AFTER_POST_JOURNAL",
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("collection");
            let transaction = root.join(".staging/transactions/pending");
            fs::create_dir_all(transaction.join("files")).unwrap();
            fs::create_dir_all(root.join("owner-edits")).unwrap();
            fs::write(root.join("owner-edits/owner"), b"before").unwrap();
            fs::write(transaction.join("files/0000"), b"after").unwrap();
            fs::write(transaction.join("manifest.json"), serde_json::to_vec(&serde_json::json!({"entries":[{"destination":"owner-edits/owner","staged":"files/0000","expected":b"before".to_vec()}]})).unwrap()).unwrap();
            let root = fs::canonicalize(&root).unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "application::migration_backup::startup_coordinator::tests::crash_child",
                ])
                .env("KG_COORDINATOR_ROOT", &root)
                .env(hook, "1")
                .status()
                .unwrap();
            assert_eq!(
                status.code(),
                Some(if hook.ends_with("PRE_JOURNAL") {
                    90
                } else {
                    91
                })
            );
            begin(&root).unwrap();
            let mut session = Session::new();
            let mut complete = false;
            for _ in 0..2000 {
                if step(&root, &mut session).unwrap() {
                    complete = true;
                    break;
                }
            }
            assert!(complete);
            let pre: String = serde_json::from_slice(
                &fs::read(root.join(".recovery/migration/pre-recovery.json")).unwrap(),
            )
            .unwrap();
            let post: MigrationState = serde_json::from_slice(
                &fs::read(root.join(".recovery/migration/state.json")).unwrap(),
            )
            .unwrap();
            let preserved = root
                .join(".recovery/migration")
                .join(&pre)
                .join("files/.staging/transactions/pending/files/0000");
            assert_eq!(fs::read(preserved).unwrap(), b"after");
            let before = temp.path().join("before");
            let after = temp.path().join("after");
            restore_to_new_directory(&root.join(".recovery/migration").join(pre), &before).unwrap();
            restore_to_new_directory(&root.join(".recovery/migration").join(post.backup), &after)
                .unwrap();
            assert_eq!(
                fs::read(before.join("owner-edits/owner")).unwrap(),
                b"before"
            );
            assert_eq!(fs::read(after.join("owner-edits/owner")).unwrap(), b"after");
        }
    }
}
