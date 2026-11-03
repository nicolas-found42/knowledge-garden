//! Rebuild a cutover plan from the verified snapshot on every process start.
//! Persisted exclusion indexes are never accepted as authorization.
use super::*;

pub(super) struct Session {
    work: tempfile::TempDir,
    verification: incremental_verify::Session,
    reference: Option<tempfile::TempDir>,
    phase: u8,
    after: Option<Vec<u8>>,
    receipt_after: Option<Vec<u8>>,
    label_after: Option<String>,
}
impl Session {
    pub(super) fn new(saved: &Path) -> Result<Self> {
        let work = tempfile::tempdir()?;
        incremental_verify::begin(
            &saved.join("files"),
            &saved.join("manifest.sqlite"),
            work.path(),
        )?;
        Ok(Self {
            work,
            verification: incremental_verify::Session::ephemeral(),
            reference: None,
            phase: 0,
            after: None,
            receipt_after: None,
            label_after: None,
        })
    }
    pub(super) fn work(&self) -> &Path {
        self.work.path()
    }
    pub(super) fn recheck_indexes(&mut self) -> Result<()> {
        if self.phase != 6 {
            return Err(GardenError::Recovery(
                "Authored cutover plan is not verified.".into(),
            ));
        }
        self.phase = 3;
        self.after = None;
        self.label_after = None;
        Ok(())
    }
}
fn database(work: &Path, name: &str) -> PathBuf {
    work.join(".recovery/migration/authored-stage").join(name)
}
/// One snapshot verification batch, one staged page, or one index row.
pub(super) fn step(saved: &Path, session: &mut Session) -> Result<bool> {
    let work = session.work.path();
    match session.phase {
        0 => {
            if incremental_verify::step(
                &saved.join("files"),
                &saved.join("manifest.sqlite"),
                work,
                &mut session.verification,
            )? {
                authored_stage::begin(saved, work)?;
                session.phase = 1;
            }
            Ok(false)
        }
        1 => {
            if authored_stage::step(saved, work)? {
                let reference = tempfile::tempdir()?;
                authored_stage::begin(saved, reference.path())?;
                session.reference = Some(reference);
                session.phase = 2;
            }
            Ok(false)
        }
        2 => {
            if authored_stage::step(saved, session.reference.as_ref().unwrap().path())? {
                session.phase = 3;
                session.after = None;
            }
            Ok(false)
        }
        3 | 4 => {
            let name = if session.phase == 3 {
                "ignored.sqlite"
            } else {
                "intents.sqlite"
            };
            let reference = session.reference.as_ref().unwrap().path();
            let actual = manifest_records::next(&database(work, name), session.after.as_deref())?;
            let expected =
                manifest_records::next(&database(reference, name), session.after.as_deref())?;
            match (actual, expected) {
                (None, None) => {
                    if session.phase == 3 {
                        session.phase = 4;
                        session.after = None;
                        Ok(false)
                    } else {
                        session.phase = 5;
                        session.label_after = None;
                        Ok(false)
                    }
                }
                (Some((key, entry)), Some((expected_key, expected_entry)))
                    if key == expected_key && entry == expected_entry =>
                {
                    if session.phase == 3
                        && manifest_records::get(&saved.join("manifest.sqlite"), &entry.path)?
                            .is_some()
                    {
                        return Err(GardenError::Recovery(
                            "A planned source collides with the saved collection.".into(),
                        ));
                    }
                    session.after = Some(key);
                    Ok(false)
                }
                _ => Err(GardenError::Recovery(
                    "Staged source exclusions or intents differ from reconstructed evidence."
                        .into(),
                )),
            }
        }
        5 => {
            let reference = session.reference.as_ref().unwrap().path();
            let actual = label_catalog::next(
                &database(work, "labels.sqlite"),
                session.label_after.as_deref(),
            )?;
            let expected = label_catalog::next(
                &database(reference, "labels.sqlite"),
                session.label_after.as_deref(),
            )?;
            match (actual, expected) {
                (None, None) => {
                    session.phase = 6;
                    Ok(true)
                }
                (Some((id, title)), Some((expected_id, expected_title)))
                    if id == expected_id && title == expected_title =>
                {
                    session.label_after = Some(id);
                    Ok(false)
                }
                _ => Err(GardenError::Recovery(
                    "Page labels differ from reconstructed snapshot evidence.".into(),
                )),
            }
        }
        6 => Ok(true),
        _ => Err(GardenError::Recovery(
            "Authored cutover plan phase is invalid.".into(),
        )),
    }
}

