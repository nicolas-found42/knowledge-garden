//! Locate the digest of a pending transaction manifest in the verified
//! pre-recovery snapshot. Legacy arrays are read one record per turn.
use super::*;
use sha2::Digest as ShaDigest;
use std::os::unix::fs::MetadataExt;
#[derive(Clone)]
pub(crate) enum Expected {
    Chain(String),
    Legacy(String),
}
impl Expected {
    pub(crate) fn verify_bytes(&self, bytes: &[u8]) -> Result<()> {
        let mut digest = Digest::new(bytes.len() as u64);
        for byte in bytes {
            digest.push(*byte);
        }
        digest.verify(self)
    }
}
pub(crate) struct FileVerifier {
    root: PathBuf,
    relative: PathBuf,
    file: File,
    identity: (u64, u64, u64, i64, i64),
    offset: u64,
    digest: Option<Digest>,
    expected: Expected,
}
fn identity(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}
impl FileVerifier {
    pub(crate) fn identity(&self) -> (u64, u64, u64, i64, i64) {
        self.identity
    }
    pub(crate) fn new(root: &Path, relative: &Path, expected: Expected) -> Result<Self> {
        let file = bounded_copy::source_file(root, relative)?;
        let identity = identity(&file.metadata()?);
        Ok(Self {
            root: root.into(),
            relative: relative.into(),
            file,
            identity,
            offset: 0,
            digest: Some(Digest::new(identity.2)),
            expected,
        })
    }
    pub(crate) fn step(&mut self) -> Result<bool> {
        match self.step_match()? {
            None => Ok(false),
            Some(true) => Ok(true),
            Some(false) => Err(GardenError::Recovery(
                "Staged publication differs from the verified recovery snapshot.".into(),
            )),
        }
    }
    pub(crate) fn step_match(&mut self) -> Result<Option<bool>> {
        if identity(&self.file.metadata()?) != self.identity
            || identity(&bounded_copy::source_file(&self.root, &self.relative)?.metadata()?)
                != self.identity
        {
            return Err(GardenError::Recovery(
                "Staged publication changed during verification.".into(),
            ));
        }
        let count = (self.identity.2 - self.offset).min(manifest_cursor::BLOCK as u64) as usize;
        if count != 0 {
            let mut block = vec![0; count];
            self.file.read_exact(&mut block)?;
            for byte in block {
                self.digest.as_mut().unwrap().push(byte);
            }
            self.offset += count as u64;
            if identity(&self.file.metadata()?) != self.identity
                || identity(&bounded_copy::source_file(&self.root, &self.relative)?.metadata()?)
                    != self.identity
            {
                return Err(GardenError::Recovery(
                    "Staged publication changed during verification.".into(),
                ));
            }
        }
        if self.offset == self.identity.2 {
            Ok(Some(self.digest.take().unwrap().matches(&self.expected)?))
        } else {
            Ok(None)
        }
    }
}
pub(crate) struct Session {
    saved: PathBuf,
    wanted: PathBuf,
    offset: u64,
    v3: bool,
    verified: Option<std::sync::Arc<verified_index::VerifiedIndex>>,
}
impl Session {
    pub(crate) fn new_with_index(
        root: &Path,
        wanted: &Path,
        verified: Option<std::sync::Arc<verified_index::VerifiedIndex>>,
    ) -> Result<Option<Self>> {
        validate_relative_collection_path(wanted)?;
        let journal = Path::new(".recovery/migration/pre-recovery.json");
        match fs::symlink_metadata(root.join(journal)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
            Ok(m) if m.file_type().is_file() => (),
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Pre-recovery journal is not a regular file.".into(),
                ))
            }
        }
        let id: String = serde_json::from_slice(&read_bounded_nofollow(root, journal, 4096)?)?;
        if !id.starts_with("backup-") || Path::new(&id).components().count() != 1 {
            return Err(GardenError::Recovery(
                "Replay backup identity is invalid.".into(),
            ));
        }
        let saved = root.join(".recovery/migration").join(id);
        let db = saved.join("manifest.sqlite");
        let v3 = match fs::symlink_metadata(&db) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e.into()),
            Ok(m) if m.file_type().is_file() => true,
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Replay manifest index is not a regular file.".into(),
                ))
            }
        };
        Ok(Some(Self {
            saved,
            wanted: wanted.to_owned(),
            offset: 0,
            v3,
            verified,
        }))
    }
    pub(crate) fn step(&mut self) -> Result<Option<Expected>> {
        if let Some(index) = &self.verified {
            let digest = index
                .get(&self.wanted)?
                .ok_or_else(|| {
                    GardenError::Recovery(
                        "Transaction manifest is absent from the verified snapshot.".into(),
                    )
                })?
                .ok_or_else(|| {
                    GardenError::Recovery("Transaction manifest snapshot is not a file.".into())
                })?;
            return Ok(Some(if self.v3 {
                Expected::Chain(digest)
            } else {
                Expected::Legacy(digest)
            }));
        }
        if self.v3 {
            let digest = if let Some(index) = &self.verified {
                index.get(&self.wanted)?
            } else {
                manifest_records::get(&self.saved.join("manifest.sqlite"), &self.wanted)?
                    .map(|entry| entry.digest)
            }
            .ok_or_else(|| {
                GardenError::Recovery(
                    "Transaction manifest is absent from the recovery snapshot.".into(),
                )
            })?;
            return Ok(Some(Expected::Chain(digest.ok_or_else(|| {
                GardenError::Recovery("Transaction manifest snapshot is not a file.".into())
            })?)));
        }
        match legacy_manifest::next(&self.saved.join("manifest.json"), self.offset)? {
            None => Err(GardenError::Recovery(
                "Transaction manifest is absent from the legacy recovery snapshot.".into(),
            )),
            Some((next, path, digest)) => {
                self.offset = next;
                if path == self.wanted {
                    Ok(Some(Expected::Legacy(digest.ok_or_else(|| {
                        GardenError::Recovery("Transaction manifest snapshot is not a file.".into())
                    })?)))
                } else {
                    Ok(None)
                }
            }
        }
    }
}
pub(crate) struct Digest {
    chain: [u8; 32],
    sha: Sha256,
    block: Vec<u8>,
    offset: u64,
    length: u64,
}
impl Digest {
    pub(crate) fn new(length: u64) -> Self {
        Self {
            chain: manifest_cursor::initial(length),
            sha: Sha256::new(),
            block: Vec::with_capacity(manifest_cursor::BLOCK),
            offset: 0,
            length,
        }
    }
    pub(crate) fn push(&mut self, byte: u8) {
        self.block.push(byte);
        if self.block.len() == manifest_cursor::BLOCK {
            self.flush();
        }
    }
    fn flush(&mut self) {
        self.sha.update(&self.block);
        self.chain = manifest_cursor::advance(self.chain, self.offset, &self.block);
        self.offset += self.block.len() as u64;
        self.block.clear();
    }
    pub(crate) fn verify(self, expected: &Expected) -> Result<()> {
        if !self.matches(expected)? {
            return Err(GardenError::Recovery(
                "Publication manifest differs from the verified recovery snapshot.".into(),
            ));
        }
        Ok(())
    }
    fn matches(mut self, expected: &Expected) -> Result<bool> {
        if !self.block.is_empty() {
            self.flush();
        }
        if self.offset != self.length {
            return Err(GardenError::Recovery(
                "Publication manifest length changed during scanning.".into(),
            ));
        }
        let actual = match expected {
            Expected::Chain(_) => manifest_cursor::finish(self.chain, self.length),
            Expected::Legacy(_) => hex::encode(self.sha.finalize()),
        };
        let wanted = match expected {
            Expected::Chain(value) | Expected::Legacy(value) => value,
        };
        Ok(actual == *wanted)
    }
}

