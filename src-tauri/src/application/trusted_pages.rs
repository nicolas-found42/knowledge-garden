use super::*;

// Move abandoned staged bytes into inspectable recovery without unlinking an
// inode another writer may still have open. The caller holds the collection lock.
pub(super) fn recover_abandoned_original_copies(root: &Path, budget: usize) -> Result<bool> {
    recover_abandoned_original_copies_with_hook(root, budget, |_| {})
}

fn recover_abandoned_original_copies_with_hook(
    root: &Path,
    budget: usize,
    mut before_rename: impl FnMut(&Path),
) -> Result<bool> {
    use rustix::fs::{mkdirat, openat, renameat_with, Dir, Mode, OFlags, RenameFlags, CWD};
    use rustix::io::Errno;
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let root_fd = openat(CWD, root, flags, Mode::empty()).map_err(std::io::Error::from)?;
    let recovery = match openat(&root_fd, ".recovery", flags, Mode::empty()) {
        Ok(fd) => fd,
        Err(Errno::NOENT) => return Ok(false),
        Err(error) => return Err(std::io::Error::from(error).into()),
    };
    let staging = match openat(&recovery, "verified-original-staging", flags, Mode::empty()) {
        Ok(fd) => fd,
        Err(Errno::NOENT) => return Ok(false),
        Err(error) => return Err(std::io::Error::from(error).into()),
    };
    let mut entries = Dir::read_from(&staging).map_err(std::io::Error::from)?;
    let mut visited = 0;
    while visited < budget.clamp(1, 64) {
        let Some(entry) = entries.read() else {
            return Ok(false);
        };
        let entry = entry.map_err(std::io::Error::from)?;
        let name = entry.file_name();
        if name.to_bytes() == b"." || name.to_bytes() == b".." {
            continue;
        }
        visited += 1;
        if !name.to_bytes().starts_with(b".staged-") {
            continue;
        }
        let fd = match openat(
            &staging,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(Errno::NOENT) => continue,
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        let file = File::from(fd);
        if !file.metadata()?.is_file() {
            continue;
        }
        if let Err(error) = file.try_lock_exclusive() {
            if error.kind() == std::io::ErrorKind::WouldBlock {
                continue;
            }
            return Err(error.into());
        }
        match mkdirat(
            &recovery,
            "abandoned-verified-originals",
            Mode::from_bits_truncate(0o700),
        ) {
            Ok(()) | Err(Errno::EXIST) => (),
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
        let abandoned = openat(
            &recovery,
            "abandoned-verified-originals",
            flags,
            Mode::empty(),
        )
        .map_err(std::io::Error::from)?;
        let identity = Uuid::new_v4().to_string();
        mkdirat(
            &abandoned,
            identity.as_str(),
            Mode::from_bits_truncate(0o700),
        )
        .map_err(std::io::Error::from)?;
        let destination = openat(&abandoned, identity.as_str(), flags, Mode::empty())
            .map_err(std::io::Error::from)?;
        let identity_fd = openat(
            &destination,
            "staged-name",
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_bits_truncate(0o600),
        )
        .map_err(std::io::Error::from)?;
        let mut identity_file = File::from(identity_fd);
        identity_file.write_all(name.to_bytes())?;
        identity_file.sync_all()?;
        let note_fd = openat(
            &destination,
            "recovery.md",
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_bits_truncate(0o600),
        )
        .map_err(std::io::Error::from)?;
        let mut note = File::from(note_fd);
        note.write_all(b"# Interrupted original copy\n\nIf a `content` file is present, its bytes may include concurrent changes. If it is missing, inspect the remaining staged copies. None of these files are active evidence.\n")?;
        note.sync_all()?;
        before_rename(
            &root
                .join(".recovery/abandoned-verified-originals")
                .join(&identity),
        );
        match renameat_with(
            &staging,
            name,
            &destination,
            "content",
            RenameFlags::NOREPLACE,
        ) {
            Ok(()) => (),
            Err(Errno::NOENT) => continue,
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
        File::from(destination).sync_all()?;
        File::from(abandoned).sync_all()?;
        File::from(staging.try_clone().map_err(std::io::Error::from)?).sync_all()?;
    }
    Ok(true)
}

#[derive(Debug, Serialize, Deserialize)]
struct BundleSeed {
    #[serde(default)]
    generation: String,
    sha256: String,
    bytes: Vec<u8>,
}

pub(super) fn write_bundle_seed(staging: &Path, bytes: &[u8]) -> Result<()> {
    let seed = BundleSeed {
        generation: Uuid::new_v4().to_string(),
        sha256: hex::encode(Sha256::digest(bytes)),
        bytes: bytes.to_vec(),
    };
    write_atomic(&staging.join(".trusted-index"), &serde_json::to_vec(&seed)?)
}

#[derive(Debug, Serialize, Deserialize)]
struct TrustedPage {
    path: PathBuf,
    sha256: String,
    #[serde(default)]
    bundle_generation: Option<String>,
}

pub(super) fn active(root: &Path) -> Result<bool> {
    let state = root.join(".recovery/migration/state.json");
    match fs::symlink_metadata(&state) {
        Ok(metadata) if metadata.is_file() => {
            let value: serde_json::Value = serde_json::from_slice(&fs::read(state)?)?;
            Ok(matches!(
                value.get("phase").and_then(|phase| phase.as_str()),
                Some("cutover_preparing" | "authority_retired")
            ))
        }
        Ok(_) => Err(GardenError::Invalid(
            "Migration state is not a regular file.".into(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn read_active_or_original(path: &Path, max: u64) -> Result<Vec<u8>> {
    for root in path.ancestors().skip(1) {
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let is_knowledge = relative.starts_with("pages")
            && relative
                .extension()
                .is_some_and(|extension| extension == "md");
        let is_source = relative.starts_with("sources")
            && ((relative.file_name().is_some_and(|name| name == "index.md")
                && relative.parent().and_then(Path::parent) == Some(Path::new("sources")))
                || (relative
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == "acquisitions")
                    && relative.parent().and_then(Path::parent).and_then(Path::parent) == Some(Path::new("sources"))
                    && relative
                        .extension()
                        .is_some_and(|extension| extension == "md")));
        if (is_knowledge || is_source)
            && root.join(".recovery/migration/state.json").exists()
            && active(root)?
        {
            let bytes = quarantine_visible(root, relative)?;
            if bytes.len() as u64 > max {
                return Err(GardenError::Invalid(
                    "A trusted page exceeds the reader's size limit.".into(),
                ));
            }
            return Ok(bytes);
        }
    }
    let mut bytes = Vec::new();
    File::open(path)?.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(GardenError::Invalid(
            "A page exceeds the reader's size limit.".into(),
        ));
    }
    Ok(bytes)
}

fn record_path(root: &Path, relative: &Path) -> PathBuf {
    root.join(".recovery/trusted-pages/records")
        .join(hex::encode(Sha256::digest(
            relative.to_string_lossy().as_bytes(),
        )))
}

pub(super) fn records_under(root: &Path, prefix: &Path) -> Result<Vec<PathBuf>> {
    let records = root.join(".recovery/trusted-pages/records");
    let mut result = Vec::new();
    if !records.is_dir() {
        return Ok(result);
    }
    for entry in fs::read_dir(records)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            return Err(GardenError::Invalid(
                "A trusted record is not a regular file.".into(),
            ));
        }
        let record: TrustedPage = serde_json::from_slice(&fs::read(entry.path())?)?;
        validate_relative_collection_path(&record.path)?;
        if record.path.starts_with(prefix) {
            result.push(entry.path());
        }
    }
    Ok(result)
}

fn retain_untracked_page(root: &Path, path: &Path) -> Result<()> {
    let recovery = root.join(".recovery/external-edits");
    validate_real_collection_ancestors(root, &recovery.join("entry"))?;
    fs::create_dir_all(&recovery)?;
    let mut saved = tempfile::NamedTempFile::new_in(&recovery)?;
    let mut input = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
        saved.write_all(&buffer[..count])?;
    }
    saved.as_file().sync_all()?;
    let expected = hex::encode(digest.finalize());
    let destination = recovery.join(&expected);
    match saved.persist_noclobber(&destination) {
        Ok(_) => sync_directory(&recovery)?,
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            if !fs::symlink_metadata(&destination)?.is_file() {
                return Err(GardenError::Invalid(
                    "External save recovery entry is not a regular file.".into(),
                ));
            }
            let mut existing = File::open(&destination)?;
            let mut digest = Sha256::new();
            loop {
                let count = existing.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                digest.update(&buffer[..count]);
            }
            if hex::encode(digest.finalize()) != expected {
                return Err(GardenError::Invalid(
                    "External save recovery entry is corrupt.".into(),
                ));
            }
        }
        Err(error) => return Err(error.error.into()),
    }
    Ok(())
}

pub(super) fn knowledge_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let records = root.join(".recovery/trusted-pages/records");
    let mut result = Vec::new();
    if !records.is_dir() {
        return Ok(result);
    }
    for entry in fs::read_dir(records)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            return Err(GardenError::Invalid(
                "A trusted record is not a regular file.".into(),
            ));
        }
        let record: TrustedPage = serde_json::from_slice(&fs::read(entry.path())?)?;
        validate_relative_collection_path(&record.path)?;
        if record.path.starts_with("pages")
            && record.path.extension().is_some_and(|ext| ext == "md")
        {
            read(root, &record.path)?;
            result.push(root.join(record.path));
        }
    }
    result.sort();
    result.dedup();
    let tracked = result.iter().cloned().collect::<HashSet<_>>();
    let mut directories = vec![root.join("pages")];
    while let Some(directory) = directories.pop() {
        if !directory.is_dir() {
            continue;
        }
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                directories.push(entry.path());
            } else if kind.is_file()
                && entry.path().extension().is_some_and(|ext| ext == "md")
                && !tracked.contains(&entry.path())
            {
                retain_untracked_page(root, &entry.path())?;
            }
        }
    }
    Ok(result)
}

