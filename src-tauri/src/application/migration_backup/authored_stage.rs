//! Per-entry staging of historical owner material from a verified v3 snapshot.
//! Opaque saves stay in the snapshot and are represented by paths, not copied
//! into an ever-growing JSON document.
use super::*;
use std::os::unix::fs::MetadataExt;

#[derive(Serialize, Deserialize)]
struct State {
    after: Option<Vec<u8>>,
    complete: bool,
    database_identity: (u64, u64, u64, i64, i64),
}
const JOURNAL: &str = ".recovery/migration/authored-stage/state.json";
fn database_identity(path: &Path) -> Result<(u64, u64, u64, i64, i64)> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() {
        return Err(GardenError::Recovery(
            "Authored staging manifest is not a regular file.".into(),
        ));
    }
    Ok((m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec()))
}
pub(super) fn begin(saved: &Path, work: &Path) -> Result<()> {
    let path = work.join(JOURNAL);
    validate_real_collection_ancestors(work, &path)?;
    fs::create_dir_all(path.parent().unwrap())?;
    let state = State {
        after: None,
        complete: false,
        database_identity: database_identity(&saved.join("manifest.sqlite"))?,
    };
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    temp.write_all(&serde_json::to_vec(&state)?)?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(&path)
        .map_err(|e| GardenError::Io(e.error))?;
    sync_directory(path.parent().unwrap())
}
fn material(files: &Path, relative: &Path) -> Result<AuthoredMaterial> {
    let mut file = bounded_copy::source_file(files, relative)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(GardenError::Recovery(
            "Authored snapshot page changed type.".into(),
        ));
    }
    if metadata.len() > MAX_PAGE_BYTES {
        return Ok(AuthoredMaterial {
            page_id: format!("unparsed:{}", relative.display()),
            corrections: Vec::new(),
            prose: Vec::new(),
            opaque_saves: vec![OpaqueOwnerSave {
                path: relative.to_string_lossy().into_owned(),
                bytes: Vec::new(),
            }],
        });
    }
    let mut raw = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(MAX_PAGE_BYTES + 1)
        .read_to_end(&mut raw)?;
    if raw.len() as u64 > MAX_PAGE_BYTES {
        return Err(GardenError::Recovery(
            "Authored snapshot page grew during staging.".into(),
        ));
    }
    let parsed = std::str::from_utf8(&raw).ok().and_then(|text| {
        parse_knowledge_header(text)
            .ok()
            .map(|header| (text, header))
    });
    let Some((markdown, mut header)) = parsed else {
        return Ok(AuthoredMaterial {
            page_id: format!("unparsed:{}", relative.display()),
            corrections: Vec::new(),
            prose: Vec::new(),
            opaque_saves: vec![OpaqueOwnerSave {
                path: relative.to_string_lossy().into_owned(),
                bytes: Vec::new(),
            }],
        });
    };
    capture_snapshot_fact_edits(&mut header, markdown);
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
    for value in &header.uninterpreted_owner_edits {
        let path = Path::new(value);
        validate_relative_collection_path(path)?;
        if path.components().next() != Some(std::path::Component::Normal("owner-edits".as_ref())) {
            return Err(GardenError::Recovery(
                "An owner save points outside retained owner edits.".into(),
            ));
        }
        if !bounded_copy::source_file(files, path)?
            .metadata()?
            .is_file()
        {
            return Err(GardenError::Recovery(
                "An owner save is not a regular file.".into(),
            ));
        }
        opaque_saves.push(OpaqueOwnerSave {
            path: value.clone(),
            bytes: Vec::new(),
        });
    }
    opaque_saves.push(OpaqueOwnerSave {
        path: relative.to_string_lossy().into_owned(),
        bytes: Vec::new(),
    });
    Ok(AuthoredMaterial {
        page_id: header.page_id,
        corrections,
        prose: header.owner_content,
        opaque_saves,
    })
}
pub(super) fn publish_once(root: &Path, relative: &Path, bytes: &[u8]) -> Result<()> {
    validate_relative_collection_path(relative)?;
    let path = root.join(relative);
    validate_real_collection_ancestors(root, &path)?;
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent)?;
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut temp = tempfile::NamedTempFile::new_in(parent)?;
            temp.write_all(bytes)?;
            temp.as_file().sync_all()?;
            temp.persist_noclobber(&path)
                .map_err(|e| GardenError::Io(e.error))?;
            sync_directory(parent)
        }
        Ok(m) if m.is_file() => {
            let mut actual = Vec::new();
            bounded_copy::source_file(root, relative)?
                .take((bytes.len() + 1) as u64)
                .read_to_end(&mut actual)?;
            if actual != bytes {
                return Err(GardenError::Recovery(
                    "Authored staging conflicts with retained work.".into(),
                ));
            }
            Ok(())
        }
        Ok(_) => Err(GardenError::Recovery(
            "Authored staging entry is not a regular file.".into(),
        )),
        Err(e) => Err(e.into()),
    }
}
fn stage_database(work: &Path, name: &str) -> Result<PathBuf> {
    if !["ignored.sqlite", "intents.sqlite", "labels.sqlite"].contains(&name) {
        return Err(GardenError::Recovery(
            "Invalid staging database name.".into(),
        ));
    }
    let path = work.join(".recovery/migration/authored-stage").join(name);
    validate_real_collection_ancestors(work, &path)?;
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent)?;
    match fs::symlink_metadata(&path) {
        Ok(m) if m.is_file() => return Ok(path),
        Ok(_) => {
            return Err(GardenError::Recovery(
                "Ignored-source manifest is not a regular file.".into(),
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    }
    let temp = tempfile::tempdir_in(parent)?;
    let database = temp.path().join(name);
    if name == "labels.sqlite" {
        label_catalog::create(&database)?;
    } else {
        manifest_records::create(&database)?;
    }
    use rustix::fs::{renameat_with, RenameFlags, CWD};
    match renameat_with(CWD, &database, CWD, &path, RenameFlags::NOREPLACE) {
        Ok(()) => sync_directory(parent)?,
        Err(rustix::io::Errno::EXIST) => {
            return Err(GardenError::Recovery(
                "Ignored-source manifest appeared concurrently.".into(),
            ))
        }
        Err(e) => return Err(std::io::Error::from(e).into()),
    }
    Ok(path)
}

fn stage_label(files: &Path, work: &Path, path: &Path, expected: &str) -> Result<()> {
    let file = bounded_copy::source_file(files, path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_PAGE_BYTES {
        return Ok(());
    }
    let mut raw = Vec::new();
    file.take(MAX_PAGE_BYTES + 1).read_to_end(&mut raw)?;
    if raw.len() as u64 > MAX_PAGE_BYTES || manifest_cursor::digest_file(files, path)? != expected {
        return Err(GardenError::Recovery(
            "Snapshot page changed while building labels.".into(),
        ));
    }
    let Some(header) = std::str::from_utf8(&raw)
        .ok()
        .and_then(|text| parse_knowledge_header(text).ok())
    else {
        return Ok(());
    };
    label_catalog::put(
        &stage_database(work, "labels.sqlite")?,
        &header.page_id,
        &header.title,
    )
}

fn plan_source(saved: &Path, work: &Path, item: &AuthoredMaterial) -> Result<()> {
    if item.corrections.is_empty() && item.prose.is_empty() {
        return Ok(());
    }
    let bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "qualification": "Historical owner-authored statements retained during migration. These statements are not verified and may be outdated.",
        "legacy_page_id": item.page_id, "corrections": item.corrections, "owner_prose": item.prose,
    }))?;
    let digest = hex::encode(Sha256::digest(&bytes));
    let source_file = PathBuf::from("authored-sources").join(format!("{digest}.txt"));
    publish_once(saved, &source_file, &bytes)?;
    let scope = format!("migration-authored:{digest}");
    let source_id = format!(
        "source-{}",
        hex::encode(Sha256::digest(
            format!("repository-source:txt:{scope}").as_bytes()
        ))
    );
    let intent = AuthoredImportIntent {
        source_id: source_id.clone(),
        source_file,
        expected_original_sha256: digest,
        expected_bundle_sha256: None,
    };
    let path = PathBuf::from(".recovery/migration/authored-stage/intents")
        .join(format!("{source_id}.json"));
    publish_once(work, &path, &serde_json::to_vec(&intent)?)?;
    let digest = source_id
        .strip_prefix("source-")
        .ok_or_else(|| GardenError::Recovery("Authored source identity is invalid.".into()))?;
    manifest_records::put(
        &stage_database(work, "ignored.sqlite")?,
        &PathBuf::from("sources").join(digest),
        None,
    )?;
    manifest_records::put(
        &stage_database(work, "intents.sqlite")?,
        Path::new(&source_id),
        Some(&intent.expected_original_sha256),
    )
}