/// One-row-at-a-time enumeration of the verified pre-recovery manifest.
pub(crate) struct SnapshotRows {
    saved: PathBuf,
    after: Option<Vec<u8>>,
    offset: u64,
    v3: bool,
    verified: Option<std::sync::Arc<verified_index::VerifiedIndex>>,
}
impl SnapshotRows {
    pub(crate) fn new(
        root: &Path,
        verified: Option<std::sync::Arc<verified_index::VerifiedIndex>>,
    ) -> Result<Self> {
        let journal = Path::new(".recovery/migration/pre-recovery.json");
        let id: String = serde_json::from_slice(&read_bounded_nofollow(root, journal, 4096)?)?;
        if !id.starts_with("backup-") || Path::new(&id).components().count() != 1 {
            return Err(GardenError::Recovery(
                "Replay backup identity is invalid.".into(),
            ));
        }
        let saved = PathBuf::from(".recovery/migration").join(id);
        let v3 = match fs::symlink_metadata(root.join(&saved).join("manifest.sqlite")) {
            Ok(metadata) if metadata.file_type().is_file() => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Replay manifest index is not a regular file.".into(),
                ))
            }
            Err(error) => return Err(error.into()),
        };
        Ok(Self {
            saved,
            after: None,
            offset: 0,
            v3,
            verified,
        })
    }
    pub(crate) fn file(&self, path: &Path) -> PathBuf {
        self.saved.join("files").join(path)
    }
    pub(crate) fn next(&mut self, root: &Path) -> Result<Option<(PathBuf, Option<Expected>)>> {
        if let Some(index) = &self.verified {
            let Some((key, path, digest)) = index.next(self.after.as_deref())? else {
                return Ok(None);
            };
            self.after = Some(key);
            return Ok(Some((
                path,
                digest.map(|value| {
                    if self.v3 {
                        Expected::Chain(value)
                    } else {
                        Expected::Legacy(value)
                    }
                }),
            )));
        }
        if self.v3 {
            let Some((key, entry)) = manifest_records::next(
                &root.join(&self.saved).join("manifest.sqlite"),
                self.after.as_deref(),
            )?
            else {
                return Ok(None);
            };
            self.after = Some(key);
            Ok(Some((entry.path, entry.digest.map(Expected::Chain))))
        } else {
            let Some((offset, path, digest)) =
                legacy_manifest::next(&root.join(&self.saved).join("manifest.json"), self.offset)?
            else {
                return Ok(None);
            };
            self.offset = offset;
            Ok(Some((path, digest.map(Expected::Legacy))))
        }
    }
}
