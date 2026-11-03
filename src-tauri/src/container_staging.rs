//! Durable decoded-part chunks. This stage never grants content authority.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

const MAX_CHUNK: usize = 65536;
const MAX_RECORD: u64 = 100 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionKind {
    EmailMimePart,
    ZipMember,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Identity {
    pub source_id: String,
    pub version: String,
    pub generation: String,
    pub kind: ProjectionKind,
    pub locator: String,
}
#[derive(Serialize, Deserialize)]
struct Record {
    identity: Identity,
    offset: u64,
    data: String,
}
#[derive(Debug)]
pub struct CompletedProjection {
    pub path: PathBuf,
    pub sha256: String,
    pub length: u64,
}
pub struct PartStore {
    dir: PathBuf,
    identity: Identity,
    offset: u64,
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn record_path(dir: &Path, offset: u64) -> PathBuf {
    dir.join(format!("{offset:020}.json"))
}
fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}
fn read_record(path: &Path, identity: &Identity, offset: u64) -> io::Result<Vec<u8>> {
    let mut f = File::open(path)?;
    if f.metadata()?.len() > MAX_RECORD {
        return Err(invalid("Decoded part record is too large"));
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut f)
        .take(MAX_RECORD + 1)
        .read_to_end(&mut bytes)?;
    let record: Record =
        serde_json::from_slice(&bytes).map_err(|_| invalid("Decoded part record is corrupt"))?;
    if record.identity != *identity || record.offset != offset {
        return Err(invalid("Decoded part record identity does not match"));
    }
    let data = STANDARD
        .decode(record.data)
        .map_err(|_| invalid("Decoded part record is corrupt"))?;
    if data.is_empty() || data.len() > MAX_CHUNK {
        return Err(invalid("Decoded part record has invalid length"));
    }
    Ok(data)
}
fn identity_dir(root: &Path, identity: &Identity) -> io::Result<PathBuf> {
    if identity.source_id.is_empty()
        || identity.version.len() != 64
        || !identity
            .version
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || identity.generation.is_empty()
        || identity.locator.is_empty()
        || identity.locator.len() > 4096
    {
        return Err(invalid("Invalid decoded part identity"));
    }
    let encoded =
        serde_json::to_vec(&identity).map_err(|_| invalid("Invalid decoded part identity"))?;
    Ok(root.join(hex::encode(Sha256::digest(encoded))))
}

/// Resolve only the immutable decoded projection belonging to this exact identity.
/// The enclosing job must independently fence the active source version and generation.
pub fn verified_projection(
    root: &Path,
    identity: &Identity,
    sha256: &str,
    supplied: &Path,
) -> io::Result<PathBuf> {
    if sha256.len() != 64
        || !sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(invalid("Invalid decoded projection digest"));
    }
    let dir = identity_dir(root, identity)?;
    if fs::symlink_metadata(&dir)?.file_type().is_symlink() {
        return Err(invalid("Decoded part staging directory is a symbolic link"));
    }
    let expected = dir.join(format!("projection-{sha256}.txt"));
    if fs::symlink_metadata(&expected)?.file_type().is_symlink()
        || fs::symlink_metadata(supplied)?.file_type().is_symlink()
    {
        return Err(invalid("Decoded projection is a symbolic link"));
    }
    let expected = expected.canonicalize()?;
    if expected.parent() != Some(dir.canonicalize()?.as_path())
        || supplied.canonicalize()? != expected
    {
        return Err(invalid("Decoded projection does not match its identity"));
    }
    let mut file = File::open(&expected)?;
    let mut hash = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    if hex::encode(hash.finalize()) != sha256 {
        return Err(invalid("Decoded projection hash does not match"));
    }
    Ok(expected)
}

