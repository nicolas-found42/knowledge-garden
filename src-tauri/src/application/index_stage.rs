use super::*;

const PAGE_LOOKUP_VERSION: &[u8] = b"page-lookup-v1";
const PAGE_LOOKUP_BATCH: usize = 128;

fn add_field(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value.as_bytes());
}

fn lookup_digest(paths: &[PathBuf]) -> Result<Option<String>> {
    let mut digest = Sha256::new();
    digest.update(PAGE_LOOKUP_VERSION);
    for path in paths {
        let Some(path_text) = path.to_str() else {
            return Ok(None);
        };
        let header = read_knowledge_header(path)?;
        add_field(&mut digest, path_text);
        add_field(&mut digest, &header.page_id);
    }
    Ok(Some(hex::encode(digest.finalize())))
}

fn staged_prefix_matches(
    index: &Connection,
    generation: &str,
    paths: &[PathBuf],
    cursor: usize,
) -> Result<bool> {
    let mut statement = index.prepare(
        "SELECT path,page_id FROM rebuild_page_lookup_stage WHERE generation=?1 ORDER BY path",
    )?;
    let mut rows = statement.query([generation])?;
    for path in paths.iter().take(cursor) {
        let Some(row) = rows.next()? else {
            return Ok(false);
        };
        let (Ok(stored_path), Ok(stored_id)) = (row.get::<_, String>(0), row.get::<_, String>(1))
        else {
            return Ok(false);
        };
        let Some(path_text) = path.to_str() else {
            return Ok(false);
        };
        if stored_path != path_text || stored_id != read_knowledge_header(path)?.page_id {
            return Ok(false);
        }
    }
    Ok(rows.next()?.is_none())
}

impl Application {
    pub(super) fn prepare_page_lookup_stage(
        &mut self,
        paths: &[PathBuf],
    ) -> Result<Option<String>> {
        let Some(digest) = lookup_digest(paths)? else {
            return Ok(None);
        };
        self.index.execute_batch(
            "CREATE TABLE IF NOT EXISTS rebuild_page_lookup_stage (
                generation TEXT NOT NULL, path TEXT NOT NULL, page_id TEXT NOT NULL,
                PRIMARY KEY(generation,path)
            ) WITHOUT ROWID;
            CREATE TABLE IF NOT EXISTS rebuild_page_lookup_progress (
                singleton INTEGER PRIMARY KEY CHECK(singleton=1), digest TEXT NOT NULL,
                generation TEXT NOT NULL, cursor INTEGER NOT NULL, complete INTEGER NOT NULL
            );",
        )?;
        let stored: Option<(Option<String>, Option<String>, Option<i64>)> = self.index.query_row(
            "SELECT digest,generation,cursor FROM rebuild_page_lookup_progress WHERE singleton=1", [],
            |row| Ok((row.get(0).ok(), row.get(1).ok(), row.get(2).ok())),
        ).optional()?;
        let reusable = if let Some((Some(stored_digest), Some(generation), Some(cursor))) = stored {
            if stored_digest == digest {
                if let Ok(cursor) = usize::try_from(cursor) {
                    if cursor <= paths.len()
                        && staged_prefix_matches(&self.index, &generation, paths, cursor)?
                    {
                        Some((generation, cursor))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        let (generation, mut cursor) = if let Some(reusable) = reusable {
            reusable
        } else {
            let generation = Uuid::new_v4().to_string();
            self.index.execute(
                "INSERT INTO rebuild_page_lookup_progress(singleton,digest,generation,cursor,complete) VALUES(1,?1,?2,0,0) ON CONFLICT(singleton) DO UPDATE SET digest=excluded.digest,generation=excluded.generation,cursor=0,complete=0",
                params![digest, generation],
            )?;
            (generation, 0)
        };
        let mut finished = false;
        while !finished {
            #[cfg(test)]
            let previous_cursor = cursor;
            #[cfg(test)]
            let limit = if self.interrupt_page_lookup_after_batch {
                1
            } else {
                PAGE_LOOKUP_BATCH
            };
            #[cfg(not(test))]
            let limit = PAGE_LOOKUP_BATCH;
            let transaction = self.index.unchecked_transaction()?;
            for _ in 0..limit {
                let mut pruned = transaction.execute(
                    "DELETE FROM rebuild_page_lookup_stage WHERE (generation,path) = (SELECT generation,path FROM rebuild_page_lookup_stage WHERE generation < ?1 ORDER BY generation,path LIMIT 1)",
                    [&generation],
                )?;
                if pruned == 0 {
                    pruned = transaction.execute(
                        "DELETE FROM rebuild_page_lookup_stage WHERE (generation,path) = (SELECT generation,path FROM rebuild_page_lookup_stage WHERE generation > ?1 ORDER BY generation,path LIMIT 1)",
                        [&generation],
                    )?;
                }
                if pruned != 0 {
                    continue;
                }
                let Some(path) = paths.get(cursor) else {
                    finished = true;
                    break;
                };
                let path_text = path
                    .to_str()
                    .expect("non-UTF-8 paths use the existing lookup");
                let header = read_knowledge_header(path)?;
                transaction.execute(
                    "INSERT INTO rebuild_page_lookup_stage(generation,path,page_id) VALUES(?1,?2,?3)",
                    params![generation, path_text, header.page_id],
                )?;
                cursor += 1;
            }
            transaction.execute(
                "UPDATE rebuild_page_lookup_progress SET cursor=?1,complete=0 WHERE singleton=1 AND generation=?2",
                params![i64::try_from(cursor).map_err(|_| GardenError::Invalid("Page lookup is too large.".into()))?, generation],
            )?;
            transaction.commit()?;
            #[cfg(test)]
            if self.interrupt_page_lookup_after_batch && cursor > previous_cursor {
                self.interrupt_page_lookup_after_batch = false;
                return Err(GardenError::Invalid(
                    "Page lookup preparation interrupted for testing.".into(),
                ));
            }
        }
        let current_paths = self.knowledge_paths()?;
        if lookup_digest(&current_paths)?.as_deref() != Some(digest.as_str())
            || !staged_prefix_matches(&self.index, &generation, paths, paths.len())?
        {
            return Err(GardenError::Invalid(
                "Page identities changed during lookup preparation; retry reconciliation.".into(),
            ));
        }
        self.index.execute(
            "UPDATE rebuild_page_lookup_progress SET complete=1 WHERE singleton=1 AND generation=?1 AND digest=?2",
            params![generation, digest],
        )?;
        Ok(Some(generation))
    }
}
