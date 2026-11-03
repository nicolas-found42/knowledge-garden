//! Desktop pre-cutover: reconstruct owner evidence, recheck the snapshot and
//! unrelated live files, then import one deterministic source per turn.
use super::*;

pub(crate) enum Action {
    More,
    Import(String),
    Receipts,
    VerifyPage(PathBuf),
    VerifyImport(String),
    Retire(PathBuf),
    Trust(PathBuf),
    TrustImport(String),
    Complete,
}
pub(crate) struct Session {
    saved: PathBuf,
    receipt_work: PathBuf,
    plan: cutover_plan::Session,
    phase: u8,
    after: Option<Vec<u8>>,
    current: Option<(Vec<u8>, String)>,
    verification: Option<(tempfile::TempDir, incremental_verify::Session)>,
    resume_retirement: bool,
    legacy_resume: bool,
    retirement_state: String,
    retirement: Option<retirement_flow::Session>,
}
impl Session {
    pub(crate) fn new(root: &Path) -> Result<Option<Self>> {
        let mut state_bytes = Vec::new();
        bounded_copy::source_file(root, Path::new(".recovery/migration/state.json"))?
            .take(4097)
            .read_to_end(&mut state_bytes)?;
        if state_bytes.len() > 4096 {
            return Err(GardenError::Recovery(
                "Migration state is too large.".into(),
            ));
        }
        let state: MigrationState = serde_json::from_slice(&state_bytes)?;
        if !["backup_verified", "evidence_imported", "cutover_preparing"]
            .contains(&state.phase.as_str())
        {
            return Ok(None);
        }
        if !state.backup.starts_with("backup-")
            || Path::new(&state.backup).components().count() != 1
        {
            return Err(GardenError::Recovery(
                "Migration backup identity is invalid.".into(),
            ));
        }
        let saved = root.join(".recovery/migration").join(&state.backup);
        match fs::symlink_metadata(saved.join("manifest.sqlite")) {
            Ok(m) if m.is_file() => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Migration manifest is not a regular file.".into(),
                ))
            }
        }
        let receipt_work = root
            .join(".recovery/migration")
            .join(format!("cutover-receipts-{}", state.backup));
        validate_real_collection_ancestors(root, &receipt_work.join("entry"))?;
        fs::create_dir_all(&receipt_work)?;
        let marker = receipt_work.join(".recovery/migration/retirement/started.json");
        let legacy_phase = state.phase != "backup_verified";
        let marker_value = match fs::symlink_metadata(&marker) {
            Ok(m) if m.is_file() => Some(serde_json::from_str::<String>(&read_bounded_text(
                &marker, 4096,
            )?)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Retirement marker is not a regular file.".into(),
                ))
            }
        };
        if marker_value
            .as_deref()
            .is_some_and(|value| value != state.backup)
        {
            return Err(GardenError::Recovery(
                "Retirement marker conflicts with migration state.".into(),
            ));
        }
        let resume_retirement = legacy_phase && marker_value.is_some();
        let legacy_resume = legacy_phase && marker_value.is_none();
        Ok(Some(Self {
            plan: cutover_plan::Session::new(&saved)?,
            saved,
            receipt_work,
            phase: 0,
            after: None,
            current: None,
            verification: None,
            resume_retirement,
            legacy_resume,
            retirement_state: state.phase,
            retirement: None,
        }))
    }
    fn begin_verification(&mut self, files: &Path, ignored: Option<&Path>) -> Result<()> {
        let scratch = tempfile::tempdir()?;
        incremental_verify::begin_ignoring(
            files,
            &self.saved.join("manifest.sqlite"),
            scratch.path(),
            ignored,
        )?;
        self.verification = Some((scratch, incremental_verify::Session::new()));
        Ok(())
    }
    fn verify_turn(&mut self, files: &Path, ignored: Option<&Path>) -> Result<bool> {
        if self.verification.is_none() {
            self.begin_verification(files, ignored)?;
            return Ok(false);
        }
        let (work, session) = self.verification.as_mut().unwrap();
        if incremental_verify::step_ignoring(
            files,
            &self.saved.join("manifest.sqlite"),
            work.path(),
            session,
            ignored,
        )? {
            self.verification = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub(crate) fn step(&mut self, root: &Path) -> Result<Action> {
        match self.phase {
            0 => {
                if cutover_plan::step(&self.saved, &mut self.plan)? {
                    self.phase = if self.resume_retirement { 7 } else { 1 };
                }
                Ok(Action::More)
            }
            1 => {
                let index = self
                    .plan
                    .work()
                    .join(".recovery/migration/authored-stage/intents.sqlite");
                let Some((key, entry)) = manifest_records::next(&index, self.after.as_deref())?
                else {
                    self.phase = if self.legacy_resume { 7 } else { 5 };
                    return Ok(Action::More);
                };
                let id = entry
                    .path
                    .to_str()
                    .ok_or_else(|| {
                        GardenError::Recovery("Authored source identity is invalid.".into())
                    })?
                    .to_owned();
                self.current = Some((key, id));
                self.phase = 2;
                Ok(Action::More)
            }
            2 => {
                let files = self.saved.join("files");
                if self.verify_turn(&files, None)? {
                    self.phase = 3;
                }
                Ok(Action::More)
            }
            3 => {
                if self.legacy_resume {
                    self.phase = 4;
                    return Ok(Action::More);
                }
                let ignored = self
                    .plan
                    .work()
                    .join(".recovery/migration/authored-stage/ignored.sqlite");
                if self.verify_turn(root, Some(&ignored))? {
                    self.phase = 4;
                }
                Ok(Action::More)
            }
            4 => Ok(Action::Import(
                self.current
                    .as_ref()
                    .ok_or_else(|| {
                        GardenError::Recovery("Authored import cursor is missing.".into())
                    })?
                    .1
                    .clone(),
            )),
            5 => {
                let files = self.saved.join("files");
                if self.verify_turn(&files, None)? {
                    self.phase = 6;
                }
                Ok(Action::More)
            }
            6 => {
                let ignored = self
                    .plan
                    .work()
                    .join(".recovery/migration/authored-stage/ignored.sqlite");
                if self.verify_turn(root, Some(&ignored))? {
                    self.phase = 7;
                }
                Ok(Action::More)
            }
            7 => Ok(Action::Receipts),
            8 => {
                if !self.resume_retirement {
                    let id = self
                        .saved
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or_else(|| {
                            GardenError::Recovery("Retirement backup identity is invalid.".into())
                        })?;
                    authored_stage::publish_once(
                        &self.receipt_work,
                        Path::new(".recovery/migration/retirement/started.json"),
                        &serde_json::to_vec(id)?,
                    )?;
                    if self.retirement_state == "backup_verified" {
                        self.transition(root, "backup_verified", "evidence_imported")?;
                        self.retirement_state = "evidence_imported".into();
                    }
                    self.resume_retirement = true;
                }
                self.retirement = Some(retirement_flow::Session::new());
                self.phase = 9;
                Ok(Action::More)
            }
            9 => {
                match self
                    .retirement
                    .as_mut()
                    .unwrap()
                    .step(root, &self.saved, &mut self.plan)?
                {
                    retirement_flow::Action::More => Ok(Action::More),
                    retirement_flow::Action::VerifyPage(path) => Ok(Action::VerifyPage(path)),
                    retirement_flow::Action::VerifyImport(id) => Ok(Action::VerifyImport(id)),
                    retirement_flow::Action::Retire(path) => Ok(Action::Retire(path)),
                    retirement_flow::Action::Trust(path) => Ok(Action::Trust(path)),
                    retirement_flow::Action::TrustImport(id) => Ok(Action::TrustImport(id)),
                    retirement_flow::Action::Complete => {
                        self.transition(root, &self.retirement_state, "authority_retired")?;
                        self.phase = 10;
                        Ok(Action::Complete)
                    }
                }
            }
            10 => Ok(Action::Complete),
            _ => Err(GardenError::Recovery(
                "Authored pre-cutover phase is invalid.".into(),
            )),
        }
    }
    fn transition(&self, root: &Path, from: &str, to: &str) -> Result<()> {
        let relative = Path::new(".recovery/migration/state.json");
        let mut bytes = Vec::new();
        bounded_copy::source_file(root, relative)?
            .take(4097)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 4096 {
            return Err(GardenError::Recovery(
                "Migration state is too large.".into(),
            ));
        }
        let mut state: MigrationState = serde_json::from_slice(&bytes)?;
        if state.phase != from
            || self.saved.file_name().and_then(|name| name.to_str()) != Some(state.backup.as_str())
        {
            return Err(GardenError::Recovery(
                "Migration state changed during retirement.".into(),
            ));
        }
        state.phase = to.into();
        write_atomic(&root.join(relative), &serde_json::to_vec(&state)?)
    }
    pub(crate) fn retirement_action(
        &mut self,
        app: &mut Application,
        action: Action,
    ) -> Result<()> {
        if app
            .startup_recovering
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return Err(GardenError::Recovery(
                "Startup recovery is already running.".into(),
            ));
        }
        let _guard = IntegrityRecoveryGuard(Arc::clone(&app.startup_recovering));
        let labels = self
            .plan
            .work()
            .join(".recovery/migration/authored-stage/labels.sqlite");
        let flow = self
            .retirement
            .as_mut()
            .ok_or_else(|| GardenError::Recovery("Retirement session is missing.".into()))?;
        match action {
            Action::VerifyPage(path) => {
                flow.verify_page(app, &self.saved, &self.receipt_work, &labels, &path)
            }
            Action::VerifyImport(id) => {
                flow.verify_import(app, &self.saved, self.plan.work(), &self.receipt_work, &id)
            }
            Action::Retire(path) => {
                flow.retire(app, &self.saved, &self.receipt_work, &labels, &path)
            }
            Action::Trust(path) => flow.trust(app, &self.saved, &self.receipt_work, &labels, &path),
            Action::TrustImport(id) => {
                flow.trust_import(app, &self.saved, self.plan.work(), &self.receipt_work, &id)
            }
            _ => Err(GardenError::Recovery("Invalid retirement action.".into())),
        }
    }
    pub(crate) fn import(&mut self, app: &mut Application, id: &str) -> Result<()> {
        let (key, expected) = self
            .current
            .as_ref()
            .ok_or_else(|| GardenError::Recovery("Authored import cursor is missing.".into()))?;
        if self.phase != 4 || expected != id {
            return Err(GardenError::Recovery(
                "Authored import cursor changed.".into(),
            ));
        }
        if app
            .startup_recovering
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return Err(GardenError::Recovery(
                "Startup recovery is already running.".into(),
            ));
        }
        let _guard = IntegrityRecoveryGuard(Arc::clone(&app.startup_recovering));
        if self.legacy_resume {
            authored_stage::recover_existing(
                app,
                &self.saved,
                self.plan.work(),
                &self.receipt_work,
                id,
            )?;
        } else {
            authored_stage::import_one_with_receipts(
                app,
                &self.saved,
                self.plan.work(),
                &self.receipt_work,
                id,
            )?;
        }
        self.after = Some(key.clone());
        self.current = None;
        self.plan.recheck_indexes()?;
        self.phase = 0;
        Ok(())
    }
    pub(crate) fn verify_receipt(&mut self, app: &mut Application) -> Result<bool> {
        if self.phase != 7 {
            return Err(GardenError::Recovery(
                "Authored receipt phase is invalid.".into(),
            ));
        }
        if cutover_plan::verify_receipts_step(app, &self.saved, &mut self.plan, &self.receipt_work)?
        {
            self.phase = 8;
            return Ok(true);
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precutover_import_child() {
        let Some(root) = std::env::var_os("KG_PRECUTOVER_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let mut app = Application::open_pending_with_all_providers_and_meaning_assets(
            &root,
            Arc::new(UnavailableProvider),
            Arc::new(LocalExtractor),
            Arc::new(NativeAudioProcessor),
            None::<&Path>,
        )
        .unwrap();
        let mut cutover = Session::new(&root).unwrap().unwrap();
        loop {
            match cutover.step(&root).unwrap() {
                Action::More => (),
                Action::Import(id) => cutover.import(&mut app, &id).unwrap(),
                Action::Receipts => {
                    cutover.verify_receipt(&mut app).unwrap();
                }
                action @ (Action::VerifyPage(_)
                | Action::VerifyImport(_)
                | Action::Retire(_)
                | Action::Trust(_)
                | Action::TrustImport(_)) => cutover.retirement_action(&mut app, action).unwrap(),
                Action::Complete => panic!("hook did not run"),
            }
        }
    }
    #[test]
    fn retirement_crash_child() {
        let Some(root) = std::env::var_os("KG_PRECUTOVER_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let mut app = Application::open_pending_with_all_providers_and_meaning_assets(
            &root,
            Arc::new(UnavailableProvider),
            Arc::new(LocalExtractor),
            Arc::new(NativeAudioProcessor),
            None::<&Path>,
        )
        .unwrap();
        let mut cutover = Session::new(&root).unwrap().unwrap();
        loop {
            match cutover.step(&root).unwrap() {
                Action::More => (),
                Action::Import(id) => cutover.import(&mut app, &id).unwrap(),
                Action::Receipts => {
                    cutover.verify_receipt(&mut app).unwrap();
                }
                action @ (Action::VerifyPage(_)
                | Action::VerifyImport(_)
                | Action::Retire(_)
                | Action::Trust(_)
                | Action::TrustImport(_)) => cutover.retirement_action(&mut app, action).unwrap(),
                Action::Complete => panic!("retirement exit hook did not run"),
            }
        }
    }
    fn retirement_fixture(root: &Path) -> PathBuf {
        fs::create_dir_all(root.join("pages")).unwrap();
        fs::create_dir_all(root.join(".staging/transactions")).unwrap();
        fs::create_dir_all(root.join("sources")).unwrap();
        let evidence = serde_json::json!({"quote":"source", "byte_start":0,"byte_end":6,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({"schema":1,"page_id":"page-1","source_id":"source-1","title":"Page","kind":"event","evidence":evidence,"facts":[],"owner_content":["owner statement"],"uninterpreted_owner_edits":[]})).unwrap();
        fs::write(
            root.join("pages/page.md"),
            format!("---\n{}---\n", serde_yaml_ng::to_string(&header).unwrap()),
        )
        .unwrap();
        let work = Path::new(".recovery/migration/test-snapshot");
        snapshot_worker::begin(root, work).unwrap();
        let mut worker = snapshot_worker::Session::new();
        let saved = loop {
            if let Some(saved) = snapshot_worker::step(root, work, &mut worker).unwrap() {
                break saved;
            }
        };
        let state = MigrationState {
            backup: saved.file_name().unwrap().to_str().unwrap().into(),
            phase: "backup_verified".into(),
        };
        fs::write(
            root.join(".recovery/migration/state.json"),
            serde_json::to_vec(&state).unwrap(),
        )
        .unwrap();
        saved
    }
    #[test]
    fn legacy_partial_cutover_is_adopted_and_resumed() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        fs::create_dir_all(root.join("pages")).unwrap();
        fs::create_dir_all(root.join(".staging/transactions")).unwrap();
        fs::create_dir_all(root.join("sources")).unwrap();
        let evidence = serde_json::json!({"quote":"source", "byte_start":0,"byte_end":6,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({"schema":1,"page_id":"page-1","source_id":"source-1","title":"Page","kind":"event","evidence":evidence,"facts":[],"owner_content":["owner statement"],"uninterpreted_owner_edits":[]})).unwrap();
        let raw = format!("---\n{}---\n", serde_yaml_ng::to_string(&header).unwrap()).into_bytes();
        fs::write(root.join("pages/page.md"), &raw).unwrap();
        let saved = backup(&root).unwrap();
        let manifest = fs::read(saved.join("manifest.json")).unwrap();
        let state_path = root.join(".recovery/migration/state.json");
        let mut state = MigrationState {
            backup: saved.file_name().unwrap().to_str().unwrap().into(),
            phase: "backup_verified".into(),
        };
        fs::write(&state_path, serde_json::to_vec(&state).unwrap()).unwrap();
        let mut app = Application::open_pending_with_all_providers_and_meaning_assets(
            &root,
            Arc::new(UnavailableProvider),
            Arc::new(LocalExtractor),
            Arc::new(NativeAudioProcessor),
            None::<&Path>,
        )
        .unwrap();
        app.startup_recovering
            .store(true, std::sync::atomic::Ordering::Release);
        import_pending_authored_sources(&mut app).unwrap();
        state.phase = "cutover_preparing".into();
        trusted_pages::record(&root, Path::new("pages/page.md"), &raw).unwrap();
        for source in fs::read_dir(root.join("sources")).unwrap() {
            let source = source.unwrap().path().join("index.md");
            trusted_pages::record(
                &root,
                source.strip_prefix(&root).unwrap(),
                &fs::read(&source).unwrap(),
            )
            .unwrap();
        }
        fs::write(&state_path, serde_json::to_vec(&state).unwrap()).unwrap();
        let mut retired = header;
        retired.owner_content.clear();
        retired.owner_metadata.clear();
        retired.uninterpreted_owner_edits.clear();
        retired.external_edit_status = None;
        let output = app.render_knowledge_page(retired, &HashMap::new()).unwrap();
        let destination = root.join("pages/page.md");
        commit_publication(
            &root,
            vec![(destination.clone(), output.into_bytes())],
            &HashMap::from([(destination, raw)]),
            None,
        )
        .unwrap();
        drop(app);
        let mut adoption = legacy_adopt::Session::new(&root).unwrap().unwrap();
        for _ in 0..10000 {
            if adoption.step().unwrap() {
                break;
            }
        }
        assert!(saved.join("manifest.sqlite").is_file());
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::precutover::tests::retirement_crash_child",
            ])
            .env("KG_PRECUTOVER_ROOT", &root)
            .env("KG_TRUSTED_RETIRE_STOP_AFTER_RECORD", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(95));
        let app = Arc::new(std::sync::Mutex::new(
            Application::open_pending_with_all_providers_and_meaning_assets(
                &root,
                Arc::new(UnavailableProvider),
                Arc::new(LocalExtractor),
                Arc::new(NativeAudioProcessor),
                None::<&Path>,
            )
            .unwrap(),
        ));
        let control = crate::background::BackgroundControl::new(true);
        control.set_startup_pending(true);
        let worker = crate::background::spawn_worker(
            Arc::clone(&app),
            Arc::new(UnavailableProvider),
            Arc::clone(&control),
            std::time::Duration::from_millis(20),
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while control.startup_pending() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            !control.startup_pending(),
            "legacy recovery failed: {:?}",
            control.status()
        );
        control.stop();
        worker.join().unwrap();
        assert_eq!(fs::read(saved.join("manifest.json")).unwrap(), manifest);
        assert!(fs::read_to_string(saved.join("files/pages/page.md"))
            .unwrap()
            .contains("owner statement"));
        let sources: Vec<_> = fs::read_dir(root.join("sources")).unwrap().collect();
        assert_eq!(sources.len(), 1);
        let original = sources[0].as_ref().unwrap().path().join("original.txt");
        assert!(fs::read_to_string(original)
            .unwrap()
            .contains("owner statement"));
        let final_state: MigrationState =
            serde_json::from_slice(&fs::read(state_path).unwrap()).unwrap();
        assert_eq!(final_state.phase, "authority_retired");
    }
    #[test]
    fn external_page_edit_after_retirement_crash_blocks_recovery() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let saved = retirement_fixture(&root);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::precutover::tests::retirement_crash_child",
            ])
            .env("KG_PRECUTOVER_ROOT", &root)
            .env("KG_RETIRE_STOP_AFTER_PUBLICATION", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(94));
        fs::write(root.join("pages/page.md"), b"external owner edit").unwrap();
        let mut app = Application::open_pending_with_all_providers_and_meaning_assets(
            &root,
            Arc::new(UnavailableProvider),
            Arc::new(LocalExtractor),
            Arc::new(NativeAudioProcessor),
            None::<&Path>,
        )
        .unwrap();
        let mut cutover = Session::new(&root).unwrap().unwrap();
        let mut rejected = false;
        for _ in 0..10000 {
            match cutover.step(&root) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(Action::More) => (),
                Ok(Action::Receipts) => {
                    if cutover.verify_receipt(&mut app).is_err() {
                        rejected = true;
                        break;
                    }
                }
                Ok(Action::Import(id)) => {
                    if cutover.import(&mut app, &id).is_err() {
                        rejected = true;
                        break;
                    }
                }
                Ok(
                    action @ (Action::VerifyPage(_)
                    | Action::VerifyImport(_)
                    | Action::Retire(_)
                    | Action::Trust(_)
                    | Action::TrustImport(_)),
                ) => {
                    if cutover.retirement_action(&mut app, action).is_err() {
                        rejected = true;
                        break;
                    }
                }
                Ok(Action::Complete) => break,
            }
        }
        assert!(rejected);
        assert_eq!(
            fs::read(root.join("pages/page.md")).unwrap(),
            b"external owner edit"
        );
        assert!(fs::read_to_string(saved.join("files/pages/page.md"))
            .unwrap()
            .contains("owner statement"));
    }
    #[test]
    fn retirement_and_trusted_record_crashes_resume_without_losing_owner_evidence() {
        for (hook, code) in [
            ("KG_RETIRE_STOP_AFTER_PUBLICATION", 94),
            ("KG_TRUSTED_RETIRE_STOP_AFTER_RECORD", 95),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("collection");
            fs::create_dir(&root).unwrap();
            let root = fs::canonicalize(root).unwrap();
            let saved = retirement_fixture(&root);
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "application::migration_backup::precutover::tests::retirement_crash_child",
                ])
                .env("KG_PRECUTOVER_ROOT", &root)
                .env(hook, "1")
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(code));
            let mut app = Application::open_pending_with_all_providers_and_meaning_assets(
                &root,
                Arc::new(UnavailableProvider),
                Arc::new(LocalExtractor),
                Arc::new(NativeAudioProcessor),
                None::<&Path>,
            )
            .unwrap();
            let mut cutover = Session::new(&root).unwrap().unwrap();
            let mut done = false;
            for _ in 0..10000 {
                match cutover.step(&root).unwrap() {
                    Action::More => (),
                    Action::Import(id) => cutover.import(&mut app, &id).unwrap(),
                    Action::Receipts => {
                        cutover.verify_receipt(&mut app).unwrap();
                    }
                    action @ (Action::VerifyPage(_)
                    | Action::VerifyImport(_)
                    | Action::Retire(_)
                    | Action::Trust(_)
                    | Action::TrustImport(_)) => {
                        cutover.retirement_action(&mut app, action).unwrap()
                    }
                    Action::Complete => {
                        done = true;
                        break;
                    }
                }
            }
            assert!(done);
            let state: MigrationState = serde_json::from_slice(
                &fs::read(root.join(".recovery/migration/state.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(state.phase, "authority_retired");
            assert!(fs::read_to_string(saved.join("files/pages/page.md"))
                .unwrap()
                .contains("owner statement"));
            let (_, entry) = manifest_records::next(
                &cutover
                    .plan
                    .work()
                    .join(".recovery/migration/authored-stage/intents.sqlite"),
                None,
            )
            .unwrap()
            .unwrap();
            assert!(fs::read_to_string(
                app.source_dir(entry.path.to_str().unwrap())
                    .unwrap()
                    .join("original.txt")
            )
            .unwrap()
            .contains("owner statement"));
        }
    }
    #[test]
    fn resumed_precutover_rechecks_and_preserves_owner_source() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        fs::create_dir_all(root.join("pages")).unwrap();
        fs::create_dir_all(root.join(".staging/transactions")).unwrap();
        fs::create_dir_all(root.join("sources")).unwrap();
        let evidence = serde_json::json!({"quote":"source", "byte_start":0,"byte_end":6,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({"schema":1,"page_id":"page-1","source_id":"source-1","title":"Page","kind":"event","evidence":evidence,"facts":[],"owner_content":["owner statement"],"uninterpreted_owner_edits":[]})).unwrap();
        fs::write(
            root.join("pages/page.md"),
            format!("---\n{}---\n", serde_yaml_ng::to_string(&header).unwrap()),
        )
        .unwrap();
        let work = Path::new(".recovery/migration/test-snapshot");
        snapshot_worker::begin(&root, work).unwrap();
        let mut snapshot = snapshot_worker::Session::new();
        let saved = loop {
            if let Some(saved) = snapshot_worker::step(&root, work, &mut snapshot).unwrap() {
                break saved;
            }
        };
        let state = MigrationState {
            backup: saved.file_name().unwrap().to_str().unwrap().into(),
            phase: "backup_verified".into(),
        };
        fs::write(
            root.join(".recovery/migration/state.json"),
            serde_json::to_vec(&state).unwrap(),
        )
        .unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::precutover::tests::precutover_import_child",
            ])
            .env("KG_PRECUTOVER_ROOT", &root)
            .env("KG_AUTHORED_STOP_AFTER_IMPORT", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(93));
        let mut app = Application::open_pending_with_all_providers_and_meaning_assets(
            &root,
            Arc::new(UnavailableProvider),
            Arc::new(LocalExtractor),
            Arc::new(NativeAudioProcessor),
            None::<&Path>,
        )
        .unwrap();
        fs::write(root.join("unrelated-change"), b"external owner bytes").unwrap();
        let mut rejected_session = Session::new(&root).unwrap().unwrap();
        let mut rejected = false;
        for _ in 0..2000 {
            match rejected_session.step(&root) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(Action::More) => (),
                Ok(_) => break,
            }
        }
        assert!(rejected);
        assert_eq!(
            fs::read(root.join("unrelated-change")).unwrap(),
            b"external owner bytes"
        );
        fs::remove_file(root.join("unrelated-change")).unwrap();
        let mut cutover = Session::new(&root).unwrap().unwrap();
        let mut complete = false;
        for _ in 0..2000 {
            match cutover.step(&root).unwrap() {
                Action::More => (),
                Action::Import(id) => cutover.import(&mut app, &id).unwrap(),
                Action::Receipts => {
                    cutover.verify_receipt(&mut app).unwrap();
                }
                action @ (Action::VerifyPage(_)
                | Action::VerifyImport(_)
                | Action::Retire(_)
                | Action::Trust(_)
                | Action::TrustImport(_)) => cutover.retirement_action(&mut app, action).unwrap(),
                Action::Complete => {
                    complete = true;
                    break;
                }
            }
        }
        assert!(complete);
        assert!(root.join("pages/page.md").is_file());
        let (_, entry) = manifest_records::next(
            &cutover
                .plan
                .work()
                .join(".recovery/migration/authored-stage/intents.sqlite"),
            None,
        )
        .unwrap()
        .unwrap();
        let source = app.source_dir(entry.path.to_str().unwrap()).unwrap();
        assert!(fs::read_to_string(source.join("original.txt"))
            .unwrap()
            .contains("owner statement"));
        app.resume_pending_startup().unwrap();
        assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
        assert!(fs::read_to_string(source.join("original.txt"))
            .unwrap()
            .contains("owner statement"));
    }
}
