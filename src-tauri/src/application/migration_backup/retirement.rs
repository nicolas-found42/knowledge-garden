//! Per-page retirement of historical manual authority. Output is derived again
//! from the verified snapshot before accepting a prior publication receipt.
use super::*;

#[derive(Serialize, Deserialize)]
struct Receipt {
    path: PathBuf,
    snapshot_sha256: String,
    output_sha256: String,
}

pub(super) fn snapshot_bytes(saved: &Path, relative: &Path) -> Result<Option<Vec<u8>>> {
    let entry = manifest_records::get(&saved.join("manifest.sqlite"), relative)?
        .ok_or_else(|| GardenError::Recovery("Retirement snapshot entry is missing.".into()))?;
    let expected = entry
        .digest
        .ok_or_else(|| GardenError::Recovery("Retirement snapshot entry is not a file.".into()))?;
    let files = saved.join("files");
    let file = bounded_copy::source_file(&files, relative)?;
    if !file.metadata()?.is_file() {
        return Err(GardenError::Recovery(
            "Retirement snapshot entry changed type.".into(),
        ));
    }
    if file.metadata()?.len() > MAX_PAGE_BYTES {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    file.take(MAX_PAGE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PAGE_BYTES
        || manifest_cursor::digest_file(&files, relative)? != expected
    {
        return Err(GardenError::Recovery(
            "Retirement snapshot entry changed.".into(),
        ));
    }
    Ok(Some(bytes))
}

fn derive_page(
    app: &mut Application,
    saved: &Path,
    relative: &Path,
    labels_db: Option<&Path>,
) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
    validate_relative_collection_path(relative)?;
    if !relative.starts_with("pages") || relative.extension().is_none_or(|ext| ext != "md") {
        return Err(GardenError::Recovery(
            "Retirement path is not a knowledge page.".into(),
        ));
    }
    let entry =
        manifest_records::get(&saved.join("manifest.sqlite"), relative)?.ok_or_else(|| {
            GardenError::Recovery("Retirement page is absent from the snapshot manifest.".into())
        })?;
    let expected = entry
        .digest
        .ok_or_else(|| GardenError::Recovery("Retirement page is not a file.".into()))?;
    let files = saved.join("files");
    let snapshot = bounded_copy::source_file(&files, relative)?;
    let metadata = snapshot.metadata()?;
    if !metadata.is_file() {
        return Err(GardenError::Recovery(
            "Retirement page changed type.".into(),
        ));
    }
    if metadata.len() > MAX_PAGE_BYTES {
        return Ok(None);
    }
    let mut raw = Vec::new();
    snapshot.take(MAX_PAGE_BYTES + 1).read_to_end(&mut raw)?;
    if raw.len() as u64 > MAX_PAGE_BYTES
        || manifest_cursor::digest_file(&files, relative)? != expected
    {
        return Err(GardenError::Recovery(
            "Retirement snapshot page changed.".into(),
        ));
    }
    let Ok(text) = std::str::from_utf8(&raw) else {
        return Ok(None);
    };
    let Ok(mut header) = parse_knowledge_header(text) else {
        return Ok(None);
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
    let mut labels = HashMap::new();
    for relationship in &header.relationships {
        for id in [&relationship.from_page_id, &relationship.to_page_id] {
            let database = labels_db.ok_or_else(|| {
                GardenError::Recovery("Retirement label catalog is missing.".into())
            })?;
            if !labels.contains_key(id) {
                if let Some(title) = label_catalog::get(database, id)? {
                    labels.insert(id.clone(), title);
                }
            }
        }
    }
    let desired = app
        .render_knowledge_page_with_labels(header, &HashMap::new(), &labels)?
        .into_bytes();
    Ok(Some((raw, desired)))
}
fn receipt_path(relative: &Path) -> Result<PathBuf> {
    let name = hex::encode(Sha256::digest(serde_json::to_vec(relative)?));
    Ok(PathBuf::from(".recovery/migration/retirement/receipts").join(format!("{name}.json")))
}
/// Recompute both valid states from the authenticated snapshot and reject a
/// forged receipt or any live bytes other than the input or derived output.
#[cfg(test)]
pub(super) fn expected_live(
    app: &mut Application,
    saved: &Path,
    work: &Path,
    relative: &Path,
) -> Result<Option<String>> {
    expected_live_with_labels(app, saved, work, relative, None)
}
pub(super) fn expected_live_with_labels(
    app: &mut Application,
    saved: &Path,
    work: &Path,
    relative: &Path,
    labels: Option<&Path>,
) -> Result<Option<String>> {
    let Some((raw, desired)) = derive_page(app, saved, relative, labels)? else {
        return Ok(None);
    };
    let mut current = Vec::new();
    bounded_copy::source_file(&app.root, relative)?
        .take(MAX_PAGE_BYTES + 1)
        .read_to_end(&mut current)?;
    if current.len() as u64 > MAX_PAGE_BYTES || (current != raw && current != desired) {
        return Err(GardenError::Recovery("A knowledge page changed during retirement; its current bytes and the snapshot remain preserved.".into()));
    }
    let path = receipt_path(relative)?;
    match fs::symlink_metadata(work.join(&path)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
        Ok(m) if m.is_file() => {
            let mut bytes = Vec::new();
            bounded_copy::source_file(work, &path)?
                .take(16385)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 16384 {
                return Err(GardenError::Recovery(
                    "Retirement receipt is too large.".into(),
                ));
            }
            let receipt: Receipt = serde_json::from_slice(&bytes)?;
            if receipt.path != relative
                || receipt.snapshot_sha256 != hex::encode(Sha256::digest(&raw))
                || receipt.output_sha256 != hex::encode(Sha256::digest(&desired))
                || current != desired
            {
                return Err(GardenError::Recovery(
                    "Retirement receipt does not match the authenticated page output.".into(),
                ));
            }
        }
        Ok(_) => {
            return Err(GardenError::Recovery(
                "Retirement receipt is not a regular file.".into(),
            ))
        }
    }
    Ok(Some(manifest_cursor::digest_bytes(&current)))
}
/// Retire one page only after the caller has verified imported receipts and
/// unrelated live files. Its receipt is validated against rederived output.
#[cfg(test)]
pub(super) fn retire_one(
    app: &mut Application,
    saved: &Path,
    work: &Path,
    relative: &Path,
) -> Result<()> {
    retire_one_with_labels(app, saved, work, relative, None)
}
pub(super) fn retire_one_with_labels(
    app: &mut Application,
    saved: &Path,
    work: &Path,
    relative: &Path,
    labels: Option<&Path>,
) -> Result<()> {
    let Some((raw, desired)) = derive_page(app, saved, relative, labels)? else {
        return Ok(());
    };
    expected_live_with_labels(app, saved, work, relative, labels)?;
    let mut current = Vec::new();
    bounded_copy::source_file(&app.root, relative)?
        .take(MAX_PAGE_BYTES + 1)
        .read_to_end(&mut current)?;
    if current.len() as u64 > MAX_PAGE_BYTES || (current != raw && current != desired) {
        return Err(GardenError::Recovery("A knowledge page changed during retirement; its current bytes and the snapshot remain preserved.".into()));
    }
    if current == raw && current != desired {
        let destination = app.root.join(relative);
        commit_publication(
            &app.root,
            vec![(destination.clone(), desired.clone())],
            &HashMap::from([(destination, raw.clone())]),
            None,
        )?;
        #[cfg(test)]
        if std::env::var_os("KG_RETIRE_STOP_AFTER_PUBLICATION").is_some() {
            std::process::exit(94);
        }
    }
    let mut published = Vec::new();
    bounded_copy::source_file(&app.root, relative)?
        .take(MAX_PAGE_BYTES + 1)
        .read_to_end(&mut published)?;
    if published != desired {
        return Err(GardenError::Recovery(
            "Retired page differs from its authenticated output.".into(),
        ));
    }
    let receipt = Receipt {
        path: relative.to_owned(),
        snapshot_sha256: hex::encode(Sha256::digest(&raw)),
        output_sha256: hex::encode(Sha256::digest(&desired)),
    };
    authored_stage::publish_once(
        work,
        &receipt_path(relative)?,
        &serde_json::to_vec(&receipt)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pending(root: &Path) -> Application {
        Application::open_pending_with_all_providers_and_meaning_assets(
            root,
            Arc::new(UnavailableProvider),
            Arc::new(LocalExtractor),
            Arc::new(NativeAudioProcessor),
            None::<&Path>,
        )
        .unwrap()
    }
    fn fixture(root: &Path) {
        fs::create_dir_all(root.join("collection/pages")).unwrap();
        fs::create_dir_all(root.join("collection/.staging/transactions")).unwrap();
        fs::create_dir_all(root.join("saved/files/pages")).unwrap();
        fs::create_dir(root.join("work")).unwrap();
        let evidence = serde_json::json!({"quote":"source", "byte_start":0,"byte_end":6,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({"schema":1,"page_id":"page-1","source_id":"source-1","title":"Page","kind":"event","evidence":evidence,"facts":[],"owner_content":["historical statement"],"uninterpreted_owner_edits":[]})).unwrap();
        let page = format!("---\n{}---\n", serde_yaml_ng::to_string(&header).unwrap());
        fs::write(root.join("collection/pages/page.md"), &page).unwrap();
        fs::write(root.join("saved/files/pages/page.md"), &page).unwrap();
        let db = root.join("saved/manifest.sqlite");
        manifest_records::create(&db).unwrap();
        let digest =
            manifest_cursor::digest_file(&root.join("saved/files"), Path::new("pages/page.md"))
                .unwrap();
        manifest_records::put(&db, Path::new("pages/page.md"), Some(&digest)).unwrap();
    }
    #[test]
    fn verified_label_catalog_keeps_relationship_output_stable_after_restart() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fixture(root);
        let evidence = serde_json::json!({"quote":"source", "byte_start":0,"byte_end":6,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({"schema":1,"page_id":"page-1","source_id":"source-1","title":"First","kind":"event","evidence":evidence,"facts":[],"owner_content":["historical statement"],"uninterpreted_owner_edits":[],"relationships":[{"relationship_id":"rel-1","from_page_id":"page-1","to_page_id":"page-2","kind":"related","qualifier":null,"origin":"source","evidence":evidence}]})).unwrap();
        let page = format!("---\n{}---\n", serde_yaml_ng::to_string(&header).unwrap());
        fs::write(root.join("collection/pages/page.md"), &page).unwrap();
        fs::write(root.join("saved/files/pages/page.md"), &page).unwrap();
        let digest =
            manifest_cursor::digest_file(&root.join("saved/files"), Path::new("pages/page.md"))
                .unwrap();
        manifest_records::set(
            &root.join("saved/manifest.sqlite"),
            Path::new("pages/page.md"),
            Some(&digest),
        )
        .unwrap();
        let labels = root.join("labels.sqlite");
        label_catalog::create(&labels).unwrap();
        label_catalog::put(&labels, "page-1", "First").unwrap();
        label_catalog::put(&labels, "page-2", "Second label").unwrap();
        let mut app = pending(&root.join("collection"));
        retire_one_with_labels(
            &mut app,
            &root.join("saved"),
            &root.join("work"),
            Path::new("pages/page.md"),
            Some(&labels),
        )
        .unwrap();
        let output = fs::read_to_string(root.join("collection/pages/page.md")).unwrap();
        assert!(output.contains("Second label"));
        drop(app);
        let mut reopened = pending(&root.join("collection"));
        expected_live_with_labels(
            &mut reopened,
            &root.join("saved"),
            &root.join("work"),
            Path::new("pages/page.md"),
            Some(&labels),
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(root.join("collection/pages/page.md")).unwrap(),
            output
        );
    }
    #[test]
    fn crash_child() {
        let Some(root) = std::env::var_os("KG_RETIRE_TEST_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let mut app = pending(&root.join("collection"));
        retire_one(
            &mut app,
            &root.join("saved"),
            &root.join("work"),
            Path::new("pages/page.md"),
        )
        .unwrap();
        panic!("publication hook did not run");
    }
    #[test]
    fn interrupted_publication_rederives_output_before_recording_receipt() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fixture(root);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::retirement::tests::crash_child",
            ])
            .env("KG_RETIRE_TEST_ROOT", root)
            .env("KG_RETIRE_STOP_AFTER_PUBLICATION", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(94));
        let mut app = pending(&root.join("collection"));
        retire_one(
            &mut app,
            &root.join("saved"),
            &root.join("work"),
            Path::new("pages/page.md"),
        )
        .unwrap();
        assert!(!fs::read_to_string(root.join("collection/pages/page.md"))
            .unwrap()
            .contains("historical statement"));
        assert!(fs::read_to_string(root.join("saved/files/pages/page.md"))
            .unwrap()
            .contains("historical statement"));
        retire_one(
            &mut app,
            &root.join("saved"),
            &root.join("work"),
            Path::new("pages/page.md"),
        )
        .unwrap();
        let receipt = root
            .join("work")
            .join(receipt_path(Path::new("pages/page.md")).unwrap());
        fs::write(&receipt, b"forged receipt").unwrap();
        assert!(expected_live(
            &mut app,
            &root.join("saved"),
            &root.join("work"),
            Path::new("pages/page.md")
        )
        .is_err());
    }
    #[test]
    fn external_edit_collision_is_preserved() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fixture(root);
        let page = root.join("collection/pages/page.md");
        fs::write(&page, b"external owner edit").unwrap();
        let mut app = pending(&root.join("collection"));
        assert!(retire_one(
            &mut app,
            &root.join("saved"),
            &root.join("work"),
            Path::new("pages/page.md")
        )
        .is_err());
        assert_eq!(fs::read(page).unwrap(), b"external owner edit");
    }
}
