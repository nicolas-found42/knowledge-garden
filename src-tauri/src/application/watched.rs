//! Bounded polling reconciliation. Absence is evidence only after complete scans.
use super::*;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchedDirectory {
    pub watch_id: String,
    pub path: PathBuf,
    pub state: String,
    pub detail: Option<String>,
    pub processed_files: u64,
    device: u64,
    inode: u64,
    #[serde(default)]
    volume_uuid: Option<String>,
    #[serde(default)]
    volume_retry_at: u64,
    generation: u64,
    completed_generation: u64,
    pending: Vec<PathBuf>,
    active: Option<PathBuf>,
    withdrawing: bool,
}

#[derive(Serialize, Deserialize)]
struct FileReceipt {
    relative_path: PathBuf,
    source_id: String,
    seen: u64,
    missing: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct DirectoryVolume {
    device: u64,
    volume_uuid: Option<String>,
    mount_boundary: bool,
}

impl Application {
    /// Registers a directory without enumerating it or waiting for semantic work.
    pub fn watch_directory(&mut self, path: impl AsRef<Path>) -> Result<WatchedDirectory> {
        self.recover_unregistering_watches()?;
        let path = fs::canonicalize(path)?;
        readable_directory(&path)?;
        if path.starts_with(&self.root) {
            return Err(GardenError::Invalid(
                "Choose a directory outside the collection.".into(),
            ));
        }
        let watch_id = hex::encode(Sha256::digest(path.as_os_str().as_encoded_bytes()));
        let destination = self.root.join("watches").join(&watch_id);
        fs::create_dir_all(destination.join("files"))?;
        fs::create_dir_all(destination.join("directories"))?;
        let metadata = fs::metadata(&path)?;
        let mut watch = if destination.join("index.md").exists() {
            read_record::<WatchedDirectory>(&destination.join("index.md"))?
        } else {
            WatchedDirectory {
                watch_id,
                path,
                state: "watching".into(),
                detail: None,
                processed_files: 0,
                device: metadata.dev(),
                inode: metadata.ino(),
                volume_uuid: None,
                volume_retry_at: 0,
                generation: 0,
                completed_generation: 0,
                pending: Vec::new(),
                active: None,
                withdrawing: false,
            }
        };
        // Explicit reselection accepts a changed filesystem identity.
        validate_watch(&watch, &destination)?;
        watch.device = metadata.dev();
        watch.inode = metadata.ino();
        watch.volume_uuid = volume_uuid(&watch.path);
        watch.state = "watching".into();
        self.watch_readers.remove(&watch.watch_id);
        start_scan(&mut watch);
        self.save_watch(&watch)?;
        Ok(watch)
    }

    pub fn unregister_watched_directory(&mut self, watch_id: &str) -> Result<()> {
        self.recover_source_removals()?;
        let mut watch = self
            .list_watched_directories()?
            .into_iter()
            .find(|watch| watch.watch_id == watch_id && watch.state != "invalid_record")
            .ok_or_else(|| GardenError::Invalid("The watched directory is unknown.".into()))?;
        watch.state = "unregistering".into();
        watch.detail = None;
        self.watch_readers.remove(&watch.watch_id);
        self.save_watch(&watch)?;
        self.finish_unregister_watched_directory(&watch)
    }

    pub(super) fn recover_unregistering_watches(&mut self) -> Result<()> {
        for watch in self.list_watched_directories()? {
            if watch.state == "unregistering" {
                self.finish_unregister_watched_directory(&watch)?;
            }
        }
        Ok(())
    }

    fn finish_unregister_watched_directory(&mut self, watch: &WatchedDirectory) -> Result<()> {
        let directory = self.root.join("watches").join(&watch.watch_id);
        validate_watch(watch, &directory)?;
        let files = directory.join("files");
        if files.exists() {
            for entry in fs::read_dir(&files)? {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    return Err(GardenError::Invalid("Watch receipt is not a file.".into()));
                }
                let receipt: FileReceipt = read_record(&entry.path())?;
                validate_watch_relative(&receipt.relative_path)?;
                let origin = watch.path.join(&receipt.relative_path);
                self.backup_and_withdraw_watched_source(
                    &receipt.source_id,
                    &origin,
                    &entry.path(),
                )?;
            }
        }
        commit_publication_with_removals(
            &self.root,
            Vec::new(),
            vec![directory],
            &HashMap::new(),
            None,
        )?;
        Ok(())
    }

