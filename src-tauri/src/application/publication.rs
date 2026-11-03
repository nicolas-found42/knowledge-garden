//! A recoverable publication retains both sides of an atomic exchange. The
//! collection lock serializes app writers; external editors need not take it.
use super::*;

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
    #[serde(default)]
    original: Option<OriginalGeneration>,
}

#[derive(Serialize, Deserialize)]
struct OriginalGeneration {
    proposed_digest: String,
}

pub(super) struct OriginalPublication {
    pub destination: PathBuf,
    pub source: PathBuf,
    pub proposed_digest: String,
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
    commit_publication_inner(root, files, Vec::new(), removals, bases, fail_after_files)
}

pub(super) fn commit_publication_with_originals(
    root: &Path,
    files: Vec<(PathBuf, Vec<u8>)>,
    originals: Vec<OriginalPublication>,
    bases: &HashMap<PathBuf, Vec<u8>>,
    fail_after_files: Option<usize>,
) -> Result<()> {
    commit_publication_inner(root, files, originals, Vec::new(), bases, fail_after_files)
}

fn commit_publication_inner(
    root: &Path,
    files: Vec<(PathBuf, Vec<u8>)>,
    originals: Vec<OriginalPublication>,
    removals: Vec<PathBuf>,
    bases: &HashMap<PathBuf, Vec<u8>>,
    fail_after_files: Option<usize>,
) -> Result<()> {
    if files.is_empty() && originals.is_empty() && removals.is_empty() {
        return Ok(());
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
    for original in originals {
        let relative = original.destination.strip_prefix(root).map_err(|_| {
            GardenError::Invalid("A publication path escaped the collection.".into())
        })?;
        validate_relative_collection_path(relative)?;
        let staged = format!("files/{:04}", entries.len());
        write_atomic_from_file(&staging.path().join(&staged), &original.source)?;
        if file_digest(&staging.path().join(&staged))?.as_deref() != Some(&original.proposed_digest)
        {
            return Err(GardenError::Invalid(
                "Candidate original changed before publication.".into(),
            ));
        }
        write_atomic_from_file(
            &staging
                .path()
                .join(format!("files/{:04}.publish", entries.len())),
            &staging.path().join(&staged),
        )?;
        entries.push(PublicationEntry {
            destination: relative.to_string_lossy().into_owned(),
            staged,
            expected: None,
            original: Some(OriginalGeneration {
                proposed_digest: original.proposed_digest,
            }),
        });
    }
    for (destination, bytes) in files {
        let index = entries.len();
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
            original: None,
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

fn apply_publication(
    root: &Path,
    transaction: &Path,
    fail_after_files: Option<usize>,
) -> Result<bool> {
    let manifest: PublicationManifest =
        serde_json::from_slice(&fs::read(transaction.join("manifest.json"))?)?;
    if fail_after_files == Some(0) {
        return interrupted();
    }
    let mut complete = true;
    let removals = manifest.removals;
    for (index, entry) in manifest.entries.into_iter().enumerate() {
        validate_relative_collection_path(Path::new(&entry.destination))?;
        validate_relative_staging_path(Path::new(&entry.staged))?;
        let done = transaction.join(format!("files/{index:04}.done"));
        if let Some(generation) = &entry.original {
            apply_immutable_original(
                root,
                transaction,
                index,
                &entry,
                generation,
                fail_after_files,
            )?;
            continue;
        }
        let proposed = fs::read(transaction.join(&entry.staged))?;
        let Some(destination) = publication_destination(root, &entry, &proposed)? else {
            complete = false;
            continue;
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
            continue;
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
                        desired = merge_owner_save(
                            root,
                            &desired,
                            entry.expected.as_deref(),
                            &displaced,
                        )?;
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
    }
    // A retained publication keeps its removals until every page identity resolves.
    if complete {
        for removal in removals {
            let relative = Path::new(&removal);
            validate_relative_collection_path(relative)?;
            remove_collection_entry(root, relative)?;
        }
    }
    Ok(complete)
}

fn file_digest(path: &Path) -> Result<Option<String>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !fs::symlink_metadata(path)?.file_type().is_file() {
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

fn apply_immutable_original(
    root: &Path,
    transaction: &Path,
    index: usize,
    entry: &PublicationEntry,
    generation: &OriginalGeneration,
    fail: Option<usize>,
) -> Result<()> {
    use rustix::fs::{linkat, AtFlags};
    let relative_destination = Path::new(&entry.destination);
    let destination_dir = open_collection_directory(root, relative_destination.parent().unwrap())?;
    let transaction_relative = transaction
        .strip_prefix(root)
        .map_err(|_| GardenError::Invalid("Transaction escaped collection.".into()))?;
    let staged_parent = transaction_relative.join(Path::new(&entry.staged).parent().unwrap());
    let staged_dir = open_collection_directory(root, &staged_parent)?;
    let staged_name = Path::new(&entry.staged).file_name().unwrap();
    let name = relative_destination.file_name().unwrap();
    let publish_name = format!("{index:04}.publish");
    if digest_at(&staged_dir, staged_name)?.as_deref() != Some(&generation.proposed_digest) {
        return Err(GardenError::Invalid(
            "The staged original no longer matches its generation.".into(),
        ));
    }
    if digest_at(&staged_dir, publish_name.as_ref())?.as_deref()
        != Some(&generation.proposed_digest)
    {
        return Err(GardenError::Invalid(
            "The publish copy no longer matches its generation.".into(),
        ));
    }
    match linkat(
        &staged_dir,
        publish_name.as_str(),
        &destination_dir,
        name,
        AtFlags::empty(),
    ) {
        Ok(()) => destination_dir.sync_all()?,
        Err(rustix::io::Errno::EXIST) => {
            if digest_at(&destination_dir, name)?.as_deref() != Some(&generation.proposed_digest) {
                return Err(GardenError::Invalid(
                    "An original already exists at this generation path; it was not overwritten."
                        .into(),
                ));
            }
        }
        Err(error) => return Err(GardenError::Io(error.into())),
    }
    if fail == Some(usize::MAX) || fail == Some(index + 1) {
        return interrupted();
    }
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