/// Import one already-planned source after the caller has verified the backup
/// and unrelated live files. A missing receipt is reconciled by validating the
/// complete deterministic bundle, so a crash after publication is idempotent.
#[cfg(test)]
pub(super) fn import_one(
    app: &mut Application,
    saved: &Path,
    work: &Path,
    source_id: &str,
) -> Result<()> {
    import_or_verify(app, saved, work, work, source_id, true, true)
}
pub(super) fn import_one_with_receipts(
    app: &mut Application,
    saved: &Path,
    intent_work: &Path,
    receipt_work: &Path,
    source_id: &str,
) -> Result<()> {
    import_or_verify(app, saved, intent_work, receipt_work, source_id, true, true)
}
pub(super) fn verify_receipt(
    app: &mut Application,
    saved: &Path,
    intent_work: &Path,
    receipt_work: &Path,
    source_id: &str,
) -> Result<()> {
    import_or_verify(
        app,
        saved,
        intent_work,
        receipt_work,
        source_id,
        false,
        false,
    )
}
pub(super) fn recover_existing(
    app: &mut Application,
    saved: &Path,
    intent_work: &Path,
    receipt_work: &Path,
    source_id: &str,
) -> Result<()> {
    import_or_verify(
        app,
        saved,
        intent_work,
        receipt_work,
        source_id,
        false,
        true,
    )
}
fn import_or_verify(
    app: &mut Application,
    saved: &Path,
    work: &Path,
    receipt_work: &Path,
    source_id: &str,
    allow_import: bool,
    create_receipt: bool,
) -> Result<()> {
    let digest = source_id
        .strip_prefix("source-")
        .filter(|id| {
            id.len() == 64
                && id
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        })
        .ok_or_else(|| GardenError::Recovery("Authored source identity is invalid.".into()))?;
    let relative = PathBuf::from(".recovery/migration/authored-stage/intents")
        .join(format!("{source_id}.json"));
    let mut bytes = Vec::new();
    bounded_copy::source_file(work, &relative)?
        .take(16385)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 16384 {
        return Err(GardenError::Recovery(
            "Authored import intent is too large.".into(),
        ));
    }
    let intent: AuthoredImportIntent = serde_json::from_slice(&bytes)?;
    if intent.source_id != source_id || intent.expected_bundle_sha256.is_some() {
        return Err(GardenError::Recovery(
            "Authored import intent is invalid.".into(),
        ));
    }
    validate_relative_collection_path(&intent.source_file)?;
    if intent.source_file
        != PathBuf::from("authored-sources")
            .join(format!("{}.txt", intent.expected_original_sha256))
    {
        return Err(GardenError::Recovery(
            "Authored import source path is invalid.".into(),
        ));
    }
    let source = saved.join(&intent.source_file);
    let original = bounded_copy::source_file(saved, &intent.source_file)?;
    if !original.metadata()?.is_file() || original.metadata()?.len() > MAX_PAGE_BYTES {
        return Err(GardenError::Recovery(
            "Authored import source is not a bounded regular file.".into(),
        ));
    }
    if hex::encode(digest_file(&source)?) != intent.expected_original_sha256 {
        return Err(GardenError::Recovery(
            "Authored import source changed.".into(),
        ));
    }
    let expected = hex::encode(Sha256::digest(
        format!(
            "repository-source:txt:migration-authored:{}",
            intent.expected_original_sha256
        )
        .as_bytes(),
    ));
    if expected != digest {
        return Err(GardenError::Recovery(
            "Authored import identity does not match its content.".into(),
        ));
    }
    let directory = app.source_dir(source_id)?;
    let receipt = PathBuf::from(".recovery/migration/authored-stage/receipts")
        .join(format!("{source_id}.json"));
    let has_receipt = match fs::symlink_metadata(receipt_work.join(&receipt)) {
        Ok(m) if m.is_file() => true,
        Ok(_) => {
            return Err(GardenError::Recovery(
                "Authored import receipt is not a regular file.".into(),
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(e.into()),
    };
    if !directory.exists() {
        if has_receipt {
            return Err(GardenError::Recovery(
                "A receipted authored source is missing; recovery will not recreate it.".into(),
            ));
        }
        if !allow_import {
            return Err(GardenError::Recovery(
                "A receipted authored source is missing.".into(),
            ));
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
        if imported.info.source_id != source_id
            || imported.info.sha256 != intent.expected_original_sha256
        {
            return Err(GardenError::Recovery(
                "Published authored source differs from its intent.".into(),
            ));
        }
        #[cfg(test)]
        if std::env::var_os("KG_AUTHORED_STOP_AFTER_IMPORT").is_some() {
            std::process::exit(93);
        }
    }
    // Bound the subsequent legacy bundle validator before it enumerates or
    // hashes the directory; unexpected extra members fail closed.
    let mut entries = fs::read_dir(&directory)?;
    let mut count = 0;
    while entries.next().transpose()?.is_some() {
        count += 1;
        if count > 3 {
            return Err(GardenError::Recovery(
                "Authored source bundle has unexpected entries.".into(),
            ));
        }
    }
    for name in ["index.md", "original.txt"] {
        let file = bounded_copy::source_file(&directory, Path::new(name))?;
        if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_PAGE_BYTES {
            return Err(GardenError::Recovery(
                "Authored source bundle member is not bounded.".into(),
            ));
        }
    }
    validate_unrecorded_import(&directory, &source, &intent)
        .map_err(|e| GardenError::Recovery(format!("Validating authored import: {e}")))?;
    let receipt_bytes = serde_json::to_vec(
        &bundle_digest(&directory)
            .map_err(|e| GardenError::Recovery(format!("Digesting authored import: {e}")))?,
    )?;
    if create_receipt {
        return publish_once(receipt_work, &receipt, &receipt_bytes)
            .map_err(|e| GardenError::Recovery(format!("Publishing authored receipt: {e}")));
    }
    let mut actual = Vec::new();
    bounded_copy::source_file(receipt_work, &receipt)?
        .take(4097)
        .read_to_end(&mut actual)?;
    if actual != receipt_bytes {
        return Err(GardenError::Recovery(
            "Authored import receipt or source bundle changed.".into(),
        ));
    }
    Ok(())
}

/// Visit one manifest row and publish at most one immutable material record.
pub(super) fn step(saved: &Path, work: &Path) -> Result<bool> {
    let mut bytes = Vec::new();
    bounded_copy::source_file(work, Path::new(JOURNAL))?
        .take(16385)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 16384 {
        return Err(GardenError::Recovery(
            "Authored staging cursor is too large.".into(),
        ));
    }
    let mut state: State = serde_json::from_slice(&bytes)?;
    let db = saved.join("manifest.sqlite");
    if database_identity(&db)? != state.database_identity {
        return Err(GardenError::Recovery(
            "Authored staging manifest changed.".into(),
        ));
    }
    if state.complete {
        return Ok(true);
    }
    let Some((key, entry)) = manifest_records::next(&db, state.after.as_deref())? else {
        stage_database(work, "ignored.sqlite")?;
        stage_database(work, "intents.sqlite")?;
        stage_database(work, "labels.sqlite")?;
        state.complete = true;
        write_atomic(&work.join(JOURNAL), &serde_json::to_vec(&state)?)?;
        return Ok(true);
    };
    if entry.digest.is_some()
        && entry.path.starts_with("pages")
        && entry.path.extension().is_some_and(|ext| ext == "md")
    {
        let files = saved.join("files");
        let item = material(&files, &entry.path)?;
        let source = bounded_copy::source_file(&files, &entry.path)?;
        if source.metadata()?.len() <= MAX_PAGE_BYTES {
            let actual = manifest_cursor::digest_file(&files, &entry.path)?;
            if entry.digest.as_deref() != Some(actual.as_str()) {
                return Err(GardenError::Recovery(
                    "Authored snapshot page failed manifest verification.".into(),
                ));
            }
        }
        stage_label(&files, work, &entry.path, entry.digest.as_deref().unwrap())?;
        plan_source(saved, work, &item)?;
        let record = serde_json::to_vec(&item)?;
        let directory = work.join(".recovery/migration/authored-stage/records");
        fs::create_dir_all(&directory)?;
        let path = directory.join(format!("{}.json", hex::encode(Sha256::digest(&key))));
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut temp = tempfile::NamedTempFile::new_in(&directory)?;
                temp.write_all(&record)?;
                temp.as_file().sync_all()?;
                temp.persist_noclobber(&path)
                    .map_err(|e| GardenError::Io(e.error))?;
                sync_directory(&directory)?;
            }
            Ok(m) if m.is_file() => {
                let mut actual = Vec::new();
                bounded_copy::source_file(
                    work,
                    path.strip_prefix(work).map_err(|_| {
                        GardenError::Recovery("Authored record escaped recovery.".into())
                    })?,
                )?
                .take((record.len() + 1) as u64)
                .read_to_end(&mut actual)?;
                if actual != record {
                    return Err(GardenError::Recovery(
                        "Staged authored record conflicts with prior work.".into(),
                    ));
                }
            }
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Staged authored record conflicts with prior work.".into(),
                ))
            }
            Err(e) => return Err(e.into()),
        }
    }
    #[cfg(test)]
    if std::env::var_os("KG_AUTHORED_STAGE_STOP_AFTER_RECORD").is_some()
        && entry.digest.is_some()
        && entry.path.starts_with("pages")
        && entry.path.extension().is_some_and(|ext| ext == "md")
    {
        std::process::exit(92);
    }
    state.after = Some(key);
    write_atomic(&work.join(JOURNAL), &serde_json::to_vec(&state)?)?;
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stages_owner_prose_as_a_stable_source_intent() {
        let temp = tempfile::tempdir().unwrap();
        let saved = temp.path().join("saved");
        let work = temp.path().join("work");
        fs::create_dir_all(saved.join("files/pages")).unwrap();
        fs::create_dir(&work).unwrap();
        let evidence = serde_json::json!({"quote":"source", "byte_start":0,"byte_end":6,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({
            "schema":1,"page_id":"page-1","source_id":"source-1","title":"Page","kind":"event","evidence":evidence,
            "facts":[],"owner_content":["owner's exact prose"],"uninterpreted_owner_edits":[]
        })).unwrap();
        fs::write(
            saved.join("files/pages/page.md"),
            format!("---\n{}---\n", serde_yaml_ng::to_string(&header).unwrap()),
        )
        .unwrap();
        let db = saved.join("manifest.sqlite");
        manifest_records::create(&db).unwrap();
        manifest_records::put(&db, Path::new("pages"), None).unwrap();
        let digest =
            manifest_cursor::digest_file(&saved.join("files"), Path::new("pages/page.md")).unwrap();
        manifest_records::put(&db, Path::new("pages/page.md"), Some(&digest)).unwrap();
        begin(&saved, &work).unwrap();
        for _ in 0..20 {
            if step(&saved, &work).unwrap() {
                break;
            }
        }
        let intents = fs::read_dir(work.join(".recovery/migration/authored-stage/intents"))
            .unwrap()
            .collect::<std::io::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(intents.len(), 1);
        let intent: AuthoredImportIntent =
            serde_json::from_slice(&fs::read(intents[0].path()).unwrap()).unwrap();
        let source = fs::read_to_string(saved.join(&intent.source_file)).unwrap();
        assert!(source.contains("owner's exact prose"));
        assert_eq!(
            hex::encode(Sha256::digest(source.as_bytes())),
            intent.expected_original_sha256
        );
    }
    #[test]
    fn owner_save_symlink_is_not_followed() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let saved = temp.path().join("saved");
        let work = temp.path().join("work");
        fs::create_dir_all(saved.join("files/pages")).unwrap();
        fs::create_dir_all(saved.join("files/owner-edits")).unwrap();
        fs::create_dir(&work).unwrap();
        let outside = temp.path().join("outside");
        fs::write(&outside, b"outside owner data").unwrap();
        symlink(&outside, saved.join("files/owner-edits/save.md")).unwrap();
        let evidence = serde_json::json!({"quote":"source", "byte_start":0,"byte_end":6,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({"schema":1,"page_id":"page-1","source_id":"source-1","title":"Page","kind":"event","evidence":evidence,"facts":[],"owner_content":["owner prose"],"uninterpreted_owner_edits":["owner-edits/save.md"]})).unwrap();
        fs::write(
            saved.join("files/pages/page.md"),
            format!("---\n{}---\n", serde_yaml_ng::to_string(&header).unwrap()),
        )
        .unwrap();
        let db = saved.join("manifest.sqlite");
        manifest_records::create(&db).unwrap();
        let digest =
            manifest_cursor::digest_file(&saved.join("files"), Path::new("pages/page.md")).unwrap();
        manifest_records::put(&db, Path::new("pages/page.md"), Some(&digest)).unwrap();
        begin(&saved, &work).unwrap();
        assert!(step(&saved, &work).is_err());
        assert_eq!(fs::read(outside).unwrap(), b"outside owner data");
    }
    #[test]
    fn import_crash_child() {
        let Some(root) = std::env::var_os("KG_AUTHORED_IMPORT_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let id = std::env::var("KG_AUTHORED_IMPORT_ID").unwrap();
        let mut app = Application::open(root.join("collection")).unwrap();
        import_one(&mut app, &root.join("saved"), &root.join("work"), &id).unwrap();
        panic!("import exit hook did not run");
    }
    #[test]
    fn published_import_without_receipt_is_validated_and_recorded_on_restart() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let saved = root.join("saved");
        let work = root.join("work");
        fs::create_dir(&saved).unwrap();
        fs::create_dir(&work).unwrap();
        let item = AuthoredMaterial {
            page_id: "historical-page".into(),
            corrections: Vec::new(),
            prose: vec!["owner statement".into()],
            opaque_saves: Vec::new(),
        };
        plan_source(&saved, &work, &item).unwrap();
        let intents = fs::read_dir(work.join(".recovery/migration/authored-stage/intents"))
            .unwrap()
            .collect::<std::io::Result<Vec<_>>>()
            .unwrap();
        let intent: AuthoredImportIntent =
            serde_json::from_slice(&fs::read(intents[0].path()).unwrap()).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::authored_stage::tests::import_crash_child",
            ])
            .env("KG_AUTHORED_IMPORT_ROOT", root)
            .env("KG_AUTHORED_IMPORT_ID", &intent.source_id)
            .env("KG_AUTHORED_STOP_AFTER_IMPORT", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(93));
        let mut app = Application::open(root.join("collection")).unwrap();
        import_one(&mut app, &saved, &work, &intent.source_id).unwrap();
        import_one(&mut app, &saved, &work, &intent.source_id).unwrap();
        assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
        let receipt = work
            .join(".recovery/migration/authored-stage/receipts")
            .join(format!("{}.json", intent.source_id));
        assert!(receipt.is_file());
        verify_receipt(&mut app, &saved, &work, &work, &intent.source_id).unwrap();
        fs::write(&receipt, b"forged receipt").unwrap();
        assert!(verify_receipt(&mut app, &saved, &work, &work, &intent.source_id).is_err());
        let directory = app.source_dir(&intent.source_id).unwrap();
        fs::rename(&directory, root.join("displaced-bundle")).unwrap();
        assert!(import_one(&mut app, &saved, &work, &intent.source_id).is_err());
        assert!(!directory.exists());
    }
    #[test]
    fn crash_child() {
        let Some(root) = std::env::var_os("KG_AUTHORED_STAGE_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let saved = root.join("saved");
        let work = root.join("work");
        begin(&saved, &work).unwrap();
        loop {
            if step(&saved, &work).unwrap() {
                panic!("hook did not run");
            }
        }
    }
    #[test]
    fn interrupted_entry_is_replayed_and_opaque_bytes_remain_in_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let saved = root.join("saved");
        let work = root.join("work");
        fs::create_dir_all(saved.join("files/pages")).unwrap();
        fs::create_dir(&work).unwrap();
        fs::write(
            saved.join("files/pages/old.md"),
            b"invalid historical page with exact owner bytes",
        )
        .unwrap();
        let db = saved.join("manifest.sqlite");
        manifest_records::create(&db).unwrap();
        manifest_records::put(&db, Path::new("pages"), None).unwrap();
        let digest =
            manifest_cursor::digest_file(&saved.join("files"), Path::new("pages/old.md")).unwrap();
        manifest_records::put(&db, Path::new("pages/old.md"), Some(&digest)).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::authored_stage::tests::crash_child",
            ])
            .env("KG_AUTHORED_STAGE_ROOT", root)
            .env("KG_AUTHORED_STAGE_STOP_AFTER_RECORD", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(92));
        for _ in 0..20 {
            if step(&saved, &work).unwrap() {
                break;
            }
        }
        let records = fs::read_dir(work.join(".recovery/migration/authored-stage/records"))
            .unwrap()
            .collect::<std::io::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(records.len(), 1);
        let saved_material: AuthoredMaterial =
            serde_json::from_slice(&fs::read(records[0].path()).unwrap()).unwrap();
        assert_eq!(saved_material.opaque_saves[0].path, "pages/old.md");
        assert!(saved_material.opaque_saves[0].bytes.is_empty());
        assert_eq!(
            fs::read(saved.join("files/pages/old.md")).unwrap(),
            b"invalid historical page with exact owner bytes"
        );
    }
}
