//! Rebuild the expected live collection from the verified snapshot and
//! independently rederived page outputs after an interrupted retirement.
use super::*;
use std::os::unix::fs::MetadataExt;

pub(super) enum Action {
    More,
    Page(PathBuf),
    Imported(String),
    Complete,
}
pub(super) struct Session {
    work: tempfile::TempDir,
    cursor: Option<Vec<u8>>,
    pending: Option<(Vec<u8>, PathBuf, Option<String>)>,
    imported_cursor: Option<Vec<u8>>,
    pending_import: Option<(Vec<u8>, String)>,
    phase: u8,
    verification: incremental_verify::Session,
    identity: (u64, u64, u64, i64, i64),
}
fn identity(path: &Path) -> Result<(u64, u64, u64, i64, i64)> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() {
        return Err(GardenError::Recovery(
            "Retirement manifest is not a regular file.".into(),
        ));
    }
    Ok((m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec()))
}
fn overlay(work: &Path, path: &Path, digest: &str) -> Result<()> {
    let database = work.join("expected.sqlite");
    let mut parent = path.parent();
    while let Some(directory) = parent {
        if directory.as_os_str().is_empty() {
            break;
        }
        if let Some(existing) = manifest_records::get(&database, directory)? {
            if existing.digest.is_some() {
                return Err(GardenError::Recovery(
                    "Trusted record parent collides with a saved file.".into(),
                ));
            }
        } else {
            manifest_records::put(&database, directory, None)?;
        }
        parent = directory.parent();
    }
    manifest_records::set(&database, path, Some(digest))
}
fn tracked(path: &Path) -> bool {
    (path.starts_with("pages") && path.extension().is_some_and(|ext| ext == "md"))
        || (path.starts_with("sources")
            && (path.file_name().is_some_and(|name| name == "index.md")
                || (path
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == "acquisitions")
                    && path.extension().is_some_and(|ext| ext == "md"))))
}
impl Session {
    pub(super) fn new(saved: &Path) -> Result<Self> {
        let work = tempfile::tempdir()?;
        manifest_records::create(&work.path().join("expected.sqlite"))?;
        Ok(Self {
            work,
            cursor: None,
            pending: None,
            imported_cursor: None,
            pending_import: None,
            phase: 0,
            verification: incremental_verify::Session::new(),
            identity: identity(&saved.join("manifest.sqlite"))?,
        })
    }
    #[cfg(test)]
    pub(super) fn step(&mut self, root: &Path, saved: &Path, ignored: &Path) -> Result<Action> {
        self.step_with_intents(root, saved, ignored, None)
    }
    pub(super) fn step_with_intents(
        &mut self,
        root: &Path,
        saved: &Path,
        ignored: &Path,
        intents: Option<&Path>,
    ) -> Result<Action> {
        if identity(&saved.join("manifest.sqlite"))? != self.identity {
            return Err(GardenError::Recovery(
                "Retirement snapshot manifest changed.".into(),
            ));
        }
        match self.phase {
            0 => {
                if let Some((_, path, _)) = &self.pending {
                    return Ok(Action::Page(path.clone()));
                }
                let Some((key, entry)) =
                    manifest_records::next(&saved.join("manifest.sqlite"), self.cursor.as_deref())?
                else {
                    self.phase = 1;
                    return Ok(Action::More);
                };
                if entry.digest.is_some() && tracked(&entry.path) {
                    self.pending = Some((key, entry.path.clone(), entry.digest));
                    return Ok(Action::Page(entry.path));
                }
                manifest_records::put(
                    &self.work.path().join("expected.sqlite"),
                    &entry.path,
                    entry.digest.as_deref(),
                )?;
                self.cursor = Some(key);
                Ok(Action::More)
            }
            1 => {
                if let Some((_, id)) = &self.pending_import {
                    return Ok(Action::Imported(id.clone()));
                }
                if let Some(intents) = intents {
                    if let Some((key, entry)) =
                        manifest_records::next(intents, self.imported_cursor.as_deref())?
                    {
                        let id = entry
                            .path
                            .to_str()
                            .ok_or_else(|| {
                                GardenError::Recovery("Imported source identity is invalid.".into())
                            })?
                            .to_owned();
                        if entry.digest.is_none() {
                            return Err(GardenError::Recovery(
                                "Imported source digest is missing.".into(),
                            ));
                        }
                        self.pending_import = Some((key, id.clone()));
                        return Ok(Action::Imported(id));
                    }
                }
                incremental_verify::begin_ignoring(
                    root,
                    &self.work.path().join("expected.sqlite"),
                    self.work.path(),
                    Some(ignored),
                )?;
                self.phase = 2;
                Ok(Action::More)
            }
            2 => {
                if incremental_verify::step_ignoring(
                    root,
                    &self.work.path().join("expected.sqlite"),
                    self.work.path(),
                    &mut self.verification,
                    Some(ignored),
                )? {
                    self.phase = 3;
                    return Ok(Action::Complete);
                }
                Ok(Action::More)
            }
            3 => Ok(Action::Complete),
            _ => Err(GardenError::Recovery(
                "Retirement verification state is invalid.".into(),
            )),
        }
    }
    pub(super) fn imported(
        &mut self,
        app: &mut Application,
        saved: &Path,
        intent_work: &Path,
        receipt_work: &Path,
        id: &str,
    ) -> Result<()> {
        let (key, expected) = self.pending_import.as_ref().ok_or_else(|| {
            GardenError::Recovery("Retirement verification has no pending import.".into())
        })?;
        if expected != id {
            return Err(GardenError::Recovery(
                "Retirement imported source cursor changed.".into(),
            ));
        }
        authored_stage::verify_receipt(app, saved, intent_work, receipt_work, id)?;
        let directory = app.source_dir(id)?;
        let path = directory
            .join("index.md")
            .strip_prefix(&app.root)
            .map_err(|_| {
                GardenError::Recovery("Imported source path escaped the collection.".into())
            })?
            .to_path_buf();
        let mut bytes = Vec::new();
        bounded_copy::source_file(&app.root, &path)?
            .take(MAX_PAGE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_PAGE_BYTES {
            return Err(GardenError::Recovery(
                "Imported source page is too large.".into(),
            ));
        }
        for (record, digest) in
            trusted_retirement::existing_expected(&app.root, receipt_work, &path, &bytes)?
        {
            overlay(self.work.path(), &record, &digest)?;
        }
        self.imported_cursor = Some(key.clone());
        self.pending_import = None;
        Ok(())
    }
    /// Called with the application mutex for only one bounded page.
    #[cfg(test)]
    pub(super) fn page(
        &mut self,
        app: &mut Application,
        saved: &Path,
        receipt_work: &Path,
        path: &Path,
    ) -> Result<()> {
        self.page_with_labels(app, saved, receipt_work, path, None)
    }
    pub(super) fn page_with_labels(
        &mut self,
        app: &mut Application,
        saved: &Path,
        receipt_work: &Path,
        path: &Path,
        labels: Option<&Path>,
    ) -> Result<()> {
        let (key, expected_path, original) = self.pending.as_ref().ok_or_else(|| {
            GardenError::Recovery("Retirement verification has no pending page.".into())
        })?;
        if path != expected_path {
            return Err(GardenError::Recovery(
                "Retirement page cursor changed.".into(),
            ));
        }
        let authenticated = if path.starts_with("pages") {
            retirement::expected_live_with_labels(app, saved, receipt_work, path, labels)?
        } else {
            let file = bounded_copy::source_file(&app.root, path)?;
            if file.metadata()?.len() > MAX_PAGE_BYTES {
                None
            } else {
                let current = manifest_cursor::digest_file(&app.root, path)?;
                if original.as_deref() != Some(current.as_str()) {
                    return Err(GardenError::Recovery(
                        "A source page changed during retirement.".into(),
                    ));
                }
                Some(current)
            }
        };
        if authenticated.is_some() {
            if path.starts_with("pages") {
                if let Some(raw) = retirement::snapshot_bytes(saved, path)? {
                    if let Some((record, digest)) =
                        trusted_retirement::existing_blob(&app.root, &raw)?
                    {
                        overlay(self.work.path(), &record, &digest)?;
                    }
                }
            }
            let mut bytes = Vec::new();
            bounded_copy::source_file(&app.root, path)?
                .take(MAX_PAGE_BYTES + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_PAGE_BYTES {
                return Err(GardenError::Recovery(
                    "Retirement page is too large.".into(),
                ));
            }
            for (record, digest) in
                trusted_retirement::existing_expected(&app.root, receipt_work, path, &bytes)?
            {
                overlay(self.work.path(), &record, &digest)?;
            }
        }
        let current = authenticated.or_else(|| original.clone());
        manifest_records::put(
            &self.work.path().join("expected.sqlite"),
            path,
            current.as_deref(),
        )?;
        self.cursor = Some(key.clone());
        self.pending = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rebuilt_live_manifest_accepts_retired_page_and_rejects_unrelated_change() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        fs::create_dir_all(root.join("pages")).unwrap();
        fs::create_dir_all(root.join(".staging/transactions")).unwrap();
        fs::create_dir_all(root.join("sources/fixture")).unwrap();
        fs::write(root.join("sources/fixture/index.md"), b"source metadata").unwrap();
        let root = fs::canonicalize(root).unwrap();
        let evidence = serde_json::json!({"quote":"source", "byte_start":0,"byte_end":6,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({"schema":1,"page_id":"page-1","source_id":"source-1","title":"Page","kind":"event","evidence":evidence,"facts":[],"owner_content":["owner statement"],"uninterpreted_owner_edits":[]})).unwrap();
        fs::write(
            root.join("pages/page.md"),
            format!("---\n{}---\n", serde_yaml_ng::to_string(&header).unwrap()),
        )
        .unwrap();
        let work = Path::new(".recovery/migration/test-snapshot");
        snapshot_worker::begin(&root, work).unwrap();
        let mut worker = snapshot_worker::Session::new();
        let saved = loop {
            if let Some(saved) = snapshot_worker::step(&root, work, &mut worker).unwrap() {
                break saved;
            }
        };
        let receipts = temp.path().join("receipts");
        fs::create_dir(&receipts).unwrap();
        let ignored = temp.path().join("ignored.sqlite");
        manifest_records::create(&ignored).unwrap();
        let mut app = Application::open_pending_with_all_providers_and_meaning_assets(
            &root,
            Arc::new(UnavailableProvider),
            Arc::new(LocalExtractor),
            Arc::new(NativeAudioProcessor),
            None::<&Path>,
        )
        .unwrap();
        retirement::retire_one(&mut app, &saved, &receipts, Path::new("pages/page.md")).unwrap();
        let retired = fs::read(root.join("pages/page.md")).unwrap();
        trusted_retirement::record_one(&root, &receipts, Path::new("pages/page.md"), &retired)
            .unwrap();
        trusted_retirement::record_one(
            &root,
            &receipts,
            Path::new("sources/fixture/index.md"),
            b"source metadata",
        )
        .unwrap();
        let mut session = Session::new(&saved).unwrap();
        let mut done = false;
        for _ in 0..500 {
            match session.step(&root, &saved, &ignored).unwrap() {
                Action::More => (),
                Action::Imported(_) => panic!("unexpected imported source"),
                Action::Page(path) => session.page(&mut app, &saved, &receipts, &path).unwrap(),
                Action::Complete => {
                    done = true;
                    break;
                }
            }
        }
        assert!(done);
        fs::write(root.join("unrelated"), b"owner edit").unwrap();
        let mut session = Session::new(&saved).unwrap();
        let mut rejected = false;
        for _ in 0..500 {
            match session.step(&root, &saved, &ignored) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(Action::More) => (),
                Ok(Action::Imported(_)) => panic!("unexpected imported source"),
                Ok(Action::Page(path)) => session.page(&mut app, &saved, &receipts, &path).unwrap(),
                Ok(Action::Complete) => break,
            }
        }
        assert!(rejected);
    }
}
