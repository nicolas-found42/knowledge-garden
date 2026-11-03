//! Incremental final verification of the two recovery snapshots. A process
//! restart begins verification again rather than trusting old completion state.
use super::*;

pub(crate) struct Proof {
    pre: String,
    post: Option<String>,
}
impl Proof {
    pub(crate) fn pre(&self) -> &str {
        &self.pre
    }
    pub(crate) fn post(&self) -> Option<&str> {
        self.post.as_deref()
    }
}
enum Verifier {
    Legacy(Box<legacy_verify::Session>),
    V3 {
        work: tempfile::TempDir,
        session: Box<incremental_verify::Session>,
    },
}
pub(crate) struct Session {
    root: PathBuf,
    pre: String,
    post: Option<String>,
    phase: u8,
    current: Option<Verifier>,
}
fn read_file(root: &Path, path: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bounded_copy::source_file(root, Path::new(path))?
        .take(4097)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(GardenError::Recovery(
            "Recovery journal is too large.".into(),
        ));
    }
    Ok(bytes)
}
fn valid(id: &str) -> bool {
    id.starts_with("backup-") && Path::new(id).components().count() == 1
}
pub(crate) fn eligible(root: &Path) -> Result<bool> {
    let state: MigrationState =
        serde_json::from_slice(&read_file(root, ".recovery/migration/state.json")?)?;
    if !valid(&state.backup)
        || ![
            "backup_verified",
            "evidence_imported",
            "cutover_preparing",
            "authority_retired",
        ]
        .contains(&state.phase.as_str())
    {
        return Err(GardenError::Recovery("Migration state is invalid.".into()));
    }
    Ok(state.phase == "authority_retired")
}
impl Session {
    pub(crate) fn new(root: &Path, include_post: bool) -> Result<Self> {
        let pre: String =
            serde_json::from_slice(&read_file(root, ".recovery/migration/pre-recovery.json")?)?;
        if !valid(&pre) {
            return Err(GardenError::Recovery(
                "Pre-recovery identity is invalid.".into(),
            ));
        }
        let state: MigrationState =
            serde_json::from_slice(&read_file(root, ".recovery/migration/state.json")?)?;
        if !valid(&state.backup) || state.phase != "authority_retired" {
            return Err(GardenError::Recovery(
                "Migration authority has not been retired.".into(),
            ));
        }
        Ok(Self {
            root: root.to_owned(),
            pre,
            post: include_post.then_some(state.backup),
            phase: 0,
            current: None,
        })
    }
    fn check_journals(&self) -> Result<()> {
        let pre: String = serde_json::from_slice(&read_file(
            &self.root,
            ".recovery/migration/pre-recovery.json",
        )?)?;
        if pre != self.pre {
            return Err(GardenError::Recovery(
                "Pre-recovery journal changed during verification.".into(),
            ));
        }
        if let Some(expected) = &self.post {
            let state: MigrationState =
                serde_json::from_slice(&read_file(&self.root, ".recovery/migration/state.json")?)?;
            if state.phase != "authority_retired" || &state.backup != expected {
                return Err(GardenError::Recovery(
                    "Migration state changed during verification.".into(),
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn step(&mut self) -> Result<Option<Proof>> {
        self.check_journals()?;
        let id = match self.phase {
            0 => &self.pre,
            1 => match &self.post {
                Some(id) => id,
                None => {
                    self.phase = 2;
                    return Ok(None);
                }
            },
            2 => {
                return Ok(Some(Proof {
                    pre: self.pre.clone(),
                    post: self.post.clone(),
                }))
            }
            _ => {
                return Err(GardenError::Recovery(
                    "Final recovery verification state is invalid.".into(),
                ))
            }
        };
        let saved = self.root.join(".recovery/migration").join(id);
        if self.current.is_none() {
            self.current = Some(match fs::symlink_metadata(saved.join("manifest.sqlite")) {
                Ok(m) if m.is_file() => {
                    let work = tempfile::tempdir()?;
                    incremental_verify::begin(
                        &saved.join("files"),
                        &saved.join("manifest.sqlite"),
                        work.path(),
                    )?;
                    Verifier::V3 {
                        work,
                        session: Box::new(incremental_verify::Session::ephemeral()),
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    Verifier::Legacy(Box::new(legacy_verify::Session::new(&saved)?))
                }
                Err(e) => return Err(e.into()),
                Ok(_) => {
                    return Err(GardenError::Recovery(
                        "Recovery manifest is not a regular file.".into(),
                    ))
                }
            });
            return Ok(None);
        }
        let complete = match self.current.as_mut().unwrap() {
            Verifier::Legacy(session) => legacy_verify::step(&saved, session)?,
            Verifier::V3 { work, session } => incremental_verify::step(
                &saved.join("files"),
                &saved.join("manifest.sqlite"),
                work.path(),
                session,
            )?,
        };
        if complete {
            self.current = None;
            self.phase += 1;
        }
        Ok(None)
    }
}
