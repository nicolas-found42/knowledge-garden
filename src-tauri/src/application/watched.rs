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
        let path = fs::canonicalize(path)?;
        readable_directory(&path)?;
        if path.starts_with(&self.root) {
            return Err(GardenError::Invalid(
                "Choose a directory outside the collection.".into(),
            ));
        }
        let watch_id = format!("{:x}", Sha256::digest(path.as_os_str().as_encoded_bytes()));
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
            let receipt_path = watch_root.join("files").join(format!(
                "{:x}.md",
                Sha256::digest(relative.as_os_str().as_encoded_bytes())
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
            let page = match readable_file(&path)
                .and_then(|()| self.import_source(&path, AcquisitionMethod::Watch))
            {
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
            self.withdraw_watched_source(&receipt.source_id, &source_path, path)?;
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
                "{:x}.md",
                Sha256::digest(relative.as_os_str().as_encoded_bytes())
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

    fn withdraw_watched_source(
        &mut self,
        source_id: &str,
        origin: &Path,
        receipt_path: &Path,
    ) -> Result<()> {
        let origin = normalized_origin(origin);
        if !self.retained_source_exists(source_id)? {
            // An overlapping watch already withdrew this same source.
            commit_publication_with_removals(
                &self.root,
                Vec::new(),
                vec![receipt_path.to_owned()],
                None,
            )?;
            return Ok(());
        }
        let mut source = self.open_source(source_id)?;
        source
            .info
            .acquisitions
            .retain(|acquisition| normalized_origin(Path::new(&acquisition.path)) != origin);
        let contexts = self.source_dir(source_id)?.join("acquisitions");
        let mut removed_contexts = vec![receipt_path.to_owned()];
        if contexts.exists() {
            for entry in fs::read_dir(contexts)? {
                let entry = entry?;
                if entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "md")
                {
                    let acquisition: Acquisition = read_record(&entry.path())?;
                    if normalized_origin(Path::new(&acquisition.path)) == origin {
                        removed_contexts.push(entry.path());
                    }
                }
            }
        }
        if !source.info.acquisitions.is_empty() {
            // An established source can have other independent acquisition origins.
            // Selecting a watch adopts its identity without owning those origins.
            let files = vec![(
                self.page_path(source_id)?,
                serialize_page(&source.info, &source.body)?.into_bytes(),
            )];
            commit_publication_with_removals(
                &self.root,
                files,
                removed_contexts,
                self.fail_after_publication_files.take(),
            )?;
            return self.rebuild_index();
        }
        self.capture_external_knowledge_edits()?;
        let removed = HashSet::from([source_id.to_owned()]);
        let mut files = Vec::new();
        let mut changed_owners = HashMap::<String, SourcePage>::new();
        let mut removals = vec![self.source_dir(source_id)?, receipt_path.to_owned()];
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
        commit_publication_with_removals(&self.root, files, removals, fail_after)?;
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
        || watch.watch_id
            != format!(
                "{:x}",
                Sha256::digest(watch.path.as_os_str().as_encoded_bytes())
            )
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
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
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
