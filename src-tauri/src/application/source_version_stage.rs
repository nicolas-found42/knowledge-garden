use super::*;
use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Unbounded};

const SOURCE_VERSION_STAGE_VERSION: &[u8] = b"source-version-and-summary-stage-v2";
const SOURCE_VERSION_BATCH: usize = 128;

pub(super) struct SourceVersionStage {
    pub generation: String,
    pub digest: String,
    paths: Vec<PathBuf>,
}

struct StoredProgress {
    digest: Option<String>,
    generation: Option<String>,
    source_cursor: Option<i64>,
    version_cursor: Option<String>,
}

struct CurrentSource {
    cursor: usize,
    source_id: Option<String>,
    summary: Option<String>,
    summary_staged: bool,
    versions: BTreeMap<String, String>,
}

fn version_map(info: &SourceInfo) -> BTreeMap<String, String> {
    let mut versions = BTreeMap::new();
    for version in &info.versions_seen {
        versions.insert(version.source_version_id.clone(), version.format.clone());
    }
    versions
}

pub(super) fn source_observation(path: &str, info: Option<&SourceInfo>) -> Result<String> {
    let source = match info {
        Some(info) => Some((&info.source_id, summary_json(info)?, version_map(info))),
        None => None,
    };
    Ok(serde_json::to_string(&(path, source))?)
}

fn add_observation(digest: &mut Sha256, observation: &str) {
    digest.update((observation.len() as u64).to_be_bytes());
    digest.update(observation.as_bytes());
}

pub(super) fn observed_source_digest(index: &Connection) -> Result<String> {
    let mut digest = Sha256::new();
    digest.update(SOURCE_VERSION_STAGE_VERSION);
    let mut statement =
        index.prepare("SELECT observation FROM rebuild_source_observations ORDER BY path")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    for row in rows {
        add_observation(&mut digest, &row?);
    }
    Ok(hex::encode(digest.finalize()))
}

