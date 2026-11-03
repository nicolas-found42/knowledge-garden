//! Disk-backed manifest entries. A row is committed only after its member has
//! been copied and fingerprinted; inserting the same row again is idempotent.
use super::*;
use rusqlite::{params, OptionalExtension};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Entry {
    pub path: PathBuf,
    pub digest: Option<String>,
}

pub(super) fn create(path: &Path) -> Result<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .sync_all()?;
    let db = Connection::open(path)?;
    db.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; CREATE TABLE entries (path BLOB PRIMARY KEY NOT NULL, digest TEXT) WITHOUT ROWID;")?;
    drop(db);
    sync_directory(path.parent().unwrap())
}
fn key(path: &Path) -> Result<Vec<u8>> {
    validate_relative_collection_path(path)?;
    Ok(serde_json::to_vec(path)?)
}
pub(super) fn put(db_path: &Path, path: &Path, digest: Option<&str>) -> Result<()> {
    if let Some(value) = digest {
        if value.len() != 64
            || !value
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(GardenError::Recovery(
                "Manifest entry digest is invalid.".into(),
            ));
        }
    }
    let db = Connection::open(db_path)?;
    db.execute_batch("PRAGMA synchronous=FULL;")?;
    let key = key(path)?;
    let existing: Option<Option<String>> = db
        .query_row(
            "SELECT digest FROM entries WHERE path=?1",
            params![&key],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(existing) = existing {
        if existing.as_deref() != digest {
            return Err(GardenError::Recovery(
                "A manifest entry conflicts with previously saved work.".into(),
            ));
        }
        return Ok(());
    }
    db.execute(
        "INSERT INTO entries(path,digest) VALUES (?1,?2)",
        params![key, digest],
    )?;
    Ok(())
}
/// Replace one expected entry in a freshly reconstructed scratch manifest.
pub(super) fn set(db_path: &Path, path: &Path, digest: Option<&str>) -> Result<()> {
    if digest.is_some_and(|value| {
        value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    }) {
        return Err(GardenError::Recovery(
            "Manifest entry digest is invalid.".into(),
        ));
    }
    let db = Connection::open(db_path)?;
    db.execute_batch("PRAGMA synchronous=FULL;")?;
    db.execute("INSERT INTO entries(path,digest) VALUES (?1,?2) ON CONFLICT(path) DO UPDATE SET digest=excluded.digest", params![key(path)?, digest])?;
    Ok(())
}
pub(super) fn get(db_path: &Path, path: &Path) -> Result<Option<Entry>> {
    let db = Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let digest: Option<Option<String>> = db
        .query_row(
            "SELECT CASE WHEN digest IS NULL OR length(digest)=64 THEN digest ELSE '' END FROM entries WHERE path=?1",
            params![key(path)?],
            |row| row.get(0),
        )
        .optional()?;
    if digest
        .as_ref()
        .and_then(|value| value.as_ref())
        .is_some_and(|value| {
            value.len() != 64
                || !value
                    .bytes()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        })
    {
        return Err(GardenError::Recovery(
            "Manifest entry digest is invalid.".into(),
        ));
    }
    Ok(digest.map(|digest| Entry {
        path: path.to_owned(),
        digest,
    }))
}
/// Keyset pagination returns one entry and never reads the full manifest.
pub(super) fn next(db_path: &Path, after: Option<&[u8]>) -> Result<Option<(Vec<u8>, Entry)>> {
    let db = Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let row: Option<(Vec<u8>, Option<String>)> = db
        .query_row(
            "SELECT CASE WHEN length(path) <= 16384 THEN path ELSE x'' END, CASE WHEN digest IS NULL OR length(digest)=64 THEN digest ELSE '' END FROM entries WHERE path > ?1 ORDER BY path LIMIT 1",
            params![after.unwrap_or(&[])],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    row.map(|(key, digest)| {
        if key.is_empty()
            || digest.as_ref().is_some_and(|value| {
                value.len() != 64
                    || !value
                        .bytes()
                        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            })
        {
            return Err(GardenError::Recovery(
                "Manifest entry is invalid or too large.".into(),
            ));
        }
        let path: PathBuf = serde_json::from_slice(&key)?;
        validate_relative_collection_path(&path)?;
        Ok((key, Entry { path, digest }))
    })
    .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entries_are_idempotent_and_page_one_at_a_time() {
        let temp = tempfile::tempdir().unwrap();
        let db = temp.path().join("manifest.sqlite");
        create(&db).unwrap();
        let digest = "a".repeat(64);
        put(&db, Path::new("one"), Some(&digest)).unwrap();
        put(&db, Path::new("directory"), None).unwrap();
        put(&db, Path::new("one"), Some(&digest)).unwrap();
        assert!(put(&db, Path::new("one"), Some(&"b".repeat(64))).is_err());
        assert!(put(&db, Path::new("../outside"), None).is_err());
        let mut cursor = None;
        let mut entries = Vec::new();
        while let Some((key, entry)) = next(&db, cursor.as_deref()).unwrap() {
            entries.push(entry);
            cursor = Some(key);
        }
        assert_eq!(entries.len(), 2);
        assert_eq!(
            get(&db, Path::new("one")).unwrap().unwrap().digest,
            Some(digest)
        );
    }
}
