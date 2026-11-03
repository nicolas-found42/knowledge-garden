//! Durable, version-fenced semantic results for bounded text chunks.
//!
//! One immutable record represents one contiguous range. A committed record is
//! renamed into place before the caller advances; recovery walks the records
//! from byte zero, so an interrupted write never advances the checkpoint.
use crate::semantic::KnowledgeDraft;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

const MAX_RECORD_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedChunk {
    pub source_version_id: String,
    pub generation: String,
    pub start: u64,
    pub end: u64,
    pub draft: KnowledgeDraft,
}

fn record_path(dir: &Path, start: u64) -> std::path::PathBuf {
    dir.join(format!("{start:020}.json"))
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn read_record(path: &Path) -> io::Result<StagedChunk> {
    let mut file = File::open(path)?;
    if file.metadata()?.len() > MAX_RECORD_BYTES {
        return Err(invalid("staged chunk exceeds its size limit"));
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_RECORD_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err(invalid("staged chunk exceeds its size limit"));
    }
    serde_json::from_slice(&bytes).map_err(|e| invalid(&format!("invalid staged chunk: {e}")))
}

/// Read one already committed range for bounded reconciliation.
pub fn read_committed(
    dir: &Path,
    start: u64,
    version: &str,
    generation: &str,
    total: u64,
) -> io::Result<StagedChunk> {
    let chunk = read_record(&record_path(dir, start))?;
    if chunk.source_version_id != version
        || chunk.generation != generation
        || chunk.start != start
        || chunk.end <= start
        || chunk.end > total
    {
        return Err(invalid(
            "staged chunk does not match its source version, generation or range",
        ));
    }
    Ok(chunk)
}

/// Find the first uncommitted offset without loading all drafts into memory.
pub fn checkpoint(dir: &Path, version: &str, generation: &str, total: u64) -> io::Result<u64> {
    let mut offset = 0;
    while offset < total {
        let chunk = match read_committed(dir, offset, version, generation, total) {
            Ok(chunk) => chunk,
            Err(e) if e.kind() == io::ErrorKind::NotFound => break,
            Err(e) => return Err(e),
        };
        offset = chunk.end;
    }
    Ok(offset)
}

/// Commit one range. A retry with identical bytes is a no-op; a conflicting
/// result cannot overwrite the prior record. The caller must hold the source
/// lock and recheck the active version and generation before calling this.
pub fn commit(dir: &Path, chunk: &StagedChunk, total: u64) -> io::Result<()> {
    if chunk.start >= chunk.end || chunk.end > total {
        return Err(invalid("invalid staged chunk range"));
    }
    fs::create_dir_all(dir)?;
    let bytes = serde_json::to_vec(chunk).map_err(|e| invalid(&e.to_string()))?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err(invalid("staged chunk exceeds its size limit"));
    }
    let path = record_path(dir, chunk.start);
    if path.exists() {
        return if fs::read(path)? == bytes {
            Ok(())
        } else {
            Err(invalid("conflicting staged chunk"))
        };
    }
    if checkpoint(dir, &chunk.source_version_id, &chunk.generation, total)? != chunk.start {
        return Err(invalid("staged chunks must be contiguous"));
    }
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(&bytes)?;
    tmp.as_file().sync_all()?;
    match tmp.persist_noclobber(&path) {
        Ok(_) => {
            File::open(dir)?.sync_all()?;
            Ok(())
        }
        Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => {
            if fs::read(path)? == bytes {
                Ok(())
            } else {
                Err(invalid("conflicting staged chunk"))
            }
        }
        Err(e) => Err(e.error),
    }
}

#[derive(Serialize, Deserialize)]
struct Cursor {
    source_version_id: String,
    generation: String,
    total: u64,
    offset: u64,
}

/// Open once per active source job. Opening audits the durable chain, including
/// any record written just before a crash, and repairs a missing or stale cursor.
/// Subsequent commits use the in-memory offset without rescanning older records.
pub struct ChunkStore {
    dir: std::path::PathBuf,
    version: String,
    generation: String,
    total: u64,
    offset: u64,
}

impl ChunkStore {
    pub fn open(dir: &Path, version: &str, generation: &str, total: u64) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let offset = checkpoint(dir, version, generation, total)?;
        let store = Self {
            dir: dir.to_owned(),
            version: version.into(),
            generation: generation.into(),
            total,
            offset,
        };
        store.write_cursor()?;
        Ok(store)
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    fn write_cursor(&self) -> io::Result<()> {
        let cursor = Cursor {
            source_version_id: self.version.clone(),
            generation: self.generation.clone(),
            total: self.total,
            offset: self.offset,
        };
        let mut tmp = tempfile::NamedTempFile::new_in(&self.dir)?;
        serde_json::to_writer(&mut tmp, &cursor).map_err(|e| invalid(&e.to_string()))?;
        tmp.as_file().sync_all()?;
        tmp.persist(self.dir.join("cursor.json"))
            .map_err(|e| e.error)?;
        File::open(&self.dir)?.sync_all()
    }