fn source_seed(root: &Path, relative: &Path) -> Result<Option<BundleSeed>> {
    if !relative.starts_with("sources")
        || !relative.file_name().is_some_and(|name| name == "index.md")
    {
        return Ok(None);
    }
    let path = root.join(relative.parent().unwrap()).join(".trusted-index");
    validate_real_collection_ancestors(root, &path)?;
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
        Ok(metadata) if metadata.is_file() => Ok(Some(read_seed(&path)?)),
        Ok(_) => Err(GardenError::Invalid(
            "Source bundle trust seed is not a regular file.".into(),
        )),
    }
}

fn read_seed(path: &Path) -> Result<BundleSeed> {
    let limit = MAX_PAGE_BYTES.saturating_mul(5).saturating_add(8192);
    use rustix::fs::{openat, Mode, OFlags, CWD};
    let directory = path
        .parent()
        .ok_or_else(|| GardenError::Recovery("Imported source trust seed has no parent.".into()))?;
    let parent = openat(
        CWD,
        directory,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)?;
    let name = path
        .file_name()
        .ok_or_else(|| GardenError::Recovery("Imported source trust seed has no name.".into()))?;
    let fd = openat(
        &parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)?;
    let mut file = File::from(fd);
    if !file.metadata()?.is_file() || file.metadata()?.len() > limit {
        return Err(GardenError::Recovery(
            "Imported source trust seed is not bounded.".into(),
        ));
    }
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(GardenError::Recovery(
            "Imported source trust seed is too large.".into(),
        ));
    }
    let seed: BundleSeed = serde_json::from_slice(&bytes)?;
    if seed.sha256 != hex::encode(Sha256::digest(&seed.bytes)) {
        return Err(GardenError::Recovery(
            "Source bundle trust seed failed integrity verification.".into(),
        ));
    }
    Ok(seed)
}
pub(super) fn validate_import_seed(path: &Path, expected: &[u8]) -> Result<()> {
    if read_seed(path)?.bytes != expected {
        return Err(GardenError::Recovery(
            "Imported source trust seed differs from its page.".into(),
        ));
    }
    Ok(())
}

pub(super) fn record_restored_source(
    root: &Path,
    relative: &Path,
    bytes: &[u8],
    staging: &Path,
) -> Result<()> {
    let seed_path = staging.join(".trusted-index");
    let generation = if seed_path.exists() {
        let seed: BundleSeed = serde_json::from_slice(&fs::read(seed_path)?)?;
        if hex::encode(Sha256::digest(&seed.bytes)) != seed.sha256 {
            return Err(GardenError::Invalid(
                "Restored source trust seed is corrupt.".into(),
            ));
        }
        Some(seed.generation)
    } else {
        None
    };
    record_with_generation(root, relative, bytes, generation)
}

pub(super) fn publication_files(
    root: &Path,
    relative: &Path,
    bytes: &[u8],
) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    validate_relative_collection_path(relative)?;
    let digest = hex::encode(Sha256::digest(bytes));
    let entry = TrustedPage {
        path: relative.to_path_buf(),
        sha256: digest.clone(),
        bundle_generation: source_seed(root, relative)?.map(|seed| seed.generation),
    };
    let record = record_path(root, relative);
    validate_real_collection_ancestors(root, &record)?;
    let blob = root.join(".recovery/trusted-pages/blobs").join(&digest);
    validate_real_collection_ancestors(root, &blob)?;
    Ok(vec![
        (blob, bytes.to_vec()),
        (record, serde_json::to_vec(&entry)?),
    ])
}

pub(super) fn record(root: &Path, relative: &Path, bytes: &[u8]) -> Result<()> {
    let generation = source_seed(root, relative)?.map(|seed| seed.generation);
    record_with_generation(root, relative, bytes, generation)
}