impl PartStore {
    /// The caller owns the source lock and must use a derived-data root outside active search.
    pub fn open(root: &Path, identity: Identity) -> io::Result<Self> {
        let dir = identity_dir(root, &identity)?;
        fs::create_dir_all(&dir)?;
        if fs::symlink_metadata(&dir)?.file_type().is_symlink() {
            return Err(invalid("Decoded part staging path is a symbolic link"));
        }
        let mut offset = 0u64;
        loop {
            match read_record(&record_path(&dir, offset), &identity, offset) {
                Ok(bytes) => {
                    offset = offset
                        .checked_add(bytes.len() as u64)
                        .ok_or_else(|| invalid("Decoded offset overflow"))?
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => break,
                Err(e) => return Err(e),
            }
        }
        Ok(Self {
            dir,
            identity,
            offset,
        })
    }
    pub fn offset(&self) -> u64 {
        self.offset
    }
    /// `check_fence` must confirm the active source version and generation while the source lock is held.
    pub fn commit(
        &mut self,
        offset: u64,
        data: &[u8],
        check_fence: impl Fn() -> io::Result<()>,
    ) -> io::Result<()> {
        check_fence()?;
        if data.is_empty() || data.len() > MAX_CHUNK {
            return Err(invalid("Invalid decoded part chunk length"));
        }
        if offset != self.offset {
            return Err(invalid("Decoded part chunks must be contiguous"));
        }
        let record = Record {
            identity: self.identity.clone(),
            offset,
            data: STANDARD.encode(data),
        };
        let bytes = serde_json::to_vec(&record)
            .map_err(|_| invalid("Cannot encode decoded part record"))?;
        if bytes.len() as u64 > MAX_RECORD {
            return Err(invalid("Decoded part record is too large"));
        }
        let path = record_path(&self.dir, offset);
        let mut temp = tempfile::NamedTempFile::new_in(&self.dir)?;
        temp.write_all(&bytes)?;
        temp.as_file().sync_all()?;
        check_fence()?;
        match temp.persist_noclobber(&path) {
            Ok(_) => sync_dir(&self.dir)?,
            Err(e)
                if e.error.kind() == io::ErrorKind::AlreadyExists
                    && read_record(&path, &self.identity, offset)? == data => {}
            Err(e) => return Err(e.error),
        }
        self.offset = self
            .offset
            .checked_add(data.len() as u64)
            .ok_or_else(|| invalid("Decoded offset overflow"))?;
        Ok(())
    }
    /// Called only after the helper reports END. Incomplete or invalid UTF-8 is never published.
    pub fn finish(
        &self,
        total: u64,
        check_fence: impl Fn() -> io::Result<()>,
    ) -> io::Result<CompletedProjection> {
        check_fence()?;
        if total == 0 || self.offset != total {
            return Err(invalid("Decoded part is empty or incomplete"));
        }
        let mut temp = tempfile::NamedTempFile::new_in(&self.dir)?;
        let mut hash = Sha256::new();
        let mut offset = 0u64;
        let mut carry = Vec::new();
        while offset < total {
            let data = read_record(&record_path(&self.dir, offset), &self.identity, offset)?;
            offset += data.len() as u64;
            if offset > total {
                return Err(invalid("Decoded part exceeds its declared length"));
            }
            temp.write_all(&data)?;
            hash.update(&data);
            carry.extend_from_slice(&data);
            match std::str::from_utf8(&carry) {
                Ok(_) => carry.clear(),
                Err(e) if e.error_len().is_none() => {
                    let trailing = carry.split_off(e.valid_up_to());
                    carry = trailing;
                }
                Err(_) => return Err(invalid("Decoded part is not valid UTF-8")),
            }
        }
        if !carry.is_empty() {
            return Err(invalid("Decoded part ends inside a UTF-8 character"));
        }
        temp.as_file().sync_all()?;
        check_fence()?;
        let sha256 = hex::encode(hash.finalize());
        let path = self.dir.join(format!("projection-{sha256}.txt"));
        match temp.persist_noclobber(&path) {
            Ok(_) => sync_dir(&self.dir)?,
            Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => {
                let mut existing = File::open(&path)?;
                let mut existing_hash = Sha256::new();
                let mut buf = [0; 65536];
                loop {
                    let n = existing.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    existing_hash.update(&buf[..n]);
                }
                if hex::encode(existing_hash.finalize()) != sha256 {
                    return Err(invalid("Stored projection differs from its identity"));
                }
            }
            Err(e) => return Err(e.error),
        }
        Ok(CompletedProjection {
            path,
            sha256,
            length: total,
        })
    }
}