/// Revalidate one prior import receipt and deterministic source bundle per
/// turn. A new session starts at the first receipt after a process restart.
pub(super) fn verify_receipts_step(
    app: &mut Application,
    saved: &Path,
    session: &mut Session,
    receipt_work: &Path,
) -> Result<bool> {
    if session.phase != 6 {
        return Err(GardenError::Recovery(
            "Authored cutover plan is not verified.".into(),
        ));
    }
    let Some((key, entry)) = manifest_records::next(
        &database(session.work.path(), "intents.sqlite"),
        session.receipt_after.as_deref(),
    )?
    else {
        return Ok(true);
    };
    let id = entry
        .path
        .to_str()
        .ok_or_else(|| GardenError::Recovery("Authored source identity is invalid.".into()))?;
    if entry.digest.is_none() {
        return Err(GardenError::Recovery(
            "Authored source intent has no content digest.".into(),
        ));
    }
    authored_stage::verify_receipt(app, saved, session.work.path(), receipt_work, id)?;
    session.receipt_after = Some(key);
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_receipts_are_revalidated_after_rebuilding_the_plan() {
        let temp = tempfile::tempdir().unwrap();
        let saved = temp.path().join("saved");
        let receipts = temp.path().join("receipts");
        fs::create_dir_all(saved.join("files/pages")).unwrap();
        fs::create_dir(&receipts).unwrap();
        let evidence = serde_json::json!({"quote":"source", "byte_start":0,"byte_end":6,"line_start":1,"line_end":1,"origin":"source","qualifier":null});
        let header: KnowledgePageHeader = serde_json::from_value(serde_json::json!({"schema":1,"page_id":"page-1","source_id":"source-1","title":"Page","kind":"event","evidence":evidence,"facts":[],"owner_content":["historical owner statement"],"uninterpreted_owner_edits":[]})).unwrap();
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
        let mut first = Session::new(&saved).unwrap();
        for _ in 0..200 {
            if step(&saved, &mut first).unwrap() {
                break;
            }
        }
        assert_eq!(first.phase, 6);
        let (_, entry) = manifest_records::next(&database(first.work(), "intents.sqlite"), None)
            .unwrap()
            .unwrap();
        let id = entry.path.to_str().unwrap().to_owned();
        let mut app = Application::open(temp.path().join("collection")).unwrap();
        authored_stage::import_one_with_receipts(&mut app, &saved, first.work(), &receipts, &id)
            .unwrap();
        let mut rebuilt = Session::new(&saved).unwrap();
        for _ in 0..200 {
            if step(&saved, &mut rebuilt).unwrap() {
                break;
            }
        }
        assert!(!verify_receipts_step(&mut app, &saved, &mut rebuilt, &receipts).unwrap());
        assert!(verify_receipts_step(&mut app, &saved, &mut rebuilt, &receipts).unwrap());
        fs::write(
            receipts
                .join(".recovery/migration/authored-stage/receipts")
                .join(format!("{id}.json")),
            b"forged",
        )
        .unwrap();
        let mut restarted = Session::new(&saved).unwrap();
        for _ in 0..200 {
            if step(&saved, &mut restarted).unwrap() {
                break;
            }
        }
        assert!(verify_receipts_step(&mut app, &saved, &mut restarted, &receipts).is_err());
    }
    #[test]
    fn forged_matching_exclusion_and_intent_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let saved = temp.path().join("saved");
        fs::create_dir_all(saved.join("files")).unwrap();
        manifest_records::create(&saved.join("manifest.sqlite")).unwrap();
        let mut plan = Session::new(&saved).unwrap();
        for _ in 0..100 {
            if plan.phase == 2 {
                break;
            }
            step(&saved, &mut plan).unwrap();
        }
        assert_eq!(plan.phase, 2);
        let id = "a".repeat(64);
        manifest_records::put(
            &database(plan.work(), "ignored.sqlite"),
            &PathBuf::from("sources").join(&id),
            None,
        )
        .unwrap();
        manifest_records::put(
            &database(plan.work(), "intents.sqlite"),
            Path::new(&format!("source-{id}")),
            Some(&"b".repeat(64)),
        )
        .unwrap();
        let mut rejected = false;
        for _ in 0..100 {
            match step(&saved, &mut plan) {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(true) => break,
                Ok(false) => (),
            }
        }
        assert!(rejected);
    }
}
