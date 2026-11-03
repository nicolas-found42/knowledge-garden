//! Session-authenticated copy of manifest rows as they pass verification.
use super::*;
use rusqlite::{params, OptionalExtension};
use std::sync::Mutex;

type VerifiedRow = (Vec<u8>, PathBuf, Option<String>);
type StoredRow = (Vec<u8>, Vec<u8>, Option<String>, Vec<u8>);

pub(crate) struct VerifiedIndex {
    _work: tempfile::TempDir,
    db: Mutex<Connection>,
    key: [u8; 32],
    chain: Mutex<(Vec<u8>, bool)>,
}
impl VerifiedIndex {
    pub(crate) fn new() -> Result<Self> {
        let work = tempfile::tempdir()?;
        let db = Connection::open(work.path().join("verified.sqlite"))?;
        // This authenticated cache is usable only with its process-held key.
        // A crash discards both; durable snapshots and manifests are separate.
        db.execute_batch("PRAGMA journal_mode=MEMORY; PRAGMA synchronous=OFF; CREATE TABLE entries(path BLOB PRIMARY KEY, previous BLOB NOT NULL, digest TEXT, tag BLOB NOT NULL) WITHOUT ROWID")?;
        let mut key = [0; 32];
        key[..16].copy_from_slice(Uuid::new_v4().as_bytes());
        key[16..].copy_from_slice(Uuid::new_v4().as_bytes());
        Ok(Self {
            _work: work,
            db: Mutex::new(db),
            key,
            chain: Mutex::new((Vec::new(), false)),
        })
    }
    fn tag(&self, previous: &[u8], path: &[u8], digest: Option<&str>) -> [u8; 32] {
        let mut inner = [0x36; 64];
        let mut outer = [0x5c; 64];
        for i in 0..32 {
            inner[i] ^= self.key[i];
            outer[i] ^= self.key[i];
        }
        let mut h = Sha256::new();
        h.update(inner);
        h.update(b"knowledge-garden-verified-manifest-row-v1\0");
        h.update((previous.len() as u64).to_be_bytes());
        h.update(previous);
        h.update((path.len() as u64).to_be_bytes());
        h.update(path);
        h.update([u8::from(digest.is_some())]);
        if let Some(value) = digest {
            h.update(value.as_bytes());
        }
        let mut result = Sha256::new();
        result.update(outer);
        result.update(h.finalize());
        result.finalize().into()
    }
    pub(crate) fn put(&self, path: &Path, digest: Option<&str>) -> Result<()> {
        validate_relative_collection_path(path)?;
        let key = serde_json::to_vec(path)?;
        let mut chain = self.chain.lock().unwrap();
        if chain.1 || (!chain.0.is_empty() && chain.0 >= key) {
            return Err(GardenError::Recovery(
                "Verified manifest rows are not ordered.".into(),
            ));
        }
        let tag = self.tag(&chain.0, &key, digest);
        self.db.lock().unwrap().execute(
            "INSERT INTO entries(path,previous,digest,tag) VALUES (?1,?2,?3,?4)",
            params![&key, &chain.0, digest, tag.as_slice()],
        )?;
        chain.0 = key;
        Ok(())
    }
    pub(crate) fn seal(&self) -> Result<()> {
        let mut chain = self.chain.lock().unwrap();
        if chain.1 {
            return Err(GardenError::Recovery(
                "Verified manifest index was already sealed.".into(),
            ));
        }
        chain.1 = true;
        Ok(())
    }
    fn decode(
        &self,
        key: Vec<u8>,
        previous: Vec<u8>,
        digest: Option<String>,
        tag: Vec<u8>,
    ) -> Result<(Vec<u8>, PathBuf, Option<String>)> {
        if key.is_empty()
            || key.len() > 16384
            || previous.len() > 16384
            || self.tag(&previous, &key, digest.as_deref()).as_slice() != tag
        {
            return Err(GardenError::Recovery(
                "Verified manifest index changed.".into(),
            ));
        }
        let path: PathBuf = serde_json::from_slice(&key)?;
        validate_relative_collection_path(&path)?;
        Ok((key, path, digest))
    }
    pub(crate) fn next(&self, after: Option<&[u8]>) -> Result<Option<VerifiedRow>> {
        let (last, sealed) = self.chain.lock().unwrap().clone();
        if !sealed {
            return Err(GardenError::Recovery(
                "Verified manifest index is incomplete.".into(),
            ));
        }
        let db = self.db.lock().unwrap();
        let row: Option<StoredRow> = db.query_row("SELECT CASE WHEN length(path)<=16384 THEN path ELSE x'' END, CASE WHEN length(previous)<=16384 THEN previous ELSE x'ff' END, CASE WHEN digest IS NULL OR length(digest)=64 THEN digest ELSE '' END, CASE WHEN length(tag)=32 THEN tag ELSE x'' END FROM entries WHERE path>?1 ORDER BY path LIMIT 1", params![after.unwrap_or(&[])], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        match row {
            Some((key, previous, digest, tag)) => {
                if previous.as_slice() != after.unwrap_or(&[]) {
                    return Err(GardenError::Recovery(
                        "Verified manifest index has a missing row.".into(),
                    ));
                }
                Ok(Some(self.decode(key, previous, digest, tag)?))
            }
            None if last.as_slice() == after.unwrap_or(&[]) => Ok(None),
            None => Err(GardenError::Recovery(
                "Verified manifest index has a missing final row.".into(),
            )),
        }
    }
    pub(crate) fn get(&self, path: &Path) -> Result<Option<Option<String>>> {
        validate_relative_collection_path(path)?;
        let key = serde_json::to_vec(path)?;
        let db = self.db.lock().unwrap();
        let row: Option<(Vec<u8>,Option<String>,Vec<u8>)> = db.query_row("SELECT CASE WHEN length(previous)<=16384 THEN previous ELSE x'ff' END, CASE WHEN digest IS NULL OR length(digest)=64 THEN digest ELSE '' END,CASE WHEN length(tag)=32 THEN tag ELSE x'' END FROM entries WHERE path=?1",params![&key],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        row.map(|(previous, digest, tag)| {
            self.decode(key, previous, digest, tag)
                .map(|(_, _, value)| value)
        })
        .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn omitted_rows_and_truncated_tail_are_rejected() {
        for removed in ["a", "b"] {
            let index = VerifiedIndex::new().unwrap();
            index.put(Path::new("a"), Some(&"a".repeat(64))).unwrap();
            index.put(Path::new("b"), Some(&"b".repeat(64))).unwrap();
            index.seal().unwrap();
            let key = serde_json::to_vec(Path::new(removed)).unwrap();
            index
                .db
                .lock()
                .unwrap()
                .execute("DELETE FROM entries WHERE path=?1", params![key])
                .unwrap();
            let mut after = None;
            let mut rejected = false;
            for _ in 0..3 {
                match index.next(after.as_deref()) {
                    Err(_) => {
                        rejected = true;
                        break;
                    }
                    Ok(Some((key, _, _))) => after = Some(key),
                    Ok(None) => break,
                }
            }
            assert!(rejected);
        }
    }
}
