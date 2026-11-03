//! Disk-backed page labels used by bounded retirement rendering.
use super::*;
use rusqlite::{params, OptionalExtension};

pub(super) fn create(path: &Path) -> Result<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .sync_all()?;
    let db = Connection::open(path)?;
    db.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; CREATE TABLE labels (id TEXT PRIMARY KEY NOT NULL, title TEXT NOT NULL) WITHOUT ROWID;")?;
    drop(db);
    sync_directory(path.parent().unwrap())
}
pub(super) fn put(path: &Path, id: &str, title: &str) -> Result<()> {
    if id.is_empty() || id.len() > 4096 || title.len() > MAX_PAGE_BYTES as usize {
        return Err(GardenError::Recovery(
            "Knowledge page label is invalid or too large.".into(),
        ));
    }
    let db = Connection::open(path)?;
    db.execute_batch("PRAGMA synchronous=FULL;")?;
    let existing: Option<String> = db
        .query_row("SELECT title FROM labels WHERE id=?1", params![id], |row| {
            row.get(0)
        })
        .optional()?;
    if let Some(existing) = existing {
        if existing != title {
            return Err(GardenError::Recovery(
                "Knowledge page identity has conflicting labels.".into(),
            ));
        }
        return Ok(());
    }
    db.execute(
        "INSERT INTO labels(id,title) VALUES (?1,?2)",
        params![id, title],
    )?;
    Ok(())
}
pub(super) fn get(path: &Path, id: &str) -> Result<Option<String>> {
    let db = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let title: Option<String> = db
        .query_row(
            "SELECT CASE WHEN length(title)<=?2 THEN title ELSE NULL END FROM labels WHERE id=?1",
            params![id, MAX_PAGE_BYTES as i64],
            |row| row.get(0),
        )
        .optional()?;
    Ok(title)
}
pub(super) fn next(path: &Path, after: Option<&str>) -> Result<Option<(String, String)>> {
    let db = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let row: Option<(String,String)> = db.query_row("SELECT CASE WHEN length(id)<=4096 THEN id ELSE '' END, CASE WHEN length(title)<=?2 THEN title ELSE NULL END FROM labels WHERE id>?1 ORDER BY id LIMIT 1", params![after.unwrap_or(""), MAX_PAGE_BYTES as i64], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
    if row.as_ref().is_some_and(|(id, _)| id.is_empty()) {
        return Err(GardenError::Recovery(
            "Knowledge page label is invalid or too large.".into(),
        ));
    }
    Ok(row)
}