fn record_with_generation(
    root: &Path,
    relative: &Path,
    bytes: &[u8],
    generation: Option<String>,
) -> Result<()> {
    validate_relative_collection_path(relative)?;
    let directory = root.join(".recovery/trusted-pages");
    validate_real_collection_ancestors(root, &directory.join("entry"))?;
    let digest = hex::encode(Sha256::digest(bytes));
    let blob = directory.join("blobs").join(&digest);
    match fs::symlink_metadata(&blob) {
        Ok(metadata) if metadata.is_file() && fs::read(&blob)? == bytes => (),
        Ok(_) => return Err(GardenError::Invalid("Trusted page blob is corrupt.".into())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => write_atomic(&blob, bytes)?,
        Err(error) => return Err(error.into()),
    }
    let entry = TrustedPage {
        path: relative.to_path_buf(),
        sha256: digest,
        bundle_generation: generation,
    };
    write_atomic(&record_path(root, relative), &serde_json::to_vec(&entry)?)
}

pub(super) fn read(root: &Path, relative: &Path) -> Result<Vec<u8>> {
    validate_relative_collection_path(relative)?;
    let record = record_path(root, relative);
    validate_real_collection_ancestors(root, &record)?;
    let seed = source_seed(root, relative)?;
    if fs::symlink_metadata(&record)
        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    {
        if let Some(seed) = seed {
            return Ok(seed.bytes);
        }
    }
    if !fs::symlink_metadata(&record)?.is_file() {
        return Err(GardenError::Invalid(
            "Trusted page record is not a regular file.".into(),
        ));
    }
    let entry: TrustedPage = serde_json::from_slice(&fs::read(record)?)?;
    if entry.path != relative
        || entry.sha256.len() != 64
        || !entry
            .sha256
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err(GardenError::Invalid(
            "Trusted page identity is invalid.".into(),
        ));
    }
    if let Some(seed) = seed {
        if entry.bundle_generation.as_deref() != Some(seed.generation.as_str()) {
            let previous = serde_json::to_vec(&entry)?;
            let retained = root
                .join(".recovery/superseded-trust")
                .join(hex::encode(Sha256::digest(&previous)));
            validate_real_collection_ancestors(root, &retained)?;
            if !retained.exists() {
                write_atomic(&retained, &previous)?;
            }
            return Ok(seed.bytes);
        }
    } else if entry.bundle_generation.is_some()
        && relative.starts_with("sources")
        && relative.file_name().is_some_and(|name| name == "index.md")
    {
        return Err(GardenError::Invalid(
            "The trusted source bundle is missing; recovery is required.".into(),
        ));
    }
    let blob = root
        .join(".recovery/trusted-pages/blobs")
        .join(&entry.sha256);
    validate_real_collection_ancestors(root, &blob)?;
    if !fs::symlink_metadata(&blob)?.is_file() {
        return Err(GardenError::Invalid(
            "Trusted page blob is not a regular file.".into(),
        ));
    }
    let bytes = fs::read(blob)?;
    if hex::encode(Sha256::digest(&bytes)) != entry.sha256 {
        return Err(GardenError::Invalid(
            "Trusted page blob failed integrity verification.".into(),
        ));
    }
    Ok(bytes)
}

// Preserve the displaced save separately; active reads use only trusted bytes.
pub(super) fn quarantine_visible(root: &Path, relative: &Path) -> Result<Vec<u8>> {
    quarantine_visible_with_hook(root, relative, || {})
}