    pub fn list_watched_directories(&self) -> Result<Vec<WatchedDirectory>> {
        let root = self.root.join("watches");
        if !root.exists() {
            return Ok(Vec::new());
        }
        if !fs::symlink_metadata(&root)?.is_dir() {
            return Err(GardenError::Invalid(
                "Watch storage is not a real directory.".into(),
            ));
        }
        let mut watches = Vec::new();
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let loaded = read_record::<WatchedDirectory>(&entry.path().join("index.md"))
                    .and_then(|watch| {
                        validate_watch(&watch, &entry.path())?;
                        Ok(watch)
                    });
                watches.push(match loaded {
                    Ok(watch) => watch,
                    Err(error) => WatchedDirectory {
                        watch_id: entry.file_name().to_string_lossy().into_owned(),
                        path: entry.path(),
                        state: "invalid_record".into(),
                        detail: Some(error.to_string()),
                        processed_files: 0,
                        device: 0,
                        inode: 0,
                        volume_uuid: None,
                        volume_retry_at: 0,
                        generation: 0,
                        completed_generation: 0,
                        pending: Vec::new(),
                        active: None,
                        withdrawing: false,
                    },
                });
            }
        }
        watches.sort_by(|a: &WatchedDirectory, b| a.path.cmp(&b.path));
        Ok(watches)
    }

    /// One bounded worker turn; callers release the collection lock between turns.
    /// A stream is held only for the active directory. Restart replays that directory
    /// using durable receipts, then continues the saved frontier.
    pub fn reconcile_watched_directories(&mut self, work_budget: usize) -> Result<()> {
        self.recover_pending_publications()?;
        self.recover_source_removals()?;
        self.recover_unregistering_watches()?;
        let mut watches = self.list_watched_directories()?;
        let mut remaining = work_budget.min(128);
        let count = watches.len();
        if count == 0 || remaining == 0 {
            return Ok(());
        }
        watches.rotate_left(self.watch_cursor % count);
        self.watch_cursor = (self.watch_cursor + 1) % count;
        for (index, mut watch) in watches.into_iter().enumerate() {
            if remaining == 0 {
                break;
            }
            if watch.state == "invalid_record" {
                continue;
            }
            let allowance = (remaining / (count - index)).max(1);
            if watch.state != "scanning" {
                start_scan(&mut watch);
            }
            let result = (|| {
                for _ in 0..allowance {
                    remaining -= 1;
                    check_root(&mut watch)?;
                    match self.watch_step(&mut watch) {
                        Ok(false) => break,
                        Ok(true) => {}
                        Err(error) if !watch.withdrawing => {
                            // Keep discovering unrelated files, but this scan can
                            // never supply absence evidence. Retry on the next pass.
                            watch.detail = Some(error.to_string());
                            if !self.watch_readers.contains_key(&watch.watch_id) {
                                watch.active = None;
                            }
                        }
                        Err(error) => return Err(error),
                    }
                    self.save_watch(&watch)?;
                }
                Ok::<_, GardenError>(())
            })();
            if let Err(error) = result {
                watch.state = "unavailable".into();
                watch.detail = Some(error.to_string());
                self.watch_readers.remove(&watch.watch_id);
            }
            self.save_watch(&watch)?;
        }
        Ok(())
    }

    fn save_watch(&self, watch: &WatchedDirectory) -> Result<()> {
        let directory = self.root.join("watches").join(&watch.watch_id);
        validate_watch(watch, &directory)?;
        validate_real_collection_ancestors(&self.root, &directory.join("index.md"))?;
        write_record(
            &self
                .root
                .join("watches")
                .join(&watch.watch_id)
                .join("index.md"),
            watch,
        )
    }

    fn watch_step(&mut self, watch: &mut WatchedDirectory) -> Result<bool> {
        let watch_root = self.root.join("watches").join(&watch.watch_id);
        if !self.watch_readers.contains_key(&watch.watch_id) {
            let directory = if watch.withdrawing {
                watch_root.join("files")
            } else {
                let active = watch.active.take().or_else(|| watch.pending.pop());
                let Some(active) = active else {
                    if watch.detail.is_some() {
                        watch.state = "unavailable".into();
                        return Ok(false);
                    }
                    watch.withdrawing = true;
                    return Ok(true);
                };
                let directory = watch.path.join(&active);
                watch.active = Some(active);
                self.check_directory_volume(watch, &directory)?;
                readable_directory(&directory)?;
                directory
            };
            // Persist the popped frontier before consuming any entries.
            self.save_watch(watch)?;
            self.watch_readers
                .insert(watch.watch_id.clone(), fs::read_dir(directory)?);
        }
        let next = self.watch_readers.get_mut(&watch.watch_id).unwrap().next();
        let Some(entry) = next else {
            self.watch_readers.remove(&watch.watch_id);
            watch.active = None;
            if watch.withdrawing {
                check_root(watch)?;
                watch.completed_generation = watch.generation;
                watch.state = "watching".into();
                watch.detail = None;
                return Ok(false);
            }
            return Ok(true);
        };
        let entry = entry?;
        if watch.withdrawing {
            if entry.file_type()?.is_file() {
                self.check_receipt(watch, &entry.path())?;
            }
            return Ok(true);
        }
        let kind = entry.file_type()?;
        let path = entry.path();
        let relative = path
            .strip_prefix(&watch.path)
            .map_err(|_| GardenError::Invalid("Watch entry escaped its directory.".into()))?
            .to_path_buf();
        if kind.is_dir() {
            if !path.starts_with(&self.root) && !watch.pending.contains(&relative) {
                self.check_directory_volume(watch, &path)?;
                readable_directory(&path)?;
                watch.pending.push(relative);
            }
        } else if kind.is_file() {
            if self.origin_is_suppressed(&path) {
                return Ok(true);
            }
            let receipt_path = watch_root.join("files").join(format!(
                "{}.md",
                hex::encode(Sha256::digest(relative.as_os_str().as_encoded_bytes()))
            ));
            if receipt_path.exists() {
                let receipt: FileReceipt = read_record(&receipt_path)?;
                if receipt.seen == watch.generation {
                    return Ok(true);
                }
            }
            let metadata = fs::symlink_metadata(&path)?;
            let fingerprint = format!(
                "{}:{}:{}:{}:{:?}",
                metadata.dev(),
                metadata.ino(),
                metadata.len(),
                metadata.mode(),
                metadata.modified().ok()
            );
            let retry_path = watch_root
                .join("retries")
                .join(receipt_path.file_name().unwrap());
            let prior = read_record::<FileRetry>(&retry_path)
                .ok()
                .filter(|retry| retry.fingerprint == fingerprint);
            let now = now_millis()?.min(u128::from(u64::MAX)) as u64;
            if prior.as_ref().is_some_and(|retry| now < retry.retry_at) {
                return Err(GardenError::Invalid("A failed watch input is waiting for its retry deadline; retained sources are preserved.".into()));
            }
            let page = match readable_file(&path).and_then(|()| {
                self.import_source_with_identity(
                    &path,
                    AcquisitionMethod::Watch,
                    None,
                    None,
                    None,
                    Some(watch.watch_id.clone()),
                )
            }) {
                Ok(page) => {
                    if retry_path.exists() {
                        fs::remove_file(&retry_path)?;
                    }
                    page
                }
                Err(error) => {
                    let attempts = prior.map_or(1, |retry| retry.attempts.saturating_add(1));
                    let delay = 5_000_u64
                        .saturating_mul(1_u64 << attempts.saturating_sub(1).min(6))
                        .min(300_000);
                    write_record(
                        &retry_path,
                        &FileRetry {
                            fingerprint,
                            attempts,
                            retry_at: now.saturating_add(delay),
                        },
                    )?;
                    return Err(error);
                }
            };
            write_record(
                &receipt_path,
                &FileReceipt {
                    relative_path: relative,
                    source_id: page.info.source_id,
                    seen: watch.generation,
                    missing: None,
                },
            )?;
            watch.processed_files += 1;
        }
        Ok(true)
    }

    fn check_receipt(&mut self, watch: &mut WatchedDirectory, path: &Path) -> Result<()> {
        let mut receipt: FileReceipt = read_record(path)?;
        validate_watch_relative(&receipt.relative_path)?;
        if receipt.seen == watch.generation {
            return Ok(());
        }
        // Inspect absence without following a replacement symlink. NotFound alone
        // does not distinguish a disconnected root; check identity and parent access.
        let source_path = watch.path.join(&receipt.relative_path);
        match fs::symlink_metadata(&source_path) {
            Ok(_) => {
                receipt.missing = None;
                write_record(path, &receipt)?;
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        check_root(watch)?;
        let mut parent = source_path.parent().unwrap();
        loop {
            self.check_directory_volume(watch, parent)?;
            match fs::symlink_metadata(parent) {
                Ok(metadata) if metadata.is_dir() => {
                    readable_directory(parent)?;
                    break;
                }
                Ok(_) => {
                    return Err(GardenError::Invalid(
                        "A source parent was replaced; retained content is unavailable.".into(),
                    ))
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound && parent != watch.path =>
                {
                    parent = parent.parent().unwrap();
                }
                Err(error) => return Err(error.into()),
            }
        }
        if receipt.missing == Some(watch.completed_generation) && watch.completed_generation != 0 {
            self.backup_and_withdraw_watched_source(&receipt.source_id, &source_path, path)?;
        } else if receipt.missing != Some(watch.generation) {
            receipt.missing = Some(watch.generation);
            write_record(path, &receipt)?;
        }
        Ok(())
    }

    fn check_directory_volume(&self, watch: &WatchedDirectory, path: &Path) -> Result<()> {
        if path == watch.path {
            return Ok(());
        }
        let relative = path
            .strip_prefix(&watch.path)
            .map_err(|_| GardenError::Invalid("Directory escaped its watch.".into()))?;
        let receipt_path = self
            .root
            .join("watches")
            .join(&watch.watch_id)
            .join("directories")
            .join(format!(
                "{}.md",
                hex::encode(Sha256::digest(relative.as_os_str().as_encoded_bytes()))
            ));
        let old = if receipt_path.exists() {
            Some(read_record::<DirectoryVolume>(&receipt_path)?)
        } else {
            None
        };
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && !old.as_ref().is_some_and(|old| old.mount_boundary) =>
            {
                return Ok(())
            }
            Err(error) => return Err(error.into()),
        };
        if let Some(old) = &old {
            if old.device == metadata.dev() {
                return Ok(());
            }
            if old.volume_uuid.is_none() || old.volume_uuid != volume_uuid(path) {
                return Err(GardenError::Invalid(format!(
                    "A nested volume is unavailable: {}",
                    path.display()
                )));
            }
        }
        // Only device changes require an OS volume query. Ordinary directory
        // replacements on the same volume remain normal reconciliation work.
        write_record(
            &receipt_path,
            &DirectoryVolume {
                device: metadata.dev(),
                volume_uuid: if metadata.dev() == watch.device {
                    watch.volume_uuid.clone()
                } else {
                    volume_uuid(path)
                },
                mount_boundary: path
                    .parent()
                    .and_then(|parent| fs::symlink_metadata(parent).ok())
                    .is_some_and(|parent| metadata.dev() != parent.dev()),
            },
        )
    }

    pub(super) fn source_generation(&self, source_id: &str) -> Result<String> {
        let path = self.root.join(".recovery/generations").join(source_id);
        validate_real_collection_ancestors(&self.root, &path)?;
        if path.exists() {
            return Ok(fs::read_to_string(path)?);
        }
        let generation = Uuid::new_v4().to_string();
        self.set_source_generation(source_id, &generation)?;
        Ok(generation)
    }

    pub(super) fn set_source_generation(&self, source_id: &str, generation: &str) -> Result<()> {
        self.source_dir(source_id)?;
        validate_real_collection_ancestors(
            &self.root,
            &self.root.join(".recovery/generations").join(source_id),
        )?;
        write_atomic(
            &self.root.join(".recovery/generations").join(source_id),
            generation.as_bytes(),
        )
    }

    pub(super) fn origin_is_suppressed(&self, path: &Path) -> bool {
        let origin = normalized_origin(path);
        let name = hex::encode(Sha256::digest(origin.as_os_str().as_encoded_bytes()));
        self.root
            .join(".recovery/suppressed-origins")
            .join(name)
            .exists()
    }

    pub(super) fn clear_origin_suppression(&self, path: &Path) -> Result<()> {
        let origin = normalized_origin(path);
        let name = hex::encode(Sha256::digest(origin.as_os_str().as_encoded_bytes()));
        let file = self.root.join(".recovery/suppressed-origins").join(name);
        match fs::remove_file(&file) {
            Ok(()) => sync_directory(file.parent().unwrap()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn remove_source(&mut self, source_id: &str) -> Result<()> {
        self.recover_pending_publications()?;
        self.prepare_source_removal(source_id, None, None, None, None)
    }

    fn backup_and_withdraw_watched_source(
        &mut self,
        source_id: &str,
        origin: &Path,
        receipt_path: &Path,
    ) -> Result<()> {
        if !self.retained_source_exists(source_id)? {
            return self.withdraw_watched_source(
                source_id,
                Some(origin),
                Some(receipt_path),
                None,
                None,
                false,
            );
        }
        self.prepare_source_removal(source_id, Some(origin), Some(receipt_path), None, None)
    }

    pub(super) fn withdraw_repository_acquisition(
        &mut self,
        source_id: &str,
        origin: &Path,
        registration_id: &str,
    ) -> Result<()> {
        self.prepare_source_removal(
            source_id,
            Some(origin),
            None,
            None,
            Some((
                AcquisitionMethod::Repository,
                Some(registration_id.to_owned()),
            )),
        )
    }

    pub(super) fn withdraw_legacy_repository_acquisition(
        &mut self,
        source_id: &str,
        origin: &Path,
    ) -> Result<()> {
        self.prepare_source_removal(
            source_id,
            Some(origin),
            None,
            None,
            Some((AcquisitionMethod::Repository, None)),
        )
    }

    pub(super) fn reconcile_reacquired_version(
        &mut self,
        source_id: &str,
        owner: &AcquisitionOwner,
        version_id: &str,
    ) -> Result<()> {
        self.prepare_source_removal(
            source_id,
            None,
            None,
            Some(VersionReassignment {
                owner: owner.clone(),
                version_id: version_id.to_owned(),
            }),
            None,
        )
    }

    fn prepare_source_removal(
        &mut self,
        source_id: &str,
        origin: Option<&Path>,
        receipt_path: Option<&Path>,
        reassignment: Option<VersionReassignment>,
        registration: Option<(AcquisitionMethod, Option<String>)>,
    ) -> Result<()> {
        let source = self.open_source(source_id)?;
        let backup_root = self
            .root
            .join(".recovery/sources")
            .join(Uuid::new_v4().to_string());
        validate_real_collection_ancestors(&self.root, &backup_root)?;
        fs::create_dir_all(&backup_root)?;
        sync_directory(&self.root.join(".recovery"))?;
        sync_directory(&self.root)?;
        let bundle = self.source_dir(source_id)?;
        copy_verified_bundle(&bundle, &backup_root.join("bundle"))?;
        self.source_recovery_checkpoint("backup")?;
        let origins = source
            .info
            .acquisitions
            .iter()
            .map(acquisition_origin)
            .collect::<Result<Vec<_>>>()?;
        let record = RemovalRecord {
            source_id: source_id.to_owned(),
            origins,
            generation: Uuid::new_v4().to_string(),
            bundle_digest: digest_bundle(&backup_root.join("bundle"))?,
            watch_origin: origin.map(normalized_origin),
            withdraw_method: registration
                .as_ref()
                .map(|value| value.0)
                .or(origin.map(|_| AcquisitionMethod::Watch)),
            watch_registration: registration
                .as_ref()
                .and_then(|value| value.1.clone())
                .or_else(|| {
                    receipt_path
                        .and_then(Path::parent)
                        .and_then(Path::parent)
                        .and_then(Path::file_name)
                        .and_then(|value| value.to_str())
                        .map(str::to_owned)
                }),
            reassignment,
            receipt: receipt_path
                .map(|path| path.strip_prefix(&self.root).map(Path::to_owned))
                .transpose()
                .map_err(|_| {
                    GardenError::Invalid("Watch receipt escaped the collection.".into())
                })?,
        };
        write_record(&backup_root.join("removal.md"), &record)?;
        sync_directory(&backup_root)?;
        sync_directory(backup_root.parent().unwrap())?;
        self.source_recovery_checkpoint("intent")?;
        self.finish_source_removal(&backup_root, &record)
    }

    pub fn list_removed_sources(&self) -> Result<Vec<RemovedSource>> {
        let root = self.root.join(".recovery/sources");
        validate_real_collection_ancestors(&self.root, &root.join("entry"))?;
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut result = Vec::new();
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() || !entry.path().join("completed").exists() {
                continue;
            }
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if Uuid::parse_str(&id).is_err() {
                continue;
            }
            let page = read_page(&entry.path().join("bundle/index.md"))?;
            if self.retained_source_exists(&page.info.source_id)? {
                continue;
            }
            result.push(RemovedSource {
                recovery_id: id,
                source_id: page.info.source_id,
                title: page.info.title,
            });
        }
        result.sort_by(|a, b| {
            a.title
                .cmp(&b.title)
                .then(a.recovery_id.cmp(&b.recovery_id))
        });
        Ok(result)
    }

    pub fn restore_removed_source(&mut self, recovery_id: &str) -> Result<SourcePage> {
        self.recover_source_removals()?;
        let id = Uuid::parse_str(recovery_id)
            .map_err(|_| GardenError::Invalid("Invalid recovery identity.".into()))?;
        let backup = self.root.join(".recovery/sources").join(id.to_string());
        validate_real_collection_ancestors(&self.root, &backup.join("bundle/index.md"))?;
        if !backup.join("completed").exists() {
            return Err(GardenError::Invalid(
                "This source backup is not complete.".into(),
            ));
        }
        let record: RemovalRecord = read_record(&backup.join("removal.md"))?;
        verify_recovery_bundle(&backup, &record)?;
        if self.retained_source_exists(&record.source_id)? {
            return Err(GardenError::Invalid(
                "This source is already active.".into(),
            ));
        }
        let generation = Uuid::new_v4().to_string();
        write_atomic(&backup.join("restore-pending"), generation.as_bytes())?;
        self.source_recovery_checkpoint("restore-intent")?;
        self.finish_source_restore(&backup, &generation)
    }

    fn finish_source_restore(&mut self, backup: &Path, generation: &str) -> Result<SourcePage> {
        let record: RemovalRecord = read_record(&backup.join("removal.md"))?;
        verify_recovery_bundle(backup, &record)?;
        let destination = self.source_dir(&record.source_id)?;
        self.set_source_generation(&record.source_id, generation)?;
        if !destination.exists() {
            let recovery_id = backup
                .file_name()
                .ok_or_else(|| GardenError::Invalid("Recovery identity is missing.".into()))?;
            let staging_root = self.root.join(".staging/restores");
            let staging = staging_root.join(recovery_id);
            validate_real_collection_ancestors(&self.root, &staging.join("index.md"))?;
            fs::create_dir_all(&staging_root)?;
            if staging.exists() {
                fs::remove_dir_all(&staging)?;
            }
            copy_verified_bundle(&backup.join("bundle"), &staging)?;
            let mut page = read_page(&staging.join("index.md"))?;
            if page.info.source_id != record.source_id {
                return Err(GardenError::Invalid(
                    "Recovery identity does not match the source bundle.".into(),
                ));
            }
            let active_version = page
                .info
                .pending_version_id
                .clone()
                .or_else(|| page.info.current_version_id.clone());
            page.info.knowledge_pages.clear();
            page.body = replace_semantic_section(&page.body, &[]);
            page.info.semantic_claim = None;
            page.info.semantic_attempts = 0;
            page.info.semantic_retry_at = None;
            if let Some(version_id) = active_version {
                if page.info.semantic_state != "unavailable" {
                    page.info.current_version_id = None;
                    page.info.pending_version_id = Some(version_id.clone());
                    page.info.semantic_state = "pending".into();
                    page.info.update_status = None;
                    if let Some(version) = page
                        .info
                        .versions_seen
                        .iter_mut()
                        .find(|v| v.source_version_id == version_id)
                    {
                        version.state = "pending".into();
                    }
                }
            }
            write_atomic(
                &staging.join("index.md"),
                serialize_page(&page.info, &page.body)?.as_bytes(),
            )?;
            sync_directory(&staging)?;
            self.source_recovery_checkpoint("restore-staged")?;
            fs::rename(&staging, &destination)?;
            sync_directory(destination.parent().unwrap())?;
            self.source_recovery_checkpoint("restore-published")?;
        }
        self.rebuild_index()?;
        for origin in &record.origins {
            self.clear_origin_suppression(origin)?;
        }
        fs::remove_file(backup.join("restore-pending"))?;
        sync_directory(backup)?;
        self.open_source(&record.source_id)
    }

    pub fn recover_source_removals(&mut self) -> Result<()> {
        let root = self.root.join(".recovery/sources");
        validate_real_collection_ancestors(&self.root, &root)?;
        if !root.exists() {
            return Ok(());
        }
        if !fs::symlink_metadata(&root)?.is_dir() {
            return Err(GardenError::Invalid(
                "Recovery storage is not a directory.".into(),
            ));
        }
        for entry in fs::read_dir(root)? {
            let path = entry?.path();
            let record = path.join("removal.md");
            if record.exists() && !path.join("completed").exists() {
                let removal: RemovalRecord = read_record(&record)?;
                self.finish_source_removal(&path, &removal)?;
            }
            let restore = path.join("restore-pending");
            if restore.exists() {
                let generation = fs::read_to_string(&restore)?;
                self.finish_source_restore(&path, &generation)?;
            }
        }
        Ok(())
    }

    fn finish_source_removal(&mut self, path: &Path, removal: &RemovalRecord) -> Result<()> {
        verify_recovery_bundle(path, removal)?;
        self.set_source_generation(&removal.source_id, &removal.generation)?;
        self.source_recovery_checkpoint("generation")?;
        let suppressed = self.root.join(".recovery/suppressed-origins");
        fs::create_dir_all(&suppressed)?;
        sync_directory(&self.root.join(".recovery"))?;
        for origin in removal
            .origins
            .iter()
            .filter(|_| removal.watch_origin.is_none() && removal.reassignment.is_none())
        {
            let name = hex::encode(Sha256::digest(origin.as_os_str().as_encoded_bytes()));
            write_record(&suppressed.join(name), origin)?;
        }
        self.source_recovery_checkpoint("suppression")?;
        let receipt = removal
            .receipt
            .as_ref()
            .map(|path| {
                validate_relative_collection_path(path)?;
                Ok::<_, GardenError>(self.root.join(path))
            })
            .transpose()?;
        if let Some(change) = &removal.reassignment {
            let mut source = self.open_source(&removal.source_id)?;
            let mut found = false;
            for version in &mut source.info.versions_seen {
                if version.source_version_id == change.version_id {
                    found = true;
                    if !version.acquisition_owners.contains(&change.owner) {
                        version.acquisition_owners.push(change.owner.clone());
                    }
                    version.withdrawn = false;
                } else {
                    version
                        .acquisition_owners
                        .retain(|owner| owner != &change.owner);
                    if version.acquisition_owners.is_empty() {
                        version.withdrawn = true;
                    }
                }
            }
            if !found {
                return Err(GardenError::Invalid(
                    "The reacquired source version is missing.".into(),
                ));
            }
            self.write_source_page(&mut source)?;
        }
        self.withdraw_watched_source(
            &removal.source_id,
            removal.watch_origin.as_deref(),
            receipt.as_deref(),
            removal.watch_registration.as_deref(),
            removal.withdraw_method,
            removal.reassignment.is_some(),
        )?;
        if removal.watch_origin.is_some() && self.retained_source_exists(&removal.source_id)? {
            let mut page = self.open_source(&removal.source_id)?;
            let initial = page.info.current_version_id.is_none();
            let processing = if initial {
                page.info.semantic_state == "processing"
            } else {
                page.info.update_status.as_deref() == Some("processing")
            };
            if processing {
                if initial {
                    page.info.semantic_state = "pending".into();
                } else {
                    page.info.update_status = Some("pending".into());
                }
                page.info.semantic_claim = None;
                page.info.semantic_retry_at = None;
                if let Some(version) = page.info.versions_seen.iter_mut().find(|version| {
                    Some(&version.source_version_id) == page.info.pending_version_id.as_ref()
                }) {
                    version.state = "pending".into();
                }
                self.write_source_page(&mut page)?;
                self.index_page(&page.info)?;
            }
        }
        self.source_recovery_checkpoint("withdrawn")?;
        write_atomic(&path.join("completed"), b"completed\n")?;
        Ok(())
    }

    fn withdraw_watched_source(
        &mut self,
        source_id: &str,
        origin: Option<&Path>,
        receipt_path: Option<&Path>,
        watch_registration: Option<&str>,
        withdraw_method: Option<AcquisitionMethod>,
        preserve_acquisitions: bool,
    ) -> Result<()> {
        let origin = origin.map(normalized_origin);
        if !self.retained_source_exists(source_id)? {
            // An overlapping watch already withdrew this same source.
            commit_publication_with_removals(
                &self.root,
                Vec::new(),
                receipt_path.map(Path::to_owned).into_iter().collect(),
                &HashMap::new(),
                None,
            )?;
            return Ok(());
        }
        let mut source = self.open_source(source_id)?;
        let mut retained_acquisitions = Vec::new();
        for acquisition in source.info.acquisitions.drain(..) {
            if preserve_acquisitions {
                retained_acquisitions.push(acquisition);
            } else if let Some(origin) = &origin {
                let matches_watch = acquisition_origin(&acquisition)? == *origin
                    && Some(acquisition.method) == withdraw_method
                    && acquisition.registration_id.as_deref() == watch_registration
                    && (watch_registration.is_some()
                        || withdraw_method == Some(AcquisitionMethod::Repository));
                if !matches_watch {
                    retained_acquisitions.push(acquisition);
                }
            }
        }
        source.info.acquisitions = retained_acquisitions;
        if withdraw_method == Some(AcquisitionMethod::Repository)
            && source.info.repository.as_ref().is_some_and(|repository| {
                watch_registration.is_none()
                    || Some(repository.repository_id.as_str()) == watch_registration
            })
        {
            source.info.repository = None;
        }
        let contexts = self.source_dir(source_id)?.join("acquisitions");
        let mut removed_contexts: Vec<PathBuf> =
            receipt_path.map(Path::to_owned).into_iter().collect();
        if contexts.exists() && !preserve_acquisitions {
            for entry in fs::read_dir(contexts)? {
                let entry = entry?;
                if entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "md")
                {
                    let acquisition: Acquisition = read_record(&entry.path())?;
                    if origin.is_none()
                        || (acquisition_origin(&acquisition)? == *origin.as_ref().unwrap()
                            && Some(acquisition.method) == withdraw_method
                            && acquisition.registration_id.as_deref() == watch_registration
                            && (watch_registration.is_some()
                                || withdraw_method == Some(AcquisitionMethod::Repository)))
                    {
                        removed_contexts.push(entry.path());
                    }
                }
            }
        }
        if let Some(origin) = &origin {
            for version in &mut source.info.versions_seen {
                version.acquisition_owners.retain(|owner| {
                    !(owner.origin == *origin
                        && Some(owner.method) == withdraw_method
                        && owner.registration_id.as_deref() == watch_registration
                        && (watch_registration.is_some()
                            || withdraw_method == Some(AcquisitionMethod::Repository)))
                });
                if version.acquisition_owners.is_empty() {
                    version.withdrawn = true;
                }
            }
        }
        let retained = !source.info.acquisitions.is_empty();
        let has_owner = |id: &str| {
            source.info.versions_seen.iter().any(|version| {
                version.source_version_id == id && !version.acquisition_owners.is_empty()
            })
        };
        let current_supported = source
            .info
            .current_version_id
            .as_deref()
            .is_some_and(has_owner);
        let pending_supported = source
            .info
            .pending_version_id
            .as_deref()
            .is_some_and(has_owner);
        let had_current = source.info.current_version_id.is_some();
        if retained && (current_supported || (!had_current && pending_supported)) {
            if source.info.pending_version_id.is_some() && !pending_supported {
                source.info.pending_version_id = None;
                source.info.update_status = None;
                source.info.semantic_state = "complete".into();
                source.info.semantic_claim = None;
            }
            let files = vec![(
                self.page_path(source_id)?,
                serialize_page(&source.info, &source.body)?.into_bytes(),
            )];
            commit_publication_with_removals(
                &self.root,
                files,
                removed_contexts,
                &HashMap::new(),
                self.fail_after_publication_files.take(),
            )?;
            return self.rebuild_index();
        }
        self.capture_external_knowledge_edits()?;
        let removed = HashSet::from([source_id.to_owned()]);
        let mut files = Vec::new();
        let mut changed_owners = HashMap::<String, SourcePage>::new();
        let mut removals = if retained {
            removed_contexts
        } else {
            vec![self.source_dir(source_id)?]
        };
        if !retained {
            removals.extend(receipt_path.map(Path::to_owned));
        }
        if retained {
            let candidates = source
                .info
                .versions_seen
                .iter()
                .filter(|version| !version.acquisition_owners.is_empty())
                .cloned()
                .collect::<Vec<_>>();
            let fallback = candidates
                .iter()
                .find(|candidate| {
                    candidates.iter().all(|other| {
                        candidate.source_version_id == other.source_version_id
                            || compare_source_order(other, candidate)
                                == Some(std::cmp::Ordering::Greater)
                    })
                })
                .cloned();
            let old_original = self.source_dir(source_id)?.join(&source.info.asset);
            if candidates.len() > 1 && fallback.is_none() {
                source.info.ordering_uncertain = true;
            }
            source.info.current_version_id = None;
            source.info.pending_version_id = None;
            source.info.knowledge_pages.clear();
            source.info.semantic_decisions.clear();
            source.info.semantic_claim = None;
            source.info.semantic_retry_at = None;
            source.info.update_status = None;
            source.info.semantic_state = "unavailable".into();
            source.info.semantic_error = Some("The withdrawn origin was the only known support for the active version. Remaining version ownership or extraction is unverified.".into());
            source.body = format!("# {}\n\nThe previous active version is no longer supported by an active origin. Retained versions remain in the recovery copy.\n", source.info.title);
            if let Some(version) = fallback {
                let path = self.version_original_path(source_id, &version)?;
                if !path.is_file() {
                    return Err(GardenError::Invalid(
                        "The supported retained version is missing; withdrawal remains pending."
                            .into(),
                    ));
                }
                let destination = self.source_dir(source_id)?.join(&version.asset);
                write_atomic_from_file(&destination, &path)?;
                self.source_recovery_checkpoint("fallback-original")?;
                if old_original != destination {
                    removals.push(old_original);
                }
                source.info.sha256 = version.sha256.clone();
                source.info.bytes = version.bytes;
                source.info.asset = version.asset.clone();
                source.info.original_name = version.original_name.clone();
                source.info.format = version.format.clone();
                source.info.extraction_version_id = None;
                source.info.extraction_coverage = None;
                source.info.pending_version_id = Some(version.source_version_id.clone());
                source.info.semantic_state = "pending".into();
                source.info.semantic_error = None;
                if is_timed_media_format(&version.format) {
                    let (audio, media) = self.inspect_timed_media(true, &path);
                    source.info.audio_processing = audio.map(|audio| {
                        audio_processing_info(audio, &version.source_version_id, media)
                    });
                    source.info.pending_audio_processing = None;
                    source.info.extraction = ExtractionState::PartialText;
                    source.info.extraction_detail =
                        "The supported recording is queued for renewed transcription.".into();
                    source.body = source_body(&source.info, "", None);
                } else if crate::extraction::is_structured_source(&path, &version.format) {
                    source.info.extraction = ExtractionState::PartialText;
                    source.info.extraction_detail =
                        "The supported source version is queued for renewed extraction.".into();
                    source.body = source_body(&source.info, "", None);
                } else if version.bytes <= MAX_TEXT_BYTES as u64 {
                    match fs::read_to_string(&path) {
                        Ok(text) => {
                            source.info.extraction = ExtractionState::TextPreserved;
                            source.info.extraction_detail = "The supported source version is queued for renewed knowledge processing.".into();
                            source.info.extraction_version_id =
                                Some(version.source_version_id.clone());
                            source.info.line_count = text.lines().count();
                            source.body = source_body(&source.info, &text, None);
                        }
                        Err(error) => {
                            source.info.extraction = ExtractionState::InvalidUtf8;
                            source.info.semantic_state = "unavailable".into();
                            source.info.semantic_error = Some(format!(
                                "The supported version cannot be read as text: {error}"
                            ));
                            source.info.pending_version_id = None;
                        }
                    }
                } else {
                    source.info.extraction = ExtractionState::TooLarge;
                    source.info.semantic_state = "unavailable".into();
                    source.info.semantic_error = Some("The supported version exceeds the current text extraction limit; its complete original remains available.".into());
                    source.info.pending_version_id = None;
                }
            } else {
                removals.push(old_original);
                if candidates.len() > 1 {
                    source.info.semantic_error = Some("Remaining source versions have no reliable ordering; their evidence cannot be promoted automatically.".into());
                }
            }
            files.push((
                self.page_path(source_id)?,
                serialize_page(&source.info, &source.body)?.into_bytes(),
            ));
        }
        let paths = self.knowledge_paths()?;
        let mut referenced = HashMap::new();
        for path in &paths {
            let header = read_knowledge_header(path)?;
            for relationship in header.relationships {
                if let Some(support) = relationship
                    .supports
                    .iter()
                    .find(|support| support.source_id != source_id)
                {
                    for page_id in [relationship.from_page_id, relationship.to_page_id] {
                        referenced.insert(
                            page_id,
                            (support.source_id.clone(), support.evidence.clone()),
                        );
                    }
                }
            }
        }
        for path in paths {
            let mut header = read_knowledge_header(&path)?;
            ensure_legacy_supports(&mut header);
            if !knowledge_header_supported_by(&header, source_id) && header.source_id != source_id {
                continue;
            }
            header.facts = self.reconcile_facts(header.facts, Vec::new(), &removed)?;
            header.relationships =
                self.reconcile_relationships(header.relationships, Vec::new(), &removed)?;
            header.tags = self.reconcile_tags(header.tags, Vec::new(), &removed);
            if header.facts.is_empty()
                && header.relationships.is_empty()
                && header.tags.is_empty()
                && header.owner_content.is_empty()
                && header.uninterpreted_owner_edits.is_empty()
                && header.owner_metadata.is_empty()
                && header.external_edit_status.is_none()
                && !referenced.contains_key(&header.page_id)
            {
                removals.push(path);
                continue;
            }
            // Shared pages need an extant owner for indexing. A manual-only page
            // retains its identity and is indexed independently of source bundles.
            if header.source_id == source_id {
                let support = header
                    .facts
                    .iter()
                    .flat_map(|fact| &fact.supports)
                    .chain(header.relationships.iter().flat_map(|rel| &rel.supports))
                    .next()
                    .map(|support| (support.source_id.clone(), support.evidence.clone()))
                    .or_else(|| {
                        header
                            .tags
                            .iter()
                            .flat_map(|tag| &tag.supports)
                            .next()
                            .map(|support| (support.source_id.clone(), support.evidence.clone()))
                    })
                    .or_else(|| referenced.get(&header.page_id).cloned());
                if let Some((owner, evidence)) = support {
                    header.source_id = owner;
                    header.evidence = evidence;
                }
            }
            let summary = KnowledgePageSummary {
                page_id: header.page_id.clone(),
                title: header.title.clone(),
                kind: header.kind.clone(),
                path: path
                    .strip_prefix(&self.root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            };
            if header.source_id != source_id {
                if !changed_owners.contains_key(&header.source_id) {
                    changed_owners.insert(
                        header.source_id.clone(),
                        self.open_source(&header.source_id)?,
                    );
                }
                let owner = changed_owners.get_mut(&header.source_id).unwrap();
                if !owner
                    .info
                    .knowledge_pages
                    .iter()
                    .any(|page| page.page_id == summary.page_id)
                {
                    owner.info.knowledge_pages.push(summary);
                    owner.body = replace_semantic_section(&owner.body, &owner.info.knowledge_pages);
                }
            }
            files.push((
                path,
                self.render_knowledge_page(header, &HashMap::new())?
                    .into_bytes(),
            ));
        }
        for (owner_id, owner) in changed_owners {
            files.push((
                self.page_path(&owner_id)?,
                serialize_page(&owner.info, &owner.body)?.into_bytes(),
            ));
        }
        let fail_after = self.fail_after_publication_files.take();
        commit_publication_with_removals(&self.root, files, removals, &HashMap::new(), fail_after)?;
        self.rebuild_index()
    }
}

fn start_scan(watch: &mut WatchedDirectory) {
    watch.generation += 1;
    watch.state = "scanning".into();
    watch.detail = None;
    watch.processed_files = 0;
    watch.pending = vec![PathBuf::new()];
    watch.active = None;
    watch.withdrawing = false;
}

fn check_root(watch: &mut WatchedDirectory) -> Result<()> {
    let metadata = fs::symlink_metadata(&watch.path)?;
    let same_volume = metadata.dev() == watch.device
        || watch
            .volume_uuid
            .as_ref()
            .is_some_and(|expected| volume_uuid(&watch.path).as_ref() == Some(expected));
    if !same_volume || metadata.ino() != watch.inode || !metadata.is_dir() {
        return Err(GardenError::Invalid("The selected directory is unavailable or has a different filesystem identity. Retained sources are preserved.".into()));
    }
    if watch.volume_uuid.is_none() && metadata.dev() == watch.device {
        let now = now_millis()?.min(u128::from(u64::MAX)) as u64;
        if now >= watch.volume_retry_at {
            watch.volume_uuid = volume_uuid(&watch.path);
            watch.volume_retry_at = now.saturating_add(60_000);
        }
    }
    watch.device = metadata.dev();
    readable_directory(&watch.path)
}

/// macOS may assign a different device number on remount. Match the persistent
/// volume UUID as well as the directory inode before accepting that new number.
/// Unsupported/network filesystems conservatively retain the device/inode check.
fn volume_uuid(path: &Path) -> Option<String> {
    let output = bounded_output(
        std::process::Command::new("/bin/df").arg("-P").arg(path),
        std::time::Duration::from_secs(1),
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let device = text.lines().nth(1)?.split_whitespace().next()?;
    if !device.starts_with("/dev/") {
        return None;
    }
    let output = bounded_output(
        std::process::Command::new("/usr/sbin/diskutil").args(["info", "-plist", device]),
        std::time::Duration::from_secs(1),
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let document = roxmltree::Document::parse_with_options(
        &text,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .ok()?;
    document
        .descendants()
        .find(|node| node.has_tag_name("key") && node.text() == Some("VolumeUUID"))?
        .next_sibling_element()?
        .text()
        .map(str::to_owned)
}

fn readable_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.permissions().mode() & 0o444 == 0
        || metadata.permissions().mode() & 0o111 == 0
    {
        return Err(GardenError::Invalid(format!(
            "Directory is unreadable: {}",
            path.display()
        )));
    }
    fs::read_dir(path)?;
    Ok(())
}

fn readable_file(path: &Path) -> Result<()> {
    if fs::symlink_metadata(path)?.permissions().mode() & 0o444 == 0 {
        return Err(GardenError::Invalid(format!(
            "Source is unreadable: {}",
            path.display()
        )));
    }
    File::open(path)?;
    Ok(())
}

fn read_record<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path)?;
    let yaml = text
        .strip_prefix("---\n")
        .and_then(|text| text.split_once("\n---\n"))
        .ok_or_else(|| GardenError::Invalid("Unreadable watch record.".into()))?
        .0;
    Ok(serde_yaml_ng::from_str(yaml)?)
}

fn write_record<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    write_atomic(
        path,
        format!(
            "---\n{}---\n\nAutomatically maintained directory reconciliation record.\n",
            serde_yaml_ng::to_string(value)?
        )
        .as_bytes(),
    )
}

#[derive(Serialize, Deserialize)]
struct FileRetry {
    fingerprint: String,
    attempts: u32,
    retry_at: u64,
}

fn validate_watch_relative(path: &Path) -> Result<()> {
    if path
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(GardenError::Invalid(
            "A watch frontier or receipt escaped its selected root.".into(),
        ));
    }
    Ok(())
}

fn validate_watch(watch: &WatchedDirectory, directory: &Path) -> Result<()> {
    if watch.watch_id.len() != 64
        || !watch
            .watch_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || directory.file_name().and_then(|name| name.to_str()) != Some(watch.watch_id.as_str())
        || !watch.path.is_absolute()
        || watch.watch_id != hex::encode(Sha256::digest(watch.path.as_os_str().as_encoded_bytes()))
    {
        return Err(GardenError::Invalid(
            "Watch identity does not match its storage directory and selected path.".into(),
        ));
    }
    for path in watch.pending.iter().chain(watch.active.iter()) {
        validate_watch_relative(path)?;
    }
    Ok(())
}

fn bounded_output(
    command: &mut std::process::Command,
    timeout: std::time::Duration,
) -> std::io::Result<std::process::Output> {
    use std::process::Stdio;
    let mut child = crate::processes::spawn(command.stdout(Stdio::piped()).stderr(Stdio::null()))?;
    let start = std::time::Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output();
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Volume lookup exceeded its deadline.",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn volume_queries_have_a_deadline() {
        let start = std::time::Instant::now();
        let error = bounded_output(
            std::process::Command::new("/bin/sleep").arg("2"),
            std::time::Duration::from_millis(30),
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
    }
}

#[derive(Serialize, Deserialize)]
struct VersionReassignment {
    owner: AcquisitionOwner,
    version_id: String,
}

#[derive(Serialize, Deserialize)]
struct RemovalRecord {
    source_id: String,
    origins: Vec<PathBuf>,
    generation: String,
    bundle_digest: String,
    #[serde(default)]
    watch_origin: Option<PathBuf>,
    #[serde(default)]
    watch_registration: Option<String>,
    #[serde(default)]
    withdraw_method: Option<AcquisitionMethod>,
    #[serde(default)]
    reassignment: Option<VersionReassignment>,
    #[serde(default)]
    receipt: Option<PathBuf>,
}

fn copy_verified_bundle(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if !metadata.is_dir() {
        return Err(GardenError::Invalid(
            "Source bundle is not a directory.".into(),
        ));
    }
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let input = entry.path();
        let output = destination.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_verified_bundle(&input, &output)?;
        } else if kind.is_file() {
            fs::copy(&input, &output)?;
            File::open(&output)?.sync_all()?;
            let mut original = File::open(&input)?;
            let mut copied = File::open(&output)?;
            let mut left = [0u8; 65536];
            let mut right = [0u8; 65536];
            loop {
                let a = std::io::Read::read(&mut original, &mut left)?;
                let b = std::io::Read::read(&mut copied, &mut right)?;
                if a != b || left[..a] != right[..b] {
                    return Err(GardenError::Invalid(
                        "Source backup verification failed.".into(),
                    ));
                }
                if a == 0 {
                    break;
                }
            }
            if fs::metadata(&input)?.len() != fs::metadata(&output)?.len() {
                return Err(GardenError::Invalid(
                    "Source backup verification failed.".into(),
                ));
            }
        } else {
            return Err(GardenError::Invalid(
                "Source backup cannot contain links or special files.".into(),
            ));
        }
    }
    sync_directory(destination)?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct RemovedSource {
    pub recovery_id: String,
    pub source_id: String,
    pub title: String,
}

