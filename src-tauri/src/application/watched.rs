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
                generation: 0,
                completed_generation: 0,
                pending: Vec::new(),
                active: None,
                withdrawing: false,
            }
        };
        // Explicit reselection accepts a changed filesystem identity.
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
        let mut watches = Vec::new();
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                watches.push(read_record(&entry.path().join("index.md"))?);
            }
        }
        watches.sort_by(|a: &WatchedDirectory, b| a.path.cmp(&b.path));
        Ok(watches)
    }

    /// One bounded worker turn; callers release the collection lock between turns.
    /// A stream is held only for the active directory. Restart replays that directory
    /// using durable receipts, then continues the saved frontier.
    pub fn reconcile_watched_directories(&mut self, work_budget: usize) -> Result<()> {
        for mut watch in self.list_watched_directories()? {
            if watch.state != "scanning" {
                start_scan(&mut watch);
            }
            let result = (|| {
                check_root(&mut watch)?;
                for _ in 0..work_budget.min(128) {
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
            if path != self.root && !watch.pending.contains(&relative) {
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
            readable_file(&path)?;
            let page = self.import_source(&path, AcquisitionMethod::Watch)?;
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
                mount_boundary: metadata.dev() != watch.device,
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
    watch.device = metadata.dev();
    readable_directory(&watch.path)
}

/// macOS may assign a different device number on remount. Match the persistent
/// volume UUID as well as the directory inode before accepting that new number.
/// Unsupported/network filesystems conservatively retain the device/inode check.
fn volume_uuid(path: &Path) -> Option<String> {
    let output = std::process::Command::new("/bin/df")
        .arg("-P")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let device = text.lines().nth(1)?.split_whitespace().next()?;
    if !device.starts_with("/dev/") {
        return None;
    }
    let output = std::process::Command::new("/usr/sbin/diskutil")
        .args(["info", "-plist", device])
        .output()
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