    pub fn commit(&mut self, chunk: &StagedChunk) -> io::Result<()> {
        if chunk.start < self.offset
            && chunk.source_version_id == self.version
            && chunk.generation == self.generation
        {
            let path = record_path(&self.dir, chunk.start);
            let bytes = serde_json::to_vec(chunk).map_err(|e| invalid(&e.to_string()))?;
            if fs::metadata(&path)?.len() <= MAX_RECORD_BYTES && fs::read(path)? == bytes {
                return Ok(());
            }
            return Err(invalid("conflicting staged chunk"));
        }
        if chunk.source_version_id != self.version
            || chunk.generation != self.generation
            || chunk.start != self.offset
            || chunk.end <= chunk.start
            || chunk.end > self.total
        {
            return Err(invalid("chunk does not match the active checkpoint"));
        }
        let bytes = serde_json::to_vec(chunk).map_err(|e| invalid(&e.to_string()))?;
        if bytes.len() as u64 > MAX_RECORD_BYTES {
            return Err(invalid("staged chunk exceeds its size limit"));
        }
        let path = record_path(&self.dir, chunk.start);
        let mut tmp = tempfile::NamedTempFile::new_in(&self.dir)?;
        tmp.write_all(&bytes)?;
        tmp.as_file().sync_all()?;
        match tmp.persist_noclobber(&path) {
            Ok(_) => File::open(&self.dir)?.sync_all()?,
            Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => {
                if fs::metadata(&path)?.len() > MAX_RECORD_BYTES || fs::read(path)? != bytes {
                    return Err(invalid("conflicting staged chunk"));
                }
            }
            Err(e) => return Err(e.error),
        }
        self.offset = chunk.end;
        self.write_cursor()
    }
}

/// Validate provider offsets against the exact chunk before rebasing them to the
/// retained source. An invalid item rejects the entire result.
pub fn validate_and_rebase(draft: &mut KnowledgeDraft, text: &str, start: u64) -> io::Result<()> {
    use crate::semantic::EvidenceDraft;
    let base = usize::try_from(start).map_err(|_| invalid("source offset is too large"))?;
    let mut validated = draft.clone();
    fn rebase(e: &mut EvidenceDraft, text: &str, base: usize) -> io::Result<()> {
        if e.quote.trim().is_empty()
            || e.byte_start >= e.byte_end
            || text.get(e.byte_start..e.byte_end) != Some(e.quote.as_str())
        {
            return Err(invalid("semantic evidence does not match the chunk"));
        }
        e.byte_start = e
            .byte_start
            .checked_add(base)
            .ok_or_else(|| invalid("evidence offset overflow"))?;
        e.byte_end = e
            .byte_end
            .checked_add(base)
            .ok_or_else(|| invalid("evidence offset overflow"))?;
        Ok(())
    }
    for item in &mut validated.entities {
        rebase(&mut item.evidence, text, base)?;
    }
    for item in &mut validated.facts {
        rebase(&mut item.evidence, text, base)?;
    }
    for item in &mut validated.relationships {
        rebase(&mut item.evidence, text, base)?;
    }
    for item in &mut validated.tags {
        rebase(&mut item.evidence, text, base)?;
    }
    for item in &mut validated.correction_candidates {
        rebase(&mut item.evidence, text, base)?;
    }
    for item in &mut validated.correction_alignments {
        rebase(&mut item.candidate.evidence, text, base)?;
    }
    if let Some(update) = &mut validated.source_update {
        rebase(&mut update.evidence, text, base)?;
        if let Some(e) = &mut update.source_date_evidence {
            rebase(e, text, base)?;
        }
        if let Some(e) = &mut update.source_revision_evidence {
            rebase(e, text, base)?;
        }
    }
    *draft = validated;
    Ok(())
}

/// Reconcile a fully contiguous chain under a fixed output budget. No partial
/// chain is publishable. Context-dependent corrections need a single complete
/// source judgment and are intentionally rejected across independent chunks.
pub fn aggregate(
    dir: &Path,
    version: &str,
    generation: &str,
    total: u64,
) -> io::Result<KnowledgeDraft> {
    let mut result = KnowledgeDraft::default();
    let mut offset = 0;
    let mut encoded_bytes = 0usize;
    while offset < total {
        let chunk = read_committed(dir, offset, version, generation, total)?;
        encoded_bytes = encoded_bytes
            .checked_add(
                serde_json::to_vec(&chunk.draft)
                    .map_err(|e| invalid(&e.to_string()))?
                    .len(),
            )
            .ok_or_else(|| invalid("semantic result size overflow"))?;
        if encoded_bytes > MAX_RECORD_BYTES as usize {
            return Err(invalid(
                "complete semantic result exceeds the 8 MiB reconciliation budget",
            ));
        }
        let mut draft = chunk.draft;
        if !draft.correction_candidates.is_empty() || !draft.correction_alignments.is_empty() {
            return Err(invalid("cross-chunk targeted corrections require complete context; prior knowledge is retained"));
        }
        result.entities.append(&mut draft.entities);
        result.facts.append(&mut draft.facts);
        result.relationships.append(&mut draft.relationships);
        result.tags.append(&mut draft.tags);
        result.decisions.append(&mut draft.decisions);
        if result.entities.len() > 500
            || result.facts.len() > 2000
            || result.relationships.len() > 2000
            || result.tags.len() > 2000
            || result.decisions.len() > 4096
        {
            return Err(invalid(
                "complete semantic result exceeds the safe record budget",
            ));
        }
        if let Some(update) = draft.source_update {
            if let Some(previous) = &result.source_update {
                // Offset-specific duplicate statements are not an independent
                // source-wide coverage judgment. Refuse conflicting metadata.
                if serde_json::to_value(previous).map_err(|e| invalid(&e.to_string()))?
                    != serde_json::to_value(&update).map_err(|e| invalid(&e.to_string()))?
                {
                    return Err(invalid(
                        "chunk source-update judgments disagree; prior knowledge is retained",
                    ));
                }
            } else {
                result.source_update = Some(update);
            }
        }
        offset = chunk.end;
    }
    Ok(result)
}