fn verify_recovery_bundle(path: &Path, record: &RemovalRecord) -> Result<()> {
    if digest_bundle(&path.join("bundle"))? != record.bundle_digest {
        return Err(GardenError::Invalid(
            "Source recovery copy failed verification.".into(),
        ));
    }
    Ok(())
}

fn digest_bundle(root: &Path) -> Result<String> {
    if !fs::symlink_metadata(root)?.is_dir() {
        return Err(GardenError::Invalid(
            "Recovery bundle is not a directory.".into(),
        ));
    }
    fn visit(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                visit(root, &entry.path(), files)?;
            } else if kind.is_file() {
                files.push(entry.path().strip_prefix(root).unwrap().to_owned());
            } else {
                return Err(GardenError::Invalid(
                    "Recovery copy contains a link or special file.".into(),
                ));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    visit(root, root, &mut files)?;
    files.sort();
    let mut digest = Sha256::new();
    for relative in files {
        let name = relative.as_os_str().as_encoded_bytes();
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name);
        let mut input = File::open(root.join(relative))?;
        let mut contents = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            let count = std::io::Read::read(&mut input, &mut buffer)?;
            if count == 0 {
                break;
            }
            contents.update(&buffer[..count]);
        }
        digest.update(contents.finalize());
    }
    Ok(hex::encode(digest.finalize()))
}
