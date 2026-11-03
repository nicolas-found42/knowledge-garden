//! Per-entry retirement with a fresh, bounded live-tree recheck before every
//! page or trusted-record publication. Cursors restart from the beginning.
use super::*;

pub(crate) enum Action {
    More,
    VerifyPage(PathBuf),
    VerifyImport(String),
    Retire(PathBuf),
    Trust(PathBuf),
    TrustImport(String),
    Complete,
}
pub(crate) struct Session {
    phase: u8,
    after: Option<Vec<u8>>,
    current: Option<(Vec<u8>, PathBuf)>,
    verify: Option<retirement_verify::Session>,
    plan_rechecking: bool,
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
    pub(crate) fn new() -> Self {
        Self {
            phase: 0,
            after: None,
            current: None,
            verify: None,
            plan_rechecking: false,
        }
    }
    fn verify_turn(
        &mut self,
        root: &Path,
        saved: &Path,
        plan: &mut cutover_plan::Session,
    ) -> Result<Option<Action>> {
        if !self.plan_rechecking {
            plan.recheck_indexes()?;
            self.plan_rechecking = true;
        }
        if !cutover_plan::step(saved, plan)? {
            return Ok(Some(Action::More));
        }
        if self.verify.is_none() {
            self.verify = Some(retirement_verify::Session::new(saved)?);
            return Ok(Some(Action::More));
        }
        let work = plan.work();
        let ignored = work.join(".recovery/migration/authored-stage/ignored.sqlite");
        let intents = work.join(".recovery/migration/authored-stage/intents.sqlite");
        match self.verify.as_mut().unwrap().step_with_intents(
            root,
            saved,
            &ignored,
            Some(&intents),
        )? {
            retirement_verify::Action::More => Ok(Some(Action::More)),
            retirement_verify::Action::Page(path) => Ok(Some(Action::VerifyPage(path))),
            retirement_verify::Action::Imported(id) => Ok(Some(Action::VerifyImport(id))),
            retirement_verify::Action::Complete => {
                self.verify = None;
                self.plan_rechecking = false;
                Ok(None)
            }
        }
    }
    pub(crate) fn step(
        &mut self,
        root: &Path,
        saved: &Path,
        plan: &mut cutover_plan::Session,
    ) -> Result<Action> {
        match self.phase {
            0 | 2 | 4 | 6 => {
                if let Some(action) = self.verify_turn(root, saved, plan)? {
                    return Ok(action);
                }
                self.phase += 1;
                Ok(Action::More)
            }
            1 => {
                let Some((key, entry)) =
                    manifest_records::next(&saved.join("manifest.sqlite"), self.after.as_deref())?
                else {
                    self.after = None;
                    self.phase = 2;
                    return Ok(Action::More);
                };
                if entry.digest.is_some()
                    && entry.path.starts_with("pages")
                    && entry.path.extension().is_some_and(|ext| ext == "md")
                {
                    self.current = Some((key, entry.path.clone()));
                    return Ok(Action::Retire(entry.path));
                }
                self.after = Some(key);
                Ok(Action::More)
            }
            3 => {
                let Some((key, entry)) =
                    manifest_records::next(&saved.join("manifest.sqlite"), self.after.as_deref())?
                else {
                    self.after = None;
                    self.phase = 4;
                    return Ok(Action::More);
                };
                if entry.digest.is_some() && tracked(&entry.path) {
                    self.current = Some((key, entry.path.clone()));
                    return Ok(Action::Trust(entry.path));
                }
                self.after = Some(key);
                Ok(Action::More)
            }
            5 => {
                let intents = plan
                    .work()
                    .join(".recovery/migration/authored-stage/intents.sqlite");
                let Some((key, entry)) = manifest_records::next(&intents, self.after.as_deref())?
                else {
                    self.after = None;
                    self.phase = 6;
                    return Ok(Action::More);
                };
                let id = entry
                    .path
                    .to_str()
                    .ok_or_else(|| {
                        GardenError::Recovery("Authored source identity is invalid.".into())
                    })?
                    .to_owned();
                self.current = Some((key, entry.path));
                Ok(Action::TrustImport(id))
            }
            7 => Ok(Action::Complete),
            _ => Err(GardenError::Recovery("Retirement phase is invalid.".into())),
        }
    }
    pub(crate) fn verify_page(
        &mut self,
        app: &mut Application,
        saved: &Path,
        receipts: &Path,
        labels: &Path,
        path: &Path,
    ) -> Result<()> {
        self.verify
            .as_mut()
            .ok_or_else(|| GardenError::Recovery("Retirement verification is missing.".into()))?
            .page_with_labels(app, saved, receipts, path, Some(labels))
    }
    pub(crate) fn verify_import(
        &mut self,
        app: &mut Application,
        saved: &Path,
        plan_work: &Path,
        receipts: &Path,
        id: &str,
    ) -> Result<()> {
        self.verify
            .as_mut()
            .ok_or_else(|| GardenError::Recovery("Retirement verification is missing.".into()))?
            .imported(app, saved, plan_work, receipts, id)
    }
    pub(crate) fn retire(
        &mut self,
        app: &mut Application,
        saved: &Path,
        receipts: &Path,
        labels: &Path,
        path: &Path,
    ) -> Result<()> {
        let (key, expected) = self
            .current
            .as_ref()
            .ok_or_else(|| GardenError::Recovery("Retirement cursor is missing.".into()))?;
        if self.phase != 1 || expected != path {
            return Err(GardenError::Recovery("Retirement cursor changed.".into()));
        }
        retirement::retire_one_with_labels(app, saved, receipts, path, Some(labels))?;
        self.after = Some(key.clone());
        self.current = None;
        self.phase = 0;
        Ok(())
    }
    pub(crate) fn trust(
        &mut self,
        app: &mut Application,
        saved: &Path,
        receipts: &Path,
        labels: &Path,
        path: &Path,
    ) -> Result<()> {
        let (key, expected) = self
            .current
            .as_ref()
            .ok_or_else(|| GardenError::Recovery("Trusted page cursor is missing.".into()))?;
        if self.phase != 3 || expected != path {
            return Err(GardenError::Recovery("Trusted page cursor changed.".into()));
        }
        let entry =
            manifest_records::get(&saved.join("manifest.sqlite"), path)?.ok_or_else(|| {
                GardenError::Recovery("Trusted page is absent from the snapshot.".into())
            })?;
        let file = bounded_copy::source_file(&app.root, path)?;
        if file.metadata()?.len() <= MAX_PAGE_BYTES {
            if path.starts_with("pages") {
                if retirement::expected_live_with_labels(app, saved, receipts, path, Some(labels))?
                    .is_none()
                {
                    if manifest_cursor::digest_file(&app.root, path)?
                        != entry.digest.unwrap_or_default()
                    {
                        return Err(GardenError::Recovery("Unparsed saved page changed.".into()));
                    }
                }
            } else if manifest_cursor::digest_file(&app.root, path)?
                != entry.digest.unwrap_or_default()
            {
                return Err(GardenError::Recovery(
                    "Source page changed during retirement.".into(),
                ));
            }
            let mut bytes = Vec::new();
            file.take(MAX_PAGE_BYTES + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_PAGE_BYTES {
                return Err(GardenError::Recovery(
                    "Trusted page grew during retirement.".into(),
                ));
            }
            trusted_retirement::record_one(&app.root, receipts, path, &bytes)?;
        }
        self.after = Some(key.clone());
        self.current = None;
        self.phase = 2;
        Ok(())
    }
    pub(crate) fn trust_import(
        &mut self,
        app: &mut Application,
        saved: &Path,
        plan_work: &Path,
        receipts: &Path,
        id: &str,
    ) -> Result<()> {
        let (key, expected) = self
            .current
            .as_ref()
            .ok_or_else(|| GardenError::Recovery("Trusted import cursor is missing.".into()))?;
        if self.phase != 5 || expected != Path::new(id) {
            return Err(GardenError::Recovery(
                "Trusted import cursor changed.".into(),
            ));
        }
        authored_stage::verify_receipt(app, saved, plan_work, receipts, id)?;
        let path = app
            .source_dir(id)?
            .join("index.md")
            .strip_prefix(&app.root)
            .map_err(|_| GardenError::Recovery("Imported source escaped the collection.".into()))?
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
        trusted_retirement::record_one(&app.root, receipts, &path, &bytes)?;
        self.after = Some(key.clone());
        self.current = None;
        self.phase = 4;
        Ok(())
    }
}
