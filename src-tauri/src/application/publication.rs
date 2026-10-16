//! A recoverable publication retains both sides of an atomic exchange. The
//! collection lock serializes app writers; external editors need not take it.
use super::*;

#[derive(Serialize, Deserialize)]
struct PublicationManifest {
    entries: Vec<PublicationEntry>,
}
#[derive(Serialize, Deserialize)]
struct PublicationEntry {
    destination: String,
    staged: String,
    #[serde(default)]
    expected: Option<Vec<u8>>,
}

pub(super) fn commit_publication(
    root: &Path,
    files: Vec<(PathBuf, Vec<u8>)>,
    bases: &HashMap<PathBuf, Vec<u8>>,
    fail_after_files: Option<usize>,
) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }
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
        &serde_json::to_vec(&PublicationManifest { entries })?,
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
    for (index, entry) in manifest.entries.into_iter().enumerate() {
        validate_relative_collection_path(Path::new(&entry.destination))?;
        validate_relative_staging_path(Path::new(&entry.staged))?;
        let done = transaction.join(format!("files/{index:04}.done"));
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
    Ok(complete)
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
    let path = format!("owner-edits/{:x}.md", Sha256::digest(bytes));
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