fn quarantine_visible_with_hook(
    root: &Path,
    relative: &Path,
    after_baseline_read: impl FnOnce(),
) -> Result<Vec<u8>> {
    let trusted = read(root, relative)?;
    after_baseline_read();
    let visible = root.join(relative);
    validate_real_collection_ancestors(root, &visible)?;
    match fs::symlink_metadata(&visible) {
        Ok(metadata) if metadata.is_file() => {
            let bytes = fs::read(&visible)?;
            if bytes != trusted {
                let digest = hex::encode(Sha256::digest(&bytes));
                let path = root.join(".recovery/external-edits").join(digest);
                validate_real_collection_ancestors(root, &path)?;
                match fs::symlink_metadata(&path) {
                    Ok(metadata) if metadata.is_file() && fs::read(&path)? == bytes => (),
                    Ok(_) => {
                        return Err(GardenError::Invalid(
                            "External edit recovery record is corrupt.".into(),
                        ))
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        write_atomic(&path, &bytes)?
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        }
        Ok(_) => {
            return Err(GardenError::Invalid(
                "Visible page is not a regular file; retained content is unchanged.".into(),
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    Ok(trusted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_read_uses_verified_baseline_and_saves_external_bytes() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        fs::create_dir(root.join("pages")).unwrap();
        let path = Path::new("pages/topic.md");
        let generated = b"generated fact";
        fs::write(root.join(path), generated).unwrap();
        record(root, path, generated).unwrap();
        fs::write(root.join(path), b"external edit").unwrap();
        assert_eq!(quarantine_visible(root, path).unwrap(), generated);
        let saved = root
            .join(".recovery/external-edits")
            .join(hex::encode(Sha256::digest(b"external edit")));
        assert_eq!(fs::read(saved).unwrap(), b"external edit");
        fs::write(root.join(path), b"later edit").unwrap();
        assert_eq!(read(root, path).unwrap(), generated);
        assert_eq!(quarantine_visible(root, path).unwrap(), generated);
        let result = quarantine_visible_with_hook(root, path, || {
            fs::write(root.join(path), b"race edit").unwrap()
        })
        .unwrap();
        assert_eq!(result, generated);
        let race_save = root
            .join(".recovery/external-edits")
            .join(hex::encode(Sha256::digest(b"race edit")));
        assert_eq!(fs::read(race_save).unwrap(), b"race edit");
    }

    #[test]
    fn trusted_records_can_be_published_in_the_same_transaction() {
        let workspace = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(workspace.path()).unwrap();
        fs::create_dir_all(root.join(".staging")).unwrap();
        let root = root.as_path();
        let path = Path::new("pages/topic.md");
        let mut files = vec![(root.join(path), b"generated".to_vec())];
        files.extend(publication_files(root, path, b"generated").unwrap());
        assert!(commit_publication(root, files, &HashMap::new(), Some(1)).is_err());
        recover_publications(root).unwrap();
        assert_eq!(read(root, path).unwrap(), b"generated");
        assert_eq!(fs::read(root.join(path)).unwrap(), b"generated");
    }

    #[test]
    fn active_application_reads_baselines_after_restart_and_retains_external_edits() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let input = workspace.path().join("input.txt");
        fs::write(&input, b"Grounded source.").unwrap();
        let mut app = Application::open(&root).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        let root = fs::canonicalize(root).unwrap();
        fs::create_dir_all(root.join("pages")).unwrap();
        let evidence = serde_json::json!({"quote":"Grounded", "byte_start":0,"byte_end":8,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({
            "schema":1,"page_id":"page-trusted","source_id":source.info.source_id,"title":"Trusted topic","kind":"topic","evidence":evidence
        })).unwrap();
        let markdown = format!(
            "---\n{}---\n\n# Trusted topic\n",
            serde_yaml_ng::to_string(&header).unwrap()
        );
        let page_relative = Path::new("pages/trusted.md");
        fs::write(root.join(page_relative), &markdown).unwrap();
        record(&root, page_relative, markdown.as_bytes()).unwrap();
        let source_path = app.page_path(&source.info.source_id).unwrap();
        let source_relative = source_path.strip_prefix(&root).unwrap();
        record(&root, source_relative, &fs::read(&source_path).unwrap()).unwrap();
        let saved = migration_backup::backup(&root).unwrap();
        let state = serde_json::json!({"backup":saved.file_name().unwrap().to_string_lossy(),"phase":"authority_retired"});
        write_atomic(
            &root.join(".recovery/migration/state.json"),
            &serde_json::to_vec(&state).unwrap(),
        )
        .unwrap();
        drop(app);
        fs::write(root.join(page_relative), b"external false fact").unwrap();
        fs::write(&source_path, b"external fake source").unwrap();
        let app = Application::open(&root).unwrap();
        assert!(app
            .open_knowledge_page("page-trusted")
            .unwrap()
            .markdown
            .contains("Trusted topic"));
        assert_eq!(
            app.open_source(&source.info.source_id)
                .unwrap()
                .info
                .source_id,
            source.info.source_id
        );
        let mut updated = header.clone();
        updated.title = "Updated trusted topic".into();
        let updated_markdown = format!(
            "---\n{}---\n\n# Updated trusted topic\n",
            serde_yaml_ng::to_string(&updated).unwrap()
        );
        fs::write(root.join(page_relative), b"concurrent external save").unwrap();
        let page_path = root.join(page_relative);
        let bases = HashMap::from([(page_path.clone(), markdown.as_bytes().to_vec())]);
        assert!(commit_publication(
            &root,
            vec![(page_path.clone(), updated_markdown.as_bytes().to_vec())],
            &bases,
            Some(1)
        )
        .is_err());
        recover_publications(&root).unwrap();
        assert!(app
            .open_knowledge_page("page-trusted")
            .unwrap()
            .markdown
            .contains("Updated trusted topic"));
        let retained = root.join("owner-edits").join(format!(
            "{}.md",
            hex::encode(Sha256::digest(b"concurrent external save"))
        ));
        assert_eq!(fs::read(retained).unwrap(), b"concurrent external save");
        fs::rename(&page_path, root.join("pages/moved.md")).unwrap();
        assert!(app
            .open_knowledge_page("page-trusted")
            .unwrap()
            .markdown
            .contains("Updated trusted topic"));
        assert!(root.join("pages/moved.md").exists());
        for edit in [
            b"external false fact".as_slice(),
            b"external fake source".as_slice(),
        ] {
            let retained = root
                .join(".recovery/external-edits")
                .join(hex::encode(Sha256::digest(edit)));
            assert_eq!(fs::read(retained).unwrap(), edit);
        }
    }

    #[test]
    fn changed_structured_original_during_extraction_is_not_published() {
        struct TamperingExtractor {
            retained: PathBuf,
        }
        impl SourceExtractor for TamperingExtractor {
            fn extract(
                &self,
                path: &Path,
                _format: &str,
                _title: &str,
            ) -> std::result::Result<OfficeProjection, String> {
                assert_eq!(fs::read(path).unwrap(), b"original document bytes");
                fs::write(&self.retained, b"changed document bytes").unwrap();
                Err("fixture stopped after external save".into())
            }
        }
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let input = workspace.path().join("input.docx");
        let original = b"original document bytes";
        fs::write(&input, original).unwrap();
        let digest = hex::encode(Sha256::digest(original));
        let mut identity = Sha256::new();
        identity.update(b"docx");
        identity.update([0]);
        identity.update(digest.as_bytes());
        let source_id = format!("source-{}", hex::encode(identity.finalize()));
        let retained = root
            .join("sources")
            .join(source_id.strip_prefix("source-").unwrap())
            .join("original.docx");
        let mut app = Application::open_with_providers(
            &root,
            Arc::new(UnavailableProvider),
            Arc::new(TamperingExtractor { retained }),
        )
        .unwrap();
        assert!(app
            .import_source(&input, AcquisitionMethod::Picker)
            .is_err());
        let recovery = root
            .join(".recovery/external-originals")
            .join(hex::encode(Sha256::digest(b"changed document bytes")));
        assert_eq!(fs::read(recovery).unwrap(), b"changed document bytes");
        assert!(app
            .open_source(&source_id)
            .unwrap()
            .knowledge_pages
            .is_empty());
        drop(app);
        let mut app = Application::open(&root).unwrap();
        assert!(app.claim_due_semantic_jobs(1).unwrap().is_empty());
        assert_eq!(
            app.open_source(&source_id).unwrap().info.semantic_state,
            "failed"
        );
        assert!(app
            .open_source(&source_id)
            .unwrap()
            .knowledge_pages
            .is_empty());
    }

    #[test]
    fn changed_audio_during_transcription_does_not_publish_segments() {
        struct TamperingAudio {
            retained: PathBuf,
        }
        impl AudioProcessor for TamperingAudio {
            fn inspect(&self, _path: &Path) -> std::result::Result<AudioInfo, String> {
                Ok(AudioInfo {
                    format: "wav".into(),
                    duration_ms: 1000,
                    sample_rate: 16000.0,
                    channels: 1,
                    codec: "fixture".into(),
                })
            }
            fn install_assets(&self) -> std::result::Result<(), String> {
                Ok(())
            }
            fn transcribe_segment(
                &self,
                path: &Path,
                start_ms: u64,
                duration_ms: u64,
            ) -> std::result::Result<crate::media::TranscriptionBatch, String> {
                assert_eq!(fs::read(path).unwrap(), b"original audio bytes");
                fs::write(&self.retained, b"changed audio bytes").unwrap();
                Ok(crate::media::TranscriptionBatch {
                    schema: 1,
                    requested_start_ms: start_ms,
                    requested_duration_ms: duration_ms,
                    processed_duration_ms: duration_ms,
                    media_duration_ms: 1000,
                    state: "complete".into(),
                    coverage: "complete".into(),
                    detail: "fixture".into(),
                    segments: vec![crate::media::TranscriptSegment {
                        segment_id: String::new(),
                        start_ms: 0,
                        end_ms: 500,
                        text: "unsupported speech".into(),
                        confidence: None,
                        alternatives: Vec::new(),
                        speaker: None,
                        speaker_state: "unidentified".into(),
                        final_result: true,
                    }],
                })
            }
        }
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let input = workspace.path().join("input.wav");
        let bytes = b"original audio bytes";
        fs::write(&input, bytes).unwrap();
        let digest = hex::encode(Sha256::digest(bytes));
        let mut identity = Sha256::new();
        identity.update(b"wav");
        identity.update([0]);
        identity.update(digest.as_bytes());
        let source_id = format!("source-{}", hex::encode(identity.finalize()));
        let retained = root
            .join("sources")
            .join(source_id.strip_prefix("source-").unwrap())
            .join("original.wav");
        let processor = Arc::new(TamperingAudio { retained });
        let mut app = Application::open_with_all_providers(
            &root,
            Arc::new(UnavailableProvider),
            Arc::new(LocalExtractor),
            processor,
        )
        .unwrap();
        app.import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        assert!(app.resume_due_audio_jobs().is_err());
        let source = app.open_source(&source_id).unwrap();
        assert!(source
            .info
            .audio_processing
            .as_ref()
            .unwrap()
            .segments
            .is_empty());
        let recovery = root
            .join(".recovery/external-originals")
            .join(hex::encode(Sha256::digest(b"changed audio bytes")));
        assert_eq!(fs::read(recovery).unwrap(), b"changed audio bytes");
        drop(app);
        let mut app = Application::open(&root).unwrap();
        app.resume_due_audio_jobs().unwrap();
        let restored = app.open_source(&source_id).unwrap();
        assert_eq!(restored.info.semantic_state, "failed");
        let summary = app
            .list_sources(0)
            .unwrap()
            .sources
            .into_iter()
            .find(|item| item.source_id == source_id)
            .unwrap();
        assert_eq!(summary.extraction, restored.info.extraction);
        assert!(restored
            .info
            .semantic_error
            .as_deref()
            .unwrap()
            .contains("changed on disk"));
        assert_eq!(
            restored.info.audio_processing.as_ref().unwrap().state,
            "failed"
        );
        assert!(app
            .open_source(&source_id)
            .unwrap()
            .info
            .audio_processing
            .as_ref()
            .unwrap()
            .segments
            .is_empty());
    }

    #[test]
    fn changed_video_during_frame_processing_publishes_no_audio_or_visual_evidence() {
        struct TamperingVideo {
            retained: PathBuf,
        }
        impl AudioProcessor for TamperingVideo {
            fn inspect_media(&self, _path: &Path) -> std::result::Result<MediaInfo, String> {
                Ok(MediaInfo {
                    format: "mp4".into(),
                    duration_ms: 1000,
                    audio: crate::media::MediaTrack {
                        present: true,
                        decodable: true,
                        sample_rate: Some(16000.0),
                        channels: Some(1),
                        ..Default::default()
                    },
                    video: crate::media::MediaTrack {
                        present: true,
                        decodable: true,
                        width: Some(16),
                        height: Some(16),
                        ..Default::default()
                    },
                })
            }
            fn inspect(&self, _path: &Path) -> std::result::Result<AudioInfo, String> {
                Err("unexpected audio inspection".into())
            }
            fn extract_frame(
                &self,
                path: &Path,
                _at_ms: u64,
                _jpeg: &Path,
            ) -> std::result::Result<crate::media::VideoFrame, String> {
                assert_eq!(fs::read(path).unwrap(), b"original video bytes");
                fs::write(&self.retained, b"changed video bytes").unwrap();
                Err("frame fixture".into())
            }
            fn install_assets(&self) -> std::result::Result<(), String> {
                Ok(())
            }
            fn transcribe_segment(
                &self,
                path: &Path,
                start_ms: u64,
                duration_ms: u64,
            ) -> std::result::Result<crate::media::TranscriptionBatch, String> {
                assert_eq!(fs::read(path).unwrap(), b"original video bytes");
                Ok(crate::media::TranscriptionBatch {
                    schema: 1,
                    requested_start_ms: start_ms,
                    requested_duration_ms: duration_ms,
                    processed_duration_ms: duration_ms,
                    media_duration_ms: 1000,
                    state: "complete".into(),
                    coverage: "complete".into(),
                    detail: "fixture".into(),
                    segments: vec![crate::media::TranscriptSegment {
                        segment_id: String::new(),
                        start_ms: 0,
                        end_ms: 500,
                        text: "unsupported video speech".into(),
                        confidence: None,
                        alternatives: Vec::new(),
                        speaker: None,
                        speaker_state: "unidentified".into(),
                        final_result: true,
                    }],
                })
            }
        }
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let input = workspace.path().join("input.mp4");
        let bytes = b"original video bytes";
        fs::write(&input, bytes).unwrap();
        let digest = hex::encode(Sha256::digest(bytes));
        let mut identity = Sha256::new();
        identity.update(b"mp4");
        identity.update([0]);
        identity.update(digest.as_bytes());
        let source_id = format!("source-{}", hex::encode(identity.finalize()));
        let retained = root
            .join("sources")
            .join(source_id.strip_prefix("source-").unwrap())
            .join("original.mp4");
        let mut app = Application::open_with_all_providers(
            &root,
            Arc::new(UnavailableProvider),
            Arc::new(LocalExtractor),
            Arc::new(TamperingVideo { retained }),
        )
        .unwrap();
        app.import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        assert!(app.resume_due_audio_jobs().is_err());
        let source = app.open_source(&source_id).unwrap();
        let audio = source.info.audio_processing.as_ref().unwrap();
        assert_eq!(audio.state, "failed");
        assert!(audio.segments.is_empty() && audio.frames.is_empty());
        let recovery = root
            .join(".recovery/external-originals")
            .join(hex::encode(Sha256::digest(b"changed video bytes")));
        assert_eq!(fs::read(recovery).unwrap(), b"changed video bytes");
        drop(app);
        let app = Application::open(&root).unwrap();
        let source = app.open_source(&source_id).unwrap();
        let audio = source.info.audio_processing.as_ref().unwrap();
        assert_eq!(audio.state, "failed");
        assert!(audio.segments.is_empty() && audio.frames.is_empty());
    }

    #[test]
    fn large_media_list_and_detail_do_not_copy_original() {
        struct InspectOnly;
        impl AudioProcessor for InspectOnly {
            fn inspect(&self, _path: &Path) -> std::result::Result<AudioInfo, String> {
                Ok(AudioInfo {
                    format: "wav".into(),
                    duration_ms: 1000,
                    sample_rate: 16000.0,
                    channels: 1,
                    codec: "fixture".into(),
                })
            }
            fn install_assets(&self) -> std::result::Result<(), String> {
                Ok(())
            }
            fn transcribe_segment(
                &self,
                _path: &Path,
                _start: u64,
                _duration: u64,
            ) -> std::result::Result<crate::media::TranscriptionBatch, String> {
                Err("not used".into())
            }
        }
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let input = workspace.path().join("large.wav");
        File::create(&input)
            .unwrap()
            .set_len(64 * 1024 * 1024)
            .unwrap();
        let mut app = Application::open_with_all_providers(
            &root,
            Arc::new(UnavailableProvider),
            Arc::new(LocalExtractor),
            Arc::new(InspectOnly),
        )
        .unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        let before = fs::read_dir(root.join(".staging")).unwrap().count();
        let snapshots_before = VERIFIED_SNAPSHOT_COUNT.with(|count| count.get());
        let start = std::time::Instant::now();
        let list = app.list_sources(0).unwrap();
        let detail = app.open_source(&source.info.source_id).unwrap();
        let elapsed = start.elapsed();
        let after = fs::read_dir(root.join(".staging")).unwrap().count();
        assert!(list
            .sources
            .iter()
            .any(|item| item.source_id == source.info.source_id));
        assert_eq!(detail.info.bytes, 64 * 1024 * 1024);
        assert_eq!(before, after);
        assert_eq!(
            VERIFIED_SNAPSHOT_COUNT.with(|count| count.get()),
            snapshots_before
        );
        eprintln!(
            "64 MiB sparse audio fixture list+detail: {:?}, one debug-build run",
            elapsed
        );
    }

    #[test]
    fn new_source_bundle_contains_its_initial_trusted_baseline() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let input = workspace.path().join("new.txt");
        fs::write(&input, b"New source evidence.").unwrap();
        let mut app = Application::open(&root).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        let index = app.page_path(&source.info.source_id).unwrap();
        assert!(index.parent().unwrap().join(".trusted-index").is_file());
        let relative = index.strip_prefix(&app.root).unwrap();
        assert!(!record_path(&app.root, relative).exists());
        fs::write(&index, b"external fake metadata").unwrap();
        assert_eq!(
            app.open_source(&source.info.source_id)
                .unwrap()
                .info
                .source_id,
            source.info.source_id
        );
        drop(app);
        let app = Application::open(&root).unwrap();
        assert_eq!(
            app.open_source(&source.info.source_id)
                .unwrap()
                .info
                .source_id,
            source.info.source_id
        );
        let recovery = root
            .join(".recovery/external-edits")
            .join(hex::encode(Sha256::digest(b"external fake metadata")));
        assert_eq!(fs::read(recovery).unwrap(), b"external fake metadata");
    }

    #[test]
    fn reimport_after_external_bundle_deletion_uses_new_generation_and_retains_old_record() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let input = workspace.path().join("input.txt");
        fs::write(&input, b"Original evidence.").unwrap();
        let mut app = Application::open(&root).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        app.claim_due_semantic_jobs(1).unwrap().remove(0);
        let bundle = app.source_dir(&source.info.source_id).unwrap();
        fs::remove_dir_all(&bundle).unwrap();
        let readded = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        app.claim_due_semantic_jobs(1).unwrap().remove(0);
        assert_eq!(source.info.source_id, readded.info.source_id);
        assert!(app.root.join(".recovery/superseded-trust").is_dir());
        assert!(fs::read_dir(app.root.join(".recovery/superseded-trust"))
            .unwrap()
            .next()
            .is_some());
    }

    #[test]
    fn consumed_intent_without_failed_version_proof_stays_pending() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let input = workspace.path().join("source.txt");
        fs::write(&input, b"Original evidence.").unwrap();
        let mut app = Application::open(&root).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        let version = &source.info.versions_seen[0].source_version_id;
        let name = format!(
            "{}.json",
            hex::encode(Sha256::digest(
                format!("{}\0{}", source.info.source_id, version).as_bytes()
            ))
        );
        let bytes =
            serde_json::to_vec(&(&source.info.source_id, version, "unapplied intent")).unwrap();
        let directory = root.join(".recovery/integrity-exclusions");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(&name), &bytes).unwrap();
        drop(app);
        migration_backup::prepare_pre_recovery(&root).unwrap();
        migration_backup::consume_integrity_intent(&root, &name, &bytes).unwrap();
        let app = Application::open(&root).unwrap();
        assert!(!app.collection_status().complete);
        assert!(app.open_source(&source.info.source_id).is_err());
        assert!(app
            .collection_status()
            .diagnostics
            .iter()
            .any(|item| item.kind == "integrity_recovery_blocked"));
    }

    #[test]
    fn consumed_intent_with_remaining_trusted_support_stays_pending() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let input = workspace.path().join("source.txt");
        fs::write(&input, b"Count was 10.").unwrap();
        let mut app = Application::open(&root).unwrap();
        let imported = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        let mut source = app.open_source(&imported.info.source_id).unwrap();
        let version = source.info.versions_seen[0].source_version_id.clone();
        source.info.versions_seen[0].state = "failed".into();
        app.write_source_page(&mut source).unwrap();
        let evidence = serde_json::json!({"quote":"Count was 10", "byte_start":0,"byte_end":12,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({
            "schema":1,"page_id":"page-proof","source_id":source.info.source_id,"title":"Count","kind":"topic","evidence":evidence,
            "facts":[{"fact_id":"fact-proof","subject_page_id":"page-proof","property":"count","value":"10","qualifier":null,"origin":"source","evidence":evidence,
                "supports":[{"source_id":source.info.source_id,"source_version_id":version,"value":"10","qualifier":null,"origin":"source","evidence":evidence}]}]
        })).unwrap();
        let page = format!(
            "---\n{}---\n\n# Count\n",
            serde_yaml_ng::to_string(&header).unwrap()
        );
        commit_publication(
            &app.root,
            vec![(app.root.join("pages/proof.md"), page.into_bytes())],
            &HashMap::new(),
            None,
        )
        .unwrap();
        let name = format!(
            "{}.json",
            hex::encode(Sha256::digest(
                format!("{}\0{}", source.info.source_id, version).as_bytes()
            ))
        );
        let bytes =
            serde_json::to_vec(&(&source.info.source_id, &version, "unapplied intent")).unwrap();
        let directory = root.join(".recovery/integrity-exclusions");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(&name), &bytes).unwrap();
        drop(app);
        migration_backup::prepare_pre_recovery(&root).unwrap();
        migration_backup::consume_integrity_intent(&root, &name, &bytes).unwrap();
        let app = Application::open(&root).unwrap();
        assert!(!app.collection_status().complete);
        assert!(app.open_knowledge_page("page-proof").is_err());
        assert!(app
            .collection_status()
            .diagnostics
            .iter()
            .any(|item| item.kind == "integrity_recovery_blocked"));
    }

    #[test]
    fn abrupt_integrity_consumption_child() {
        let Some(root) = std::env::var_os("KG_INTEGRITY_CONSUMPTION_ROOT") else {
            return;
        };
        let source_id = std::env::var("KG_INTEGRITY_CONSUMPTION_SOURCE").unwrap();
        let mut app = Application::open(root).unwrap();
        fs::write(app.original_path(&source_id).unwrap(), b"changed original").unwrap();
        let _ = app.verified_original_open_path(&source_id);
        panic!("integrity consumption exit hook did not run");
    }

    #[test]
    fn abrupt_exit_after_integrity_consumption_resumes_from_verified_proof() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let input = workspace.path().join("source.txt");
        fs::write(&input, b"Original evidence.").unwrap();
        let mut app = Application::open(&root).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        drop(app);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::trusted_pages::tests::abrupt_integrity_consumption_child",
            ])
            .env("KG_INTEGRITY_CONSUMPTION_ROOT", &root)
            .env("KG_INTEGRITY_CONSUMPTION_SOURCE", &source.info.source_id)
            .env("KG_STOP_AFTER_INTEGRITY_CONSUMPTION", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(79));
        assert!(root.join(".recovery/migration/pre-recovery.json").is_file());
        assert_eq!(
            fs::read_dir(root.join(".recovery/integrity-exclusions"))
                .unwrap()
                .count(),
            0
        );
        assert_eq!(
            fs::read_dir(root.join(".recovery/integrity-consumed"))
                .unwrap()
                .count(),
            1
        );
        let app = Application::open(&root).unwrap();
        assert!(app.collection_status().complete);
        assert_eq!(
            app.open_source(&source.info.source_id)
                .unwrap()
                .info
                .versions_seen[0]
                .state,
            "failed"
        );
        assert!(!root.join(".recovery/migration/pre-recovery.json").exists());
        let changed = root
            .join(".recovery/external-originals")
            .join(hex::encode(Sha256::digest(b"changed original")));
        assert_eq!(fs::read(changed).unwrap(), b"changed original");
    }

    #[test]
    fn abrupt_original_copy_exit_child() {
        let Some(root) = std::env::var_os("KG_ORIGINAL_COPY_EXIT_FIXTURE") else {
            return;
        };
        let app = Application::open(root).unwrap();
        let bytes = b"abrupt original copy";
        let digest = hex::encode(Sha256::digest(bytes));
        let _ = app.retain_verified_original_with_hook(
            &digest,
            &mut std::io::Cursor::new(bytes),
            || std::process::exit(77),
        );
        panic!("the exit hook did not run");
    }

    #[test]
    fn abrupt_original_copy_exit_recovers_staged_bytes() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        drop(Application::open(&root).unwrap());
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::trusted_pages::tests::abrupt_original_copy_exit_child",
            ])
            .env("KG_ORIGINAL_COPY_EXIT_FIXTURE", &root)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(77));
        let app = Application::open(&root).unwrap();
        assert!(
            fs::read_dir(root.join(".recovery/abandoned-verified-originals"))
                .unwrap()
                .any(
                    |entry| fs::read(entry.unwrap().path().join("content")).unwrap()
                        == b"abrupt original copy"
                )
        );
        assert!(!app.has_pending_integrity_exclusions().unwrap());
        let status = app.collection_status();
        let diagnostic = status
            .diagnostics
            .iter()
            .find(|item| item.kind == "abandoned_original_copies")
            .unwrap();
        assert!(app
            .collection_file_path(&diagnostic.paths[0])
            .unwrap()
            .is_file());
    }

    #[test]
    fn abandoned_original_copy_is_retained_after_restart_and_scan_converges() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let app = Application::open(&root).unwrap();
        let bytes = b"interrupted verified original";
        let digest = hex::encode(Sha256::digest(bytes));
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = app.retain_verified_original_with_hook(
                &digest,
                &mut std::io::Cursor::new(bytes),
                || panic!("simulated crash"),
            );
        }));
        assert!(interrupted.is_err());
        drop(app);
        let mut reopened = Application::open(&root).unwrap();
        let abandoned = root.join(".recovery/abandoned-verified-originals");
        assert!(fs::read_dir(abandoned).unwrap().any(|entry| fs::read(
            entry.unwrap().path().join("content")
        )
        .unwrap()
            == bytes));
        for _ in 0..32 {
            if reopened.reconcile_collection(8).unwrap().complete {
                return;
            }
        }
        panic!("reconciliation did not converge after recovering staged bytes");
    }

    #[test]
    fn pre_migration_staged_copy_is_backed_up_before_recovery_moves_it() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let staging = root.join(".recovery/verified-original-staging");
        fs::create_dir_all(&staging).unwrap();
        fs::write(
            staging.join(".staged-before-migration"),
            b"preserve before migration",
        )
        .unwrap();
        let app = Application::open(&root).unwrap();
        let state: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join(".recovery/migration/state.json")).unwrap())
                .unwrap();
        let backup = root
            .join(".recovery/migration")
            .join(state["backup"].as_str().unwrap());
        assert_eq!(
            fs::read(
                backup.join("files/.recovery/verified-original-staging/.staged-before-migration")
            )
            .unwrap(),
            b"preserve before migration"
        );
        assert!(
            fs::read_dir(root.join(".recovery/abandoned-verified-originals"))
                .unwrap()
                .any(
                    |entry| fs::read(entry.unwrap().path().join("content")).unwrap()
                        == b"preserve before migration"
                )
        );
        assert!(app
            .collection_status()
            .diagnostics
            .iter()
            .any(|item| item.kind == "abandoned_original_copies"));
    }

    #[test]
    fn abandoned_copy_cleanup_preserves_concurrent_destination() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let app = Application::open(&root).unwrap();
        let staging = root.join(".recovery/verified-original-staging");
        fs::create_dir_all(&staging).unwrap();
        let source = staging.join(".staged-concurrent");
        fs::write(&source, b"abandoned bytes").unwrap();
        let mut created = None;
        let result = recover_abandoned_original_copies_with_hook(&root, 2, |destination| {
            let target = destination.join("content");
            fs::write(&target, b"concurrent bytes").unwrap();
            created = Some(target);
        });
        assert!(result.is_err());
        assert_eq!(fs::read(source).unwrap(), b"abandoned bytes");
        assert_eq!(fs::read(created.unwrap()).unwrap(), b"concurrent bytes");
        drop(app);
    }

    #[test]
    fn interrupted_cleanup_folder_is_reported_and_does_not_prevent_idle() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let mut app = Application::open(&root).unwrap();
        let staging = root.join(".recovery/verified-original-staging");
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join(".staged-interrupted"), b"retained bytes").unwrap();
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = recover_abandoned_original_copies_with_hook(&root, 2, |_| {
                panic!("simulated cleanup interruption")
            });
        }));
        assert!(interrupted.is_err());
        for _ in 0..32 {
            if app.reconcile_collection(8).unwrap().complete {
                break;
            }
        }
        assert!(app.collection_status().complete);
        let abandoned = root.join(".recovery/abandoned-verified-originals");
        assert!(fs::read_dir(&abandoned).unwrap().count() >= 2);
        assert!(app
            .collection_status()
            .diagnostics
            .iter()
            .any(|item| item.kind == "abandoned_original_copies"));
        assert_eq!(fs::read_dir(&staging).unwrap().count(), 0);
        let mut idle = false;
        for _ in 0..32 {
            if app.reconcile_collection(8).unwrap().complete {
                idle = true;
                break;
            }
        }
        assert!(idle);
    }

    #[test]
    fn abandoned_original_cleanup_obeys_entry_budget_and_converges() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let app = Application::open(&root).unwrap();
        let staging = root.join(".recovery/verified-original-staging");
        fs::create_dir_all(&staging).unwrap();
        for index in 0..7 {
            fs::write(
                staging.join(format!(".staged-{index}")),
                format!("retained {index}"),
            )
            .unwrap();
        }
        assert!(recover_abandoned_original_copies(&root, 2).unwrap());
        assert_eq!(fs::read_dir(&staging).unwrap().count(), 5);
        for _ in 0..8 {
            if !recover_abandoned_original_copies(&root, 2).unwrap() {
                break;
            }
        }
        assert_eq!(fs::read_dir(&staging).unwrap().count(), 0);
        assert_eq!(
            fs::read_dir(root.join(".recovery/abandoned-verified-originals"))
                .unwrap()
                .count(),
            7
        );
        drop(app);
    }

    #[test]
    fn large_staging_directory_recovers_in_bounded_turns() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let app = Application::open(&root).unwrap();
        let staging = root.join(".recovery/verified-original-staging");
        fs::create_dir_all(&staging).unwrap();
        for index in 0..256 {
            fs::write(
                staging.join(format!(".staged-{index:04}")),
                index.to_string(),
            )
            .unwrap();
        }
        assert!(recover_abandoned_original_copies(&root, 8).unwrap());
        assert_eq!(fs::read_dir(&staging).unwrap().count(), 248);
        for _ in 0..40 {
            if !recover_abandoned_original_copies(&root, 8).unwrap() {
                break;
            }
        }
        assert_eq!(fs::read_dir(&staging).unwrap().count(), 0);
        assert_eq!(
            fs::read_dir(root.join(".recovery/abandoned-verified-originals"))
                .unwrap()
                .count(),
            256
        );
        drop(app);
    }

    #[test]
    fn active_original_copy_is_not_relocated_by_cleanup() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let app = Application::open(&root).unwrap();
        let bytes = b"active verified original";
        let digest = hex::encode(Sha256::digest(bytes));
        app.retain_verified_original_with_hook(&digest, &mut std::io::Cursor::new(bytes), || {
            recover_abandoned_original_copies(&root, 8).unwrap();
        })
        .unwrap();
        assert_eq!(
            fs::read(root.join(".recovery/verified-originals").join(&digest)).unwrap(),
            bytes
        );
        assert!(!root.join(".recovery/abandoned-verified-originals").exists());
    }

    #[test]
    fn version_snapshot_stays_exact_and_recheck_rejects_later_tampering() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let input = workspace.path().join("input.txt");
        fs::write(&input, b"Original evidence.").unwrap();
        let mut app = Application::open(&root).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        let version = source.info.versions_seen[0].clone();
        let snapshot = app
            .verified_version_snapshot(
                &source.info.source_id,
                &version.source_version_id,
                &version.asset,
            )
            .unwrap();
        let snapshot_path = snapshot.path().to_path_buf();
        assert_eq!(fs::read(&snapshot_path).unwrap(), b"Original evidence.");
        fs::write(
            app.original_path(&source.info.source_id).unwrap(),
            b"changed after snapshot",
        )
        .unwrap();
        assert!(app.verify_version_snapshot_current(&snapshot).is_err());
        assert_eq!(fs::read(&snapshot_path).unwrap(), b"Original evidence.");
        let saved = root
            .join(".recovery/external-originals")
            .join(hex::encode(Sha256::digest(b"changed after snapshot")));
        assert_eq!(fs::read(saved).unwrap(), b"changed after snapshot");
        drop(snapshot);
        assert!(!snapshot_path.exists());
    }

    #[test]
    fn concurrent_recovery_copy_creation_never_overwrites_conflicting_bytes() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let app = Application::open(&root).unwrap();
        let expected = b"verified original";
        let digest = hex::encode(Sha256::digest(expected));
        let destination = root.join(".recovery/verified-originals").join(&digest);
        let result = app.retain_verified_original_with_hook(
            &digest,
            &mut std::io::Cursor::new(expected),
            || {
                fs::write(&destination, b"concurrent owner bytes").unwrap();
            },
        );
        assert!(result.is_err());
        assert_eq!(fs::read(destination).unwrap(), b"concurrent owner bytes");
    }

    #[cfg(unix)]
    #[test]
    fn recovery_copy_never_follows_a_swapped_directory() {
        use std::os::unix::fs::symlink;
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let app = Application::open(&root).unwrap();
        let outside = workspace.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let expected = b"verified original";
        let digest = hex::encode(Sha256::digest(expected));
        let target = root.join(".recovery/verified-originals");
        let displaced = root.join(".recovery/displaced-originals");
        let result = app.retain_verified_original_with_hook(
            &digest,
            &mut std::io::Cursor::new(expected),
            || {
                fs::rename(&target, &displaced).unwrap();
                symlink(&outside, &target).unwrap();
            },
        );
        assert!(result.is_err());
        assert!(!outside.join(&digest).exists());
        assert_eq!(fs::read(displaced.join(&digest)).unwrap(), expected);
    }

    #[cfg(unix)]
    #[test]
    fn changed_recovery_copy_is_preserved_as_a_conflict() {
        use std::os::unix::fs::PermissionsExt;
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let input = workspace.path().join("input.txt");
        fs::write(&input, b"Original evidence.").unwrap();
        let mut app = Application::open(&root).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        app.verified_original_open_path(&source.info.source_id)
            .unwrap();
        let retained = root
            .join(".recovery/verified-originals")
            .join(&source.info.sha256);
        fs::set_permissions(&retained, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&retained, b"changed recovery bytes").unwrap();
        assert!(app
            .verified_original_open_path(&source.info.source_id)
            .is_err());
        assert_eq!(fs::read(retained).unwrap(), b"changed recovery bytes");
        assert_eq!(
            fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
            b"Original evidence."
        );
        assert!(!app.has_pending_integrity_exclusions().unwrap());
        assert_ne!(
            app.open_source(&source.info.source_id)
                .unwrap()
                .info
                .versions_seen[0]
                .state,
            "failed"
        );
    }

    #[test]
    fn external_acquisition_edit_does_not_change_provenance() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let first = workspace.path().join("first.txt");
        let second = workspace.path().join("second.txt");
        fs::write(&first, b"same original").unwrap();
        fs::write(&second, b"same original").unwrap();
        let mut app = Application::open(&root).unwrap();
        let source = app
            .import_source(&first, AcquisitionMethod::Picker)
            .unwrap();
        app.import_source(&second, AcquisitionMethod::Picker)
            .unwrap();
        let acquisition_dir = app
            .source_dir(&source.info.source_id)
            .unwrap()
            .join("acquisitions");
        let path = fs::read_dir(acquisition_dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        fs::write(&path, b"external forged provenance").unwrap();
        let page = app.open_source(&source.info.source_id).unwrap();
        assert!(page
            .info
            .acquisitions
            .iter()
            .any(|item| item.path == second.to_string_lossy()));
        assert!(!page
            .info
            .acquisitions
            .iter()
            .any(|item| item.path.contains("forged")));
        drop(app);
        let app = Application::open(&root).unwrap();
        assert!(app
            .open_source(&source.info.source_id)
            .unwrap()
            .info
            .acquisitions
            .iter()
            .any(|item| item.path == second.to_string_lossy()));
    }

    #[test]
    fn open_original_and_version_reject_altered_bytes_and_preserve_them() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let input = workspace.path().join("input.txt");
        fs::write(&input, b"Original evidence.").unwrap();
        let mut app = Application::open(&root).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        let verified = app
            .verified_original_open_path(&source.info.source_id)
            .unwrap();
        assert_eq!(fs::read(&verified).unwrap(), b"Original evidence.");
        let modified = fs::metadata(&verified).unwrap().modified().unwrap();
        let reused = app
            .verified_original_open_path(&source.info.source_id)
            .unwrap();
        assert_eq!(reused, verified);
        assert_eq!(fs::metadata(&reused).unwrap().modified().unwrap(), modified);
        let physical = app.original_path(&source.info.source_id).unwrap();
        fs::write(&physical, b"changed external original").unwrap();
        assert!(app
            .verified_original_open_path(&source.info.source_id)
            .is_err());
        assert!(app
            .verified_original_version_open_path(
                &source.info.source_id,
                &source.info.sha256,
                &source.info.asset
            )
            .is_err());
        let retained = root
            .join(".recovery/external-originals")
            .join(hex::encode(Sha256::digest(b"changed external original")));
        assert_eq!(fs::read(retained).unwrap(), b"changed external original");
        assert_eq!(fs::read(&verified).unwrap(), b"Original evidence.");
        drop(app);
        assert_eq!(fs::read(&verified).unwrap(), b"Original evidence.");
        let mut app = Application::open(&root).unwrap();
        assert!(app
            .search_pages(PageSearchRequest {
                query: "changed external original".into(),
                ..Default::default()
            })
            .unwrap()
            .pages
            .is_empty());
        assert!(app
            .verified_original_open_path(&source.info.source_id)
            .is_err());
        assert_eq!(
            app.open_source(&source.info.source_id)
                .unwrap()
                .info
                .semantic_state,
            "failed"
        );
    }

    #[test]
    fn changed_text_original_is_not_accepted_as_semantic_evidence_after_restart() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join("collection");
        let input = workspace.path().join("input.txt");
        fs::write(&input, b"Original observation.").unwrap();
        let mut app = Application::open(&root).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        let original = app.original_path(&source.info.source_id).unwrap();
        fs::write(&original, b"forged observation").unwrap();
        assert!(app.claim_due_semantic_jobs(1).is_err());
        let retained = root
            .join(".recovery/external-originals")
            .join(hex::encode(Sha256::digest(b"forged observation")));
        assert_eq!(fs::read(retained).unwrap(), b"forged observation");
        let results = app
            .search_pages(PageSearchRequest {
                query: "forged".into(),
                ..Default::default()
            })
            .unwrap();
        assert!(results.pages.is_empty());
        let side = app
            .comparison_side(crate::application::comparison::ComparisonRequest {
                source_id: source.info.source_id.clone(),
                line_start: 1,
                expected_signature: None,
            })
            .unwrap();
        assert_eq!(
            side.status,
            crate::application::comparison::ComparisonStatus::Unavailable
        );
        assert!(side.detail.contains("integrity"));
        assert!(side.passage.is_none());
        drop(app);
        let mut app = Application::open(&root).unwrap();
        assert!(app.claim_due_semantic_jobs(1).unwrap().is_empty());
        assert_eq!(
            app.open_source(&source.info.source_id)
                .unwrap()
                .info
                .semantic_state,
            "failed"
        );
    }

    #[test]
    fn managed_page_removal_removes_trusted_identity_after_recovery() {
        let workspace = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(workspace.path()).unwrap();
        fs::create_dir_all(root.join(".staging")).unwrap();
        let state = serde_json::json!({"phase":"authority_retired"});
        write_atomic(
            &root.join(".recovery/migration/state.json"),
            &serde_json::to_vec(&state).unwrap(),
        )
        .unwrap();
        let relative = Path::new("pages/remove.md");
        commit_publication(
            &root,
            vec![(root.join(relative), b"page".to_vec())],
            &HashMap::new(),
            None,
        )
        .unwrap();
        assert_eq!(knowledge_paths(&root).unwrap().len(), 1);
        commit_publication_with_removals(
            &root,
            Vec::new(),
            vec![root.join(relative)],
            &HashMap::new(),
            None,
        )
        .unwrap();
        recover_publications(&root).unwrap();
        assert!(knowledge_paths(&root).unwrap().is_empty());
        assert!(!root.join(relative).exists());
    }

    #[test]
    fn corrupted_trusted_blob_is_never_used() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        let path = Path::new("pages/topic.md");
        record(root, path, b"generated").unwrap();
        let blob = root
            .join(".recovery/trusted-pages/blobs")
            .join(hex::encode(Sha256::digest(b"generated")));
        fs::write(blob, b"tampered").unwrap();
        assert!(read(root, path).is_err());
    }
}