impl Application {
    fn source_stage_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for entry in fs::read_dir(self.root.join("sources"))? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                paths.push(entry.path());
            }
        }
        paths.sort();
        Ok(paths)
    }

    fn readable_source_path(&self, path: &Path) -> Option<SourcePage> {
        let page = read_page(&path.join("index.md")).ok()?;
        if path != self.source_dir(&page.info.source_id).ok()? {
            return None;
        }
        self.open_source(&page.info.source_id).ok()
    }

    fn source_stage_digest(&self, paths: &[PathBuf]) -> Result<Option<String>> {
        let mut digest = Sha256::new();
        digest.update(SOURCE_VERSION_STAGE_VERSION);
        for path in paths {
            let Some(path_text) = path.to_str() else {
                return Ok(None);
            };
            let page = self.readable_source_path(path);
            add_observation(
                &mut digest,
                &source_observation(path_text, page.as_ref().map(|page| &page.info))?,
            );
        }
        Ok(Some(hex::encode(digest.finalize())))
    }

    fn source_prefix_matches(
        &self,
        generation: &str,
        paths: &[PathBuf],
        source_cursor: usize,
        version_cursor: Option<&str>,
    ) -> Result<bool> {
        if source_cursor > paths.len() || (source_cursor == paths.len() && version_cursor.is_some())
        {
            return Ok(false);
        }
        let mut expected_summaries = 0_i64;
        for (index, path) in paths.iter().enumerate() {
            if index > source_cursor {
                break;
            }
            let Some(page) = self.readable_source_path(path) else {
                if index == source_cursor && version_cursor.is_some() {
                    return Ok(false);
                }
                continue;
            };
            let stored: Option<Option<String>> = self.index.query_row(
                "SELECT summary FROM rebuild_source_summaries_stage WHERE generation=?1 AND source_id=?2",
                params![generation,page.info.source_id], |row| Ok(row.get(0).ok()),
            ).optional()?;
            let required = index < source_cursor || version_cursor.is_some();
            match stored {
                Some(Some(summary)) if summary == summary_json(&page.info)? => {
                    expected_summaries += 1
                }
                None if !required => {}
                _ => return Ok(false),
            }
        }
        let actual_summaries: i64 = self.index.query_row(
            "SELECT count(*) FROM rebuild_source_summaries_stage WHERE generation=?1",
            [generation],
            |row| row.get(0),
        )?;
        if actual_summaries != expected_summaries {
            return Ok(false);
        }
        let mut statement = self.index.prepare("SELECT source_id,version_id,format FROM rebuild_source_versions_stage WHERE generation=?1 ORDER BY source_id,version_id")?;
        let mut rows = statement.query([generation])?;
        for (index, path) in paths.iter().enumerate() {
            if index > source_cursor || (index == source_cursor && version_cursor.is_none()) {
                break;
            }
            let Some(page) = self.readable_source_path(path) else {
                if index == source_cursor && version_cursor.is_some() {
                    return Ok(false);
                }
                continue;
            };
            let versions = version_map(&page.info);
            if index == source_cursor
                && version_cursor.is_some_and(|cursor| !versions.contains_key(cursor))
            {
                return Ok(false);
            }
            for (version_id, format) in versions {
                if index == source_cursor
                    && version_cursor.is_some_and(|cursor| version_id.as_str() > cursor)
                {
                    break;
                }
                let Some(row) = rows.next()? else {
                    return Ok(false);
                };
                let (Ok(stored_source), Ok(stored_version), Ok(stored_format)) = (
                    row.get::<_, String>(0),
                    row.get::<_, String>(1),
                    row.get::<_, String>(2),
                ) else {
                    return Ok(false);
                };
                if stored_source != page.info.source_id
                    || stored_version != version_id
                    || stored_format != format
                {
                    return Ok(false);
                }
            }
        }
        Ok(rows.next()?.is_none())
    }

    pub(super) fn prepare_source_version_stage(&mut self) -> Result<Option<SourceVersionStage>> {
        let paths = self.source_stage_paths()?;
        let Some(digest) = self.source_stage_digest(&paths)? else {
            return Ok(None);
        };
        self.index.execute_batch("CREATE TABLE IF NOT EXISTS rebuild_source_versions_stage (
            generation TEXT NOT NULL, source_id TEXT NOT NULL, version_id TEXT NOT NULL, format TEXT NOT NULL,
            PRIMARY KEY(generation,source_id,version_id)) WITHOUT ROWID;
            CREATE TABLE IF NOT EXISTS rebuild_source_summaries_stage (
            generation TEXT NOT NULL, source_id TEXT NOT NULL, summary TEXT NOT NULL,
            PRIMARY KEY(generation,source_id)) WITHOUT ROWID;
            CREATE TABLE IF NOT EXISTS rebuild_source_versions_progress (
            singleton INTEGER PRIMARY KEY CHECK(singleton=1), digest TEXT NOT NULL, generation TEXT NOT NULL,
            source_cursor INTEGER NOT NULL, version_cursor TEXT, complete INTEGER NOT NULL);")?;
        let stored: Option<StoredProgress> = self.index.query_row(
            "SELECT digest,generation,source_cursor,version_cursor FROM rebuild_source_versions_progress WHERE singleton=1", [],
            |row| Ok(StoredProgress { digest: row.get(0).ok(), generation: row.get(1).ok(), source_cursor: row.get(2).ok(), version_cursor: row.get(3).ok() }),
        ).optional()?;
        let reusable = if let Some(StoredProgress {
            digest: Some(old_digest),
            generation: Some(generation),
            source_cursor: Some(source_cursor),
            version_cursor,
        }) = stored
        {
            if old_digest == digest {
                if let Ok(source_cursor) = usize::try_from(source_cursor) {
                    if self.source_prefix_matches(
                        &generation,
                        &paths,
                        source_cursor,
                        version_cursor.as_deref(),
                    )? {
                        Some((generation, source_cursor, version_cursor))
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
        let (generation, mut source_cursor, mut version_cursor) = if let Some(reusable) = reusable {
            reusable
        } else {
            let generation = Uuid::new_v4().to_string();
            self.index.execute("INSERT INTO rebuild_source_versions_progress(singleton,digest,generation,source_cursor,version_cursor,complete) VALUES(1,?1,?2,0,NULL,0) ON CONFLICT(singleton) DO UPDATE SET digest=excluded.digest,generation=excluded.generation,source_cursor=0,version_cursor=NULL,complete=0", params![digest,generation])?;
            (generation, 0, None)
        };
        let mut finished = false;
        let mut current_source: Option<CurrentSource> = None;
        while !finished {
            #[cfg(test)]
            let limit = if self.interrupt_source_versions_after_batch {
                1
            } else {
                SOURCE_VERSION_BATCH
            };
            #[cfg(not(test))]
            let limit = SOURCE_VERSION_BATCH;
            #[cfg(test)]
            let mut inserted = false;
            #[cfg(test)]
            let mut summary_inserted = false;
            let transaction = self.index.unchecked_transaction()?;
            for _ in 0..limit {
                let mut pruned = transaction.execute("DELETE FROM rebuild_source_summaries_stage WHERE (generation,source_id) = (SELECT generation,source_id FROM rebuild_source_summaries_stage WHERE generation < ?1 ORDER BY generation,source_id LIMIT 1)", [&generation])?;
                if pruned == 0 {
                    pruned = transaction.execute("DELETE FROM rebuild_source_summaries_stage WHERE (generation,source_id) = (SELECT generation,source_id FROM rebuild_source_summaries_stage WHERE generation > ?1 ORDER BY generation,source_id LIMIT 1)", [&generation])?;
                }
                if pruned == 0 {
                    pruned = transaction.execute("DELETE FROM rebuild_source_versions_stage WHERE (generation,source_id,version_id) = (SELECT generation,source_id,version_id FROM rebuild_source_versions_stage WHERE generation < ?1 ORDER BY generation,source_id,version_id LIMIT 1)", [&generation])?;
                }
                if pruned == 0 {
                    pruned = transaction.execute("DELETE FROM rebuild_source_versions_stage WHERE (generation,source_id,version_id) = (SELECT generation,source_id,version_id FROM rebuild_source_versions_stage WHERE generation > ?1 ORDER BY generation,source_id,version_id LIMIT 1)", [&generation])?;
                }
                if pruned != 0 {
                    continue;
                }
                let Some(path) = paths.get(source_cursor) else {
                    finished = true;
                    break;
                };
                if current_source
                    .as_ref()
                    .is_none_or(|source| source.cursor != source_cursor)
                {
                    let page = self.readable_source_path(path);
                    let source_id = page.as_ref().map(|page| page.info.source_id.clone());
                    let summary = page
                        .as_ref()
                        .map(|page| summary_json(&page.info))
                        .transpose()?;
                    let versions = page
                        .as_ref()
                        .map(|page| version_map(&page.info))
                        .unwrap_or_default();
                    current_source = Some(CurrentSource {
                        cursor: source_cursor,
                        source_id,
                        summary,
                        summary_staged: false,
                        versions,
                    });
                }
                let current = current_source.as_mut().unwrap();
                if !current.summary_staged {
                    if let (Some(source_id), Some(summary)) =
                        (current.source_id.clone(), current.summary.clone())
                    {
                        transaction.execute("INSERT OR REPLACE INTO rebuild_source_summaries_stage(generation,source_id,summary) VALUES(?1,?2,?3)", params![generation,source_id,summary])?;
                        #[cfg(test)]
                        {
                            summary_inserted = true;
                        }
                    }
                    current.summary_staged = true;
                }
                let versions = &current.versions;
                let next = match &version_cursor {
                    Some(cursor) => versions
                        .range::<str, _>((Excluded(cursor.as_str()), Unbounded))
                        .next(),
                    None => versions.iter().next(),
                };
                if let Some((version_id, format)) = next {
                    let source_id = current.source_id.as_ref().unwrap();
                    transaction.execute("INSERT INTO rebuild_source_versions_stage(generation,source_id,version_id,format) VALUES(?1,?2,?3,?4)", params![generation,source_id,version_id,format])?;
                    version_cursor = Some(version_id.clone());
                    #[cfg(test)]
                    {
                        inserted = true;
                    }
                } else {
                    source_cursor += 1;
                    version_cursor = None;
                }
            }
            transaction.execute("UPDATE rebuild_source_versions_progress SET source_cursor=?1,version_cursor=?2,complete=0 WHERE singleton=1 AND generation=?3", params![i64::try_from(source_cursor).map_err(|_| GardenError::Invalid("Too many source directories.".into()))?,version_cursor,generation])?;
            transaction.commit()?;
            #[cfg(test)]
            if self.interrupt_source_summaries_after_batch && summary_inserted {
                self.interrupt_source_summaries_after_batch = false;
                return Err(GardenError::Invalid(
                    "Source-summary staging interrupted for testing.".into(),
                ));
            }
            #[cfg(test)]
            if self.interrupt_source_versions_after_batch && inserted {
                self.interrupt_source_versions_after_batch = false;
                return Err(GardenError::Invalid(
                    "Source-version staging interrupted for testing.".into(),
                ));
            }
        }
        let current_paths = self.source_stage_paths()?;
        if self.source_stage_digest(&current_paths)?.as_deref() != Some(digest.as_str())
            || !self.source_prefix_matches(&generation, &paths, paths.len(), None)?
        {
            return Err(GardenError::Invalid(
                "Source versions changed during staging; retry reconciliation.".into(),
            ));
        }
        self.index.execute("UPDATE rebuild_source_versions_progress SET complete=1 WHERE singleton=1 AND generation=?1 AND digest=?2", params![generation,digest])?;
        Ok(Some(SourceVersionStage {
            generation,
            digest,
            paths,
        }))
    }

    pub(super) fn source_stage_inputs_match(&self, stage: &SourceVersionStage) -> Result<bool> {
        let paths = self.source_stage_paths()?;
        if paths != stage.paths {
            return Ok(false);
        }
        Ok(self.source_stage_digest(&paths)?.as_deref() == Some(stage.digest.as_str()))
    }
}
