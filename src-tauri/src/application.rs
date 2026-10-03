//! Public, disk-backed collection operations. Markdown and originals are authoritative.
use crate::semantic::{
    EntityDraft, EvidenceDraft, KnowledgeDraft, KnowledgePage, KnowledgePageSummary, ProviderError,
    SemanticDecision, SemanticJob, SemanticProvider,
};
use fs2::FileExt;
use rusqlite::{params, params_from_iter, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use uuid::Uuid;

pub const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_PAGE_BYTES: u64 = (MAX_TEXT_BYTES * 3 + 64 * 1024) as u64;
const PAGE_SIZE: usize = 50;

#[derive(Debug, Error)]
pub enum GardenError {
    #[error("Collection I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Collection index failed: {0}")]
    Index(#[from] rusqlite::Error),
    #[error("Source page metadata is unreadable: {0}")]
    Metadata(#[from] serde_yaml_ng::Error),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, GardenError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionMethod {
    Picker,
    Drop,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionState {
    TextPreserved,
    Unsupported,
    InvalidUtf8,
    TooLarge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Acquisition {
    pub path: String,
    pub method: AcquisitionMethod,
    /// Milliseconds since the Unix epoch, stored as a string to avoid JS precision loss.
    pub received_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    pub schema: u32,
    pub source_id: String,
    pub page_id: String,
    pub title: String,
    pub original_name: String,
    pub asset: String,
    pub sha256: String,
    pub bytes: u64,
    pub format: String,
    pub extraction: ExtractionState,
    pub extraction_detail: String,
    pub line_count: usize,
    pub acquisitions: Vec<Acquisition>,
    #[serde(default = "semantic_pending")]
    pub semantic_state: String,
    #[serde(default)]
    pub semantic_error: Option<String>,
    #[serde(default)]
    pub semantic_attempts: u32,
    #[serde(default)]
    pub semantic_retry_at: Option<String>,
    #[serde(default)]
    pub knowledge_pages: Vec<KnowledgePageSummary>,
    #[serde(default)]
    pub semantic_decisions: Vec<SemanticDecision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSummary {
    pub source_id: String,
    pub page_id: String,
    pub title: String,
    pub extraction: ExtractionState,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourcePage {
    pub info: SourceInfo,
    pub markdown: String,
    pub body: String,
    #[serde(default)]
    pub knowledge_pages: Vec<KnowledgePageSummary>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceList {
    pub sources: Vec<SourceSummary>,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PageSearchRequest {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub date_from: Option<String>,
    #[serde(default)]
    pub date_to: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub processing_status: Option<String>,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSearchResults {
    pub pages: Vec<PageResult>,
    pub next_offset: Option<usize>,
    pub available_tags: Vec<String>,
    pub available_formats: Vec<String>,
    pub available_statuses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageResult {
    pub page_id: String,
    pub source_id: String,
    pub page_type: String,
    pub title: String,
    pub kind: String,
    pub excerpt: String,
    pub tags: Vec<String>,
    pub format: String,
    pub event_date: Option<String>,
    pub extraction: ExtractionState,
    pub processing_status: String,
    pub matched_by: String,
    pub match_location: Option<SearchMatchLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatchLocation {
    pub record_id: String,
    pub source_id: String,
    pub quote: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub line_start: usize,
    pub line_end: usize,
}

pub struct Application {
    root: PathBuf,
    index: Connection,
    // The lock survives for this application's lifetime, including across commands.
    _lock: File,
    semantic_provider: Arc<dyn SemanticProvider>,
}

fn semantic_pending() -> String {
    "pending".into()
}

impl Application {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        Self::open_inner(root, Arc::new(UnavailableProvider))
    }

    pub fn open_with_semantic_provider(
        root: impl AsRef<Path>,
        semantic_provider: Arc<dyn SemanticProvider>,
    ) -> Result<Self> {
        Self::open_inner(root, semantic_provider)
    }

    fn open_inner(
        root: impl AsRef<Path>,
        semantic_provider: Arc<dyn SemanticProvider>,
    ) -> Result<Self> {
        fs::create_dir_all(root.as_ref())?;
        let root = fs::canonicalize(root)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join(".collection.lock"))?;
        lock.try_lock_exclusive().map_err(|_| {
            GardenError::Invalid("This collection is already open in another process.".into())
        })?;
        fs::create_dir_all(root.join("sources"))?;
        fs::create_dir_all(root.join(".staging"))?;
        fs::create_dir_all(root.join(".derived"))?;
        let index = Connection::open(root.join(".derived/lookup.sqlite"))?;
        index.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS sources (
                 source_id TEXT PRIMARY KEY, summary TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS page_tags (
                 page_id TEXT NOT NULL,
                 normalized TEXT NOT NULL,
                 label TEXT NOT NULL,
                 source_id TEXT NOT NULL,
                 source_version_id TEXT NOT NULL,
                 support_json TEXT NOT NULL,
                 PRIMARY KEY (page_id, normalized, source_id, source_version_id, support_json)
             );
             CREATE INDEX IF NOT EXISTS page_tags_normalized ON page_tags(normalized, page_id);
             CREATE VIRTUAL TABLE IF NOT EXISTS page_search USING fts5(
                 page_id UNINDEXED,
                 source_id UNINDEXED,
                 page_type UNINDEXED,
                 title,
                 kind UNINDEXED,
                 content,
                 tags,
                 format UNINDEXED,
                 extraction UNINDEXED,
                 processing_status UNINDEXED,
                 event_date UNINDEXED,
                 doc_kind UNINDEXED,
                 match_location UNINDEXED,
                 tokenize = 'unicode61 remove_diacritics 2'
             );",
        )?;
        let mut app = Self {
            root,
            index,
            _lock: lock,
            semantic_provider,
        };
        app.rebuild_index()?;
        Ok(app)
    }

    pub fn import_source(
        &mut self,
        path: impl AsRef<Path>,
        method: AcquisitionMethod,
    ) -> Result<SourcePage> {
        let supplied_path = if path.as_ref().is_absolute() {
            path.as_ref().to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        let path = fs::canonicalize(&supplied_path)?;
        if !fs::metadata(&path)?.is_file() {
            return Err(GardenError::Invalid(
                "Choose a regular file, rather than a directory or device.".into(),
            ));
        }
        let mut input = File::open(&path)?;
        let original_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let format = extension.to_lowercase();
        let asset = if extension.len() <= 16
            && !extension.is_empty()
            && extension.chars().all(|c| c.is_ascii_alphanumeric())
        {
            format!("original.{extension}")
        } else {
            "original".into()
        };
        let staging = tempfile::tempdir_in(self.root.join(".staging"))?;
        let mut output = File::create(staging.path().join(&asset))?;
        let mut hasher = Sha256::new();
        let mut text_bytes = Vec::new();
        let mut count = 0_u64;
        let mut chunk = [0_u8; 64 * 1024];
        loop {
            let read = input.read(&mut chunk)?;
            if read == 0 {
                break;
            }
            output.write_all(&chunk[..read])?;
            hasher.update(&chunk[..read]);
            count += read as u64;
            if count <= MAX_TEXT_BYTES as u64 {
                text_bytes.extend_from_slice(&chunk[..read]);
            }
        }
        output.sync_all()?;
        let digest = format!("{:x}", hasher.finalize());
        // A format label changes extraction coverage and the retained asset name.
        // Keep exact-byte imports together only when that label also agrees.
        let mut identity = Sha256::new();
        identity.update(format.as_bytes());
        identity.update([0]);
        identity.update(digest.as_bytes());
        let source_id = format!("source-{:x}", identity.finalize());
        let acquisition = Acquisition {
            path: supplied_path.to_string_lossy().into_owned(),
            method,
            received_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| GardenError::Invalid(e.to_string()))?
                .as_millis()
                .to_string(),
        };
        let destination = self.source_dir(&source_id)?;
        if destination.exists() {
            let existing = self.open_source(&source_id)?;
            if !existing
                .info
                .acquisitions
                .iter()
                .any(|old| old.path == acquisition.path && old.method == acquisition.method)
            {
                // Additional provenance has its own Markdown record; never rewrite an owner's page.
                let contexts = destination.join("acquisitions");
                fs::create_dir_all(&contexts)?;
                let key = format!(
                    "{:x}",
                    Sha256::digest(
                        format!("{}:{:?}", acquisition.path, acquisition.method).as_bytes()
                    )
                );
                let mut record = tempfile::NamedTempFile::new_in(&contexts)?;
                write!(
                    record,
                    "---\n{}---\n\n# Source acquisition\n\n[Source page](../index.md)\n",
                    serde_yaml_ng::to_string(&acquisition)?
                )?;
                record.as_file().sync_all()?;
                record
                    .persist_noclobber(contexts.join(format!("{key}.md")))
                    .map_err(|e| GardenError::Io(e.error))?;
                sync_directory(&contexts)?;
            }
            self.index_page(&existing.info)?;
            return self.open_source(&source_id);
        }
        let (extraction, extraction_detail, text) = if !matches!(
            format.as_str(),
            "" | "txt" | "text" | "md" | "markdown"
        ) {
            (
                ExtractionState::Unsupported,
                "This format is not supported by the text importer. The original is retained.",
                String::new(),
            )
        } else if count > MAX_TEXT_BYTES as u64 {
            (
                ExtractionState::TooLarge,
                "Text exceeds the 2 MiB preview limit. The complete original is retained.",
                String::new(),
            )
        } else {
            match String::from_utf8(text_bytes) {
                Err(_) => (ExtractionState::InvalidUtf8, "The source is not valid UTF-8. The original is retained without lossy decoding.", String::new()),
                Ok(text) if text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) =>
                    (ExtractionState::Unsupported, "The source contains binary control characters. The original is retained.", String::new()),
                Ok(text) => (ExtractionState::TextPreserved, "Source text preserved. Semantic fact extraction has not run.", text.trim_start_matches('\u{feff}').to_owned()),
            }
        };
        let line_count = if text.is_empty() {
            0
        } else {
            text.lines().count()
        };
        let info = SourceInfo {
            schema: 1,
            source_id,
            page_id: format!("page-{}", Uuid::new_v4()),
            title: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            original_name,
            asset,
            sha256: digest,
            bytes: count,
            format,
            extraction,
            extraction_detail: extraction_detail.into(),
            line_count,
            acquisitions: vec![acquisition],
            semantic_state: if extraction == ExtractionState::TextPreserved {
                "pending".into()
            } else {
                "unavailable".into()
            },
            semantic_error: None,
            semantic_attempts: 0,
            semantic_retry_at: None,
            knowledge_pages: Vec::new(),
            semantic_decisions: Vec::new(),
        };
        let body = source_body(&info, &text);
        let markdown = serialize_page(&info, &body)?;
        let mut page_file = File::create(staging.path().join("index.md"))?;
        page_file.write_all(markdown.as_bytes())?;
        page_file.sync_all()?;
        sync_directory(staging.path())?;
        fs::rename(staging.path(), &destination)?;
        sync_directory(&self.root.join("sources"))?;
        let page = SourcePage {
            info,
            markdown,
            body,
            knowledge_pages: Vec::new(),
        };
        self.index_page(&page.info)?;
        Ok(page)
    }

    pub fn open_source(&self, source_id: &str) -> Result<SourcePage> {
        let mut page = read_page(&self.page_path(source_id)?)?;
        let contexts = self.source_dir(source_id)?.join("acquisitions");
        if contexts.exists() {
            for entry in fs::read_dir(contexts)? {
                let entry = entry?;
                if entry.path().extension().and_then(|v| v.to_str()) != Some("md") {
                    continue;
                }
                let mut record = String::new();
                File::open(entry.path())?
                    .take(64 * 1024)
                    .read_to_string(&mut record)?;
                let header = record
                    .strip_prefix("---\n")
                    .and_then(|v| v.split_once("\n---\n"))
                    .ok_or_else(|| {
                        GardenError::Invalid("An acquisition record is unreadable.".into())
                    })?
                    .0;
                page.info
                    .acquisitions
                    .push(serde_yaml_ng::from_str(header)?);
            }
            page.info
                .acquisitions
                .sort_by(|a, b| a.received_at.cmp(&b.received_at).then(a.path.cmp(&b.path)));
        }
        page.knowledge_pages = page.info.knowledge_pages.clone();
        Ok(page)
    }

    pub fn open_knowledge_page(&self, page_id: &str) -> Result<KnowledgePage> {
        if !page_id.starts_with("page-")
            || !page_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(GardenError::Invalid(
                "Invalid knowledge page identity.".into(),
            ));
        }
        let path = self.root.join("pages").join(format!("{page_id}.md"));
        read_knowledge_page(&path)
    }

    /// Claim persisted work under the collection lock. Provider I/O happens after this returns,
    /// so the desktop can keep import and navigation responsive during a slow service call.
    pub fn claim_due_semantic_jobs(&mut self, max_jobs: usize) -> Result<Vec<SemanticJob>> {
        let now = now_millis()?;
        let mut due = Vec::new();
        for entry in fs::read_dir(self.root.join("sources"))? {
            if due.len() >= max_jobs {
                break;
            }
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let page = read_page(&entry.path().join("index.md"))?;
            if page.info.semantic_state == "processing" {
                let mut interrupted = page;
                interrupted.info.semantic_state = "pending".into();
                interrupted.info.semantic_error = Some(
                    "A previous semantic run was interrupted; automatic retry is scheduled.".into(),
                );
                let delay_seconds = (1_u64 << interrupted.info.semantic_attempts.min(12)).min(3600);
                interrupted.info.semantic_retry_at =
                    Some((now + u128::from(delay_seconds * 1000)).to_string());
                self.write_source_page(&interrupted)?;
                self.index_page(&interrupted.info)?;
                continue;
            }
            let retry_at = page
                .info
                .semantic_retry_at
                .as_deref()
                .and_then(|value| value.parse::<u128>().ok())
                .unwrap_or(0);
            if page.info.semantic_state == "pending" && retry_at <= now {
                due.push(page.info.source_id);
            }
        }
        let mut jobs = Vec::new();
        for source_id in due {
            let mut page = read_page(&self.page_path(&source_id)?)?;
            if page.info.extraction != ExtractionState::TextPreserved
                || page.info.semantic_state != "pending"
            {
                continue;
            }
            let bytes = fs::read(self.original_path(&source_id)?)?;
            let text = String::from_utf8(bytes).map_err(|_| {
                GardenError::Invalid("The retained text source is no longer UTF-8.".into())
            })?;
            page.info.semantic_state = "processing".into();
            page.info.semantic_attempts = page.info.semantic_attempts.saturating_add(1);
            page.info.semantic_error = None;
            page.info.semantic_retry_at = None;
            let attempt = page.info.semantic_attempts;
            self.write_source_page(&page)?;
            self.index_page(&page.info)?;
            jobs.push(SemanticJob {
                source_id,
                attempt,
                source_text: text,
            });
        }
        Ok(jobs)
    }

    pub fn finish_semantic_job(
        &mut self,
        job: SemanticJob,
        result: std::result::Result<KnowledgeDraft, ProviderError>,
    ) -> Result<()> {
        let mut page = read_page(&self.page_path(&job.source_id)?)?;
        if page.info.semantic_state != "processing" || page.info.semantic_attempts != job.attempt {
            return Err(GardenError::Invalid(
                "The semantic job is no longer the active source attempt.".into(),
            ));
        }
        match result {
            Ok(draft) => match self.publish_knowledge(&mut page, &job.source_text, draft) {
                Ok(()) => {
                    page.info.semantic_state = "complete".into();
                    page.info.semantic_error = None;
                    page.info.semantic_retry_at = None;
                }
                Err(error) => self.record_semantic_failure(&mut page, error.to_string(), true)?,
            },
            Err(error) => {
                self.record_semantic_failure(&mut page, error.message, error.retryable)?
            }
        }
        self.write_source_page(&page)?;
        self.index_page(&page.info)?;
        Ok(())
    }

    pub fn resume_due_semantic_jobs(&mut self) -> Result<()> {
        let jobs = self.claim_due_semantic_jobs(4)?;
        for job in jobs {
            let result = self.semantic_provider.form_knowledge(&job.source_text);
            self.finish_semantic_job(job, result)?;
        }
        Ok(())
    }

    fn publish_knowledge(
        &self,
        source: &mut SourcePage,
        source_text: &str,
        draft: KnowledgeDraft,
    ) -> Result<()> {
        if draft.entities.len() > 500
            || draft.facts.len() > 2_000
            || draft.relationships.len() > 2_000
        {
            return Err(GardenError::Invalid(
                "Semantic result exceeded its safe record limit.".into(),
            ));
        }
        let pages_dir = self.root.join("pages");
        fs::create_dir_all(&pages_dir)?;
        let mut by_label = std::collections::HashMap::new();
        let mut page_records = Vec::new();
        for entity in &draft.entities {
            let evidence = validate_evidence(source_text, &entity.evidence)?;
            if entity.kind.trim().is_empty() || entity.label.trim().is_empty() {
                return Err(GardenError::Invalid(
                    "Semantic entity has no type or label.".into(),
                ));
            }
            let page_id = stable_page_id(&source.info.source_id, entity);
            let summary = KnowledgePageSummary {
                page_id: page_id.clone(),
                title: entity.label.clone(),
                kind: entity.kind.clone(),
                path: format!("pages/{page_id}.md"),
            };
            by_label
                .entry(entity.label.as_str())
                .or_insert_with(Vec::new)
                .push(summary.clone());
            page_records.push((entity, summary, evidence));
        }
        let page_for = |label: &str| -> Result<&KnowledgePageSummary> {
            match by_label.get(label).map(Vec::as_slice) {
                Some([page]) => Ok(page),
                Some(_) => Err(GardenError::Invalid(format!(
                    "Semantic identity is ambiguous for `{label}`."
                ))),
                None => Err(GardenError::Invalid(format!(
                    "Semantic item refers to missing entity `{label}`."
                ))),
            }
        };
        let mut contents = std::collections::HashMap::<String, String>::new();
        for (entity, summary, evidence) in &page_records {
            let facts = draft
                .facts
                .iter()
                .filter(|fact| fact.subject == entity.label)
                .collect::<Vec<_>>();
            let relationships = draft
                .relationships
                .iter()
                .filter(|rel| rel.from == entity.label || rel.to == entity.label)
                .collect::<Vec<_>>();
            let fact_records = facts
                .iter()
                .map(|fact| {
                    let evidence = validate_evidence(source_text, &fact.evidence)?;
                    Ok(FactRecord {
                        fact_id: stable_record_id(
                            "fact",
                            &source.info.source_id,
                            &fact.property,
                            &fact.value,
                            evidence.byte_start,
                        ),
                        subject_page_id: summary.page_id.clone(),
                        property: fact.property.clone(),
                        value: fact.value.clone(),
                        qualifier: evidence.qualifier.clone(),
                        origin: evidence.origin.clone(),
                        evidence,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let relationship_records = relationships
                .iter()
                .map(|relationship| {
                    let from = page_for(&relationship.from)?;
                    let to = page_for(&relationship.to)?;
                    let evidence = validate_evidence(source_text, &relationship.evidence)?;
                    Ok(RelationshipRecord {
                        relationship_id: stable_record_id(
                            "relationship",
                            &source.info.source_id,
                            &relationship.kind,
                            &format!("{}→{}", from.page_id, to.page_id),
                            evidence.byte_start,
                        ),
                        from_page_id: from.page_id.clone(),
                        to_page_id: to.page_id.clone(),
                        kind: relationship.kind.clone(),
                        qualifier: relationship.qualifier.clone(),
                        origin: evidence.origin.clone(),
                        evidence,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let mut tag_records = Vec::<TagRecord>::new();
            {
                let mut add_tag = |label: &str, evidence: EvidenceLocation| -> Result<()> {
                    let normalized = normalize_tag(label)?;
                    let support = TagSupport {
                        source_id: source.info.source_id.clone(),
                        source_version_id: source.info.sha256.clone(),
                        value: label.trim().trim_start_matches('#').to_owned(),
                        qualifier: evidence.qualifier.clone(),
                        origin: evidence.origin.clone(),
                        evidence,
                    };
                    if let Some(existing) = tag_records
                        .iter_mut()
                        .find(|tag| tag.normalized == normalized)
                    {
                        if !existing.supports.iter().any(|old| {
                            old.source_id == support.source_id
                                && old.source_version_id == support.source_version_id
                                && old.evidence.byte_start == support.evidence.byte_start
                        }) {
                            existing.supports.push(support);
                        }
                    } else {
                        tag_records.push(TagRecord {
                            normalized,
                            label: label.trim().trim_start_matches('#').to_owned(),
                            supports: vec![support],
                        });
                    }
                    Ok(())
                };
                for tag in draft.tags.iter().filter(|tag| tag.subject == entity.label) {
                    add_tag(&tag.label, validate_evidence(source_text, &tag.evidence)?)?;
                }
                for fact in fact_records
                    .iter()
                    .filter(|fact| fact.property.eq_ignore_ascii_case("location"))
                {
                    add_tag(&fact.value, fact.evidence.clone())?;
                }
            }
            tag_records.sort_by(|left, right| left.normalized.cmp(&right.normalized));
            let mut body = format!("# {}\n\n## Facts\n", escape_heading(&entity.label));
            if facts.is_empty() {
                body.push_str("\nNo facts were selected for this page.\n");
            }
            for fact in &fact_records {
                body.push_str(&format!(
                    "\n- **{}:** {}{}\n  - Fact identity: `{}`\n  - Evidence: “{}”\n  - Origin: {} · source lines {}–{}, bytes {}–{}\n  - Original opens at the beginning; use these source lines and byte offsets to locate this passage.\n  - Links: [Source page](../sources/{}/index.md) · [Retained original](../sources/{}/{})\n",
                    escape_markdown(&fact.property.replace('_', " ")), escape_markdown(&fact.value), fact.qualifier.as_deref().map(|q| format!(" ({})", escape_markdown(q))).unwrap_or_default(), fact.fact_id,
                    escape_markdown(&fact.evidence.quote), escape_markdown(&fact.origin), fact.evidence.line_start, fact.evidence.line_end,
                    fact.evidence.byte_start, fact.evidence.byte_end, &source.info.source_id["source-".len()..],
                    &source.info.source_id["source-".len()..], source.info.asset
                ));
            }
            body.push_str("\n## Relationships\n");
            if relationships.is_empty() {
                body.push_str("\nNo relationships were selected for this page.\n");
            }
            for relationship in &relationship_records {
                let from = page_records
                    .iter()
                    .find(|(_, page, _)| page.page_id == relationship.from_page_id)
                    .map(|(entity, page, _)| (entity, page))
                    .unwrap();
                let to = page_records
                    .iter()
                    .find(|(_, page, _)| page.page_id == relationship.to_page_id)
                    .map(|(entity, page, _)| (entity, page))
                    .unwrap();
                body.push_str(&format!(
                    "\n- [{}](../pages/{}.md) — **{} →** — [{}](../pages/{}.md){}\n  - Relationship identity: `{}`\n  - Evidence: “{}”\n  - Origin: {} · source lines {}–{}, bytes {}–{}\n  - Original opens at the beginning; use these source lines and byte offsets to locate this passage.\n  - Links: [Source page](../sources/{}/index.md) · [Retained original](../sources/{}/{})\n",
                    escape_markdown(&from.0.label), from.1.page_id, escape_markdown(&relationship.kind.replace('_', " ")), escape_markdown(&to.0.label), to.1.page_id,
                    relationship.qualifier.as_deref().map(|q| format!(" (qualifier: {})", escape_markdown(q))).unwrap_or_default(), relationship.relationship_id,
                    escape_markdown(&relationship.evidence.quote), escape_markdown(&relationship.origin), relationship.evidence.line_start, relationship.evidence.line_end,
                    relationship.evidence.byte_start, relationship.evidence.byte_end, &source.info.source_id["source-".len()..],
                    &source.info.source_id["source-".len()..], source.info.asset
                ));
            }
            body.push_str("\n## Tags\n");
            if tag_records.is_empty() {
                body.push_str("\nNo supported tags were selected for this page.\n");
            }
            for tag in &tag_records {
                body.push_str(&format!(
                    "\n- **#{}** · {}\n  - Evidence: “{}” · source lines {}–{}, bytes {}–{}\n",
                    escape_markdown(&tag.label),
                    escape_markdown(&tag.normalized),
                    escape_markdown(&tag.supports[0].evidence.quote),
                    tag.supports[0].evidence.line_start,
                    tag.supports[0].evidence.line_end,
                    tag.supports[0].evidence.byte_start,
                    tag.supports[0].evidence.byte_end,
                ));
            }
            let header = serde_yaml_ng::to_string(&KnowledgePageHeader {
                schema: 1,
                page_id: summary.page_id.clone(),
                source_id: source.info.source_id.clone(),
                title: summary.title.clone(),
                kind: summary.kind.clone(),
                evidence: evidence.clone(),
                facts: fact_records,
                relationships: relationship_records,
                tags: tag_records,
            })?;
            contents.insert(
                summary.page_id.clone(),
                format!("---\n{header}---\n\n{body}"),
            );
        }
        // Validate all facts and relationship evidence even when attached to a page without an entity record.
        for fact in &draft.facts {
            let _ = page_for(&fact.subject)?;
            validate_evidence(source_text, &fact.evidence)?;
        }
        for relationship in &draft.relationships {
            let _ = page_for(&relationship.from)?;
            let _ = page_for(&relationship.to)?;
            validate_evidence(source_text, &relationship.evidence)?;
        }
        for tag in &draft.tags {
            let _ = page_for(&tag.subject)?;
            validate_tag_label(&tag.label)?;
            validate_evidence(source_text, &tag.evidence)?;
        }
        for (page_id, markdown) in contents {
            write_atomic(
                &pages_dir.join(format!("{page_id}.md")),
                markdown.as_bytes(),
            )?;
        }
        sync_directory(&pages_dir)?;
        source.info.knowledge_pages = page_records
            .iter()
            .map(|(_, summary, _)| summary.clone())
            .collect();
        source.knowledge_pages = source.info.knowledge_pages.clone();
        source.info.semantic_decisions = draft.decisions;
        source.info.semantic_state = "complete".into();
        source.body = replace_semantic_section(&source.body, &source.info.knowledge_pages);
        source.markdown = serialize_page(&source.info, &source.body)?;
        Ok(())
    }

    fn record_semantic_failure(
        &self,
        page: &mut SourcePage,
        message: String,
        retryable: bool,
    ) -> Result<()> {
        page.info.semantic_state = if retryable { "pending" } else { "failed" }.into();
        page.info.semantic_error = Some(message.chars().take(300).collect());
        let delay_seconds = (1_u64 << page.info.semantic_attempts.min(12)).min(3600);
        page.info.semantic_retry_at = retryable
            .then(|| (now_millis().unwrap_or(0) + u128::from(delay_seconds * 1000)).to_string());
        page.markdown = serialize_page(&page.info, &page.body)?;
        Ok(())
    }

    fn write_source_page(&self, page: &SourcePage) -> Result<()> {
        let markdown = serialize_page(&page.info, &page.body)?;
        write_atomic(&self.page_path(&page.info.source_id)?, markdown.as_bytes())
    }

    pub fn page_path(&self, source_id: &str) -> Result<PathBuf> {
        Ok(self.source_dir(source_id)?.join("index.md"))
    }

    pub fn original_path(&self, source_id: &str) -> Result<PathBuf> {
        let page = self.open_source(source_id)?;
        let asset = &page.info.asset;
        if !asset.starts_with("original") || Path::new(asset).components().count() != 1 {
            return Err(GardenError::Invalid(
                "The original's link is not a retained asset.".into(),
            ));
        }
        let bundle = fs::canonicalize(self.source_dir(source_id)?)?;
        let original = fs::canonicalize(bundle.join(asset))?;
        if original.parent() != Some(bundle.as_path()) {
            return Err(GardenError::Invalid(
                "The original's link leaves its source bundle.".into(),
            ));
        }
        Ok(original)
    }

    pub fn list_sources(&self, offset: usize) -> Result<SourceList> {
        let offset_i64 = i64::try_from(offset)
            .map_err(|_| GardenError::Invalid("Invalid page offset.".into()))?;
        let mut statement = self
            .index
            .prepare("SELECT summary FROM sources ORDER BY source_id LIMIT ?1 OFFSET ?2")?;
        let rows = statement.query_map(params![PAGE_SIZE + 1, offset_i64], |row| {
            row.get::<_, String>(0)
        })?;
        let mut sources = Vec::new();
        for row in rows {
            sources.push(
                serde_json::from_str::<SourceSummary>(&row?)
                    .map_err(|e| GardenError::Invalid(e.to_string()))?,
            );
        }
        let next_offset = if sources.len() > PAGE_SIZE {
            sources.pop();
            Some(offset + PAGE_SIZE)
        } else {
            None
        };
        Ok(SourceList {
            sources,
            next_offset,
        })
    }

    pub fn search_pages(&self, request: PageSearchRequest) -> Result<PageSearchResults> {
        let query = request.query.trim();
        let normalized_tags = request
            .tags
            .iter()
            .map(|tag| normalize_tag(tag))
            .collect::<Result<Vec<_>>>()?;
        let date_from = request
            .date_from
            .as_deref()
            .map(validate_iso_date)
            .transpose()?;
        let date_to = request
            .date_to
            .as_deref()
            .map(validate_iso_date)
            .transpose()?;
        if date_from
            .as_ref()
            .zip(date_to.as_ref())
            .is_some_and(|(from, to)| from > to)
        {
            return Err(GardenError::Invalid(
                "The start date must not be after the end date.".into(),
            ));
        }
        let processing_status = request.processing_status.as_deref();
        if processing_status.is_some_and(|status| {
            !["pending", "processing", "complete", "failed", "unavailable"].contains(&status)
        }) {
            return Err(GardenError::Invalid("Unknown processing status.".into()));
        }
        let mut where_parts = Vec::<String>::new();
        let mut values = Vec::<rusqlite::types::Value>::new();
        if !query.is_empty() {
            where_parts.push("page_search MATCH ?".into());
            values.push(fts_expression(query).into());
        }
        if !normalized_tags.is_empty() {
            let placeholders = vec!["?"; normalized_tags.len()].join(", ");
            where_parts.push(format!(
                "EXISTS (SELECT 1 FROM page_tags t WHERE t.page_id = page_search.page_id AND t.normalized IN ({placeholders}))"
            ));
            values.extend(normalized_tags.iter().cloned().map(Into::into));
        }
        if let Some(from) = date_from {
            where_parts.push("event_date IS NOT NULL AND event_date >= ?".into());
            values.push(from.into());
        }
        if let Some(to) = date_to {
            where_parts.push("event_date IS NOT NULL AND event_date <= ?".into());
            values.push(to.into());
        }
        if let Some(format) = request.format.as_deref().filter(|value| !value.is_empty()) {
            where_parts.push("lower(format) = lower(?)".into());
            values.push(format.to_lowercase().into());
        }
        if let Some(status) = processing_status {
            where_parts.push("processing_status = ?".into());
            values.push(status.to_owned().into());
        }
        let where_sql = if where_parts.is_empty() {
            "1 = 1".to_owned()
        } else {
            where_parts.join(" AND ")
        };
        let excerpt = if query.is_empty() {
            "substr(content, 1, 240)"
        } else {
            "snippet(page_search, 5, '', '', ' … ', 24)"
        };
        let sql = format!(
            "SELECT page_id, source_id, page_type, title, kind, {excerpt} AS excerpt,
                    event_date, format, extraction, processing_status, match_location
             FROM page_search
             WHERE {where_sql}
             ORDER BY CASE WHEN ? = '' THEN 0 WHEN lower(title) = lower(?) THEN 0
                           WHEN instr(lower(title), lower(?)) > 0 THEN 1 ELSE 2 END,
                      bm25(page_search), title COLLATE NOCASE, page_id
             LIMIT ? OFFSET ?"
        );
        values.extend([
            query.to_owned().into(),
            query.to_owned().into(),
            query.to_owned().into(),
            ((PAGE_SIZE + 1) as i64).into(),
            i64::try_from(request.offset)
                .map_err(|_| GardenError::Invalid("Invalid result offset.".into()))?
                .into(),
        ]);
        let mut statement = self.index.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, Option<String>>(10)?,
            ))
        })?;
        let mut pages = Vec::new();
        for row in rows {
            let (
                page_id,
                source_id,
                page_type,
                title,
                kind,
                excerpt,
                event_date,
                format,
                extraction,
                processing_status,
                match_location,
            ) = row?;
            let tags = self.page_tags(&page_id)?;
            let match_location = match_location
                .map(|value| serde_json::from_str(&value))
                .transpose()
                .map_err(|error| GardenError::Invalid(error.to_string()))?
                .or(self.find_match_location(
                    &page_type,
                    &page_id,
                    &source_id,
                    query,
                    &normalized_tags,
                )?);
            pages.push(PageResult {
                page_id,
                source_id: source_id.clone(),
                page_type,
                title: title.clone(),
                kind,
                excerpt: excerpt.trim().to_owned(),
                tags,
                format,
                event_date: event_date.clone(),
                extraction: parse_extraction(&extraction)?,
                processing_status,
                matched_by: if !query.is_empty()
                    && title.to_lowercase().contains(&query.to_lowercase())
                {
                    "title"
                } else if !normalized_tags.is_empty() {
                    "tag"
                } else {
                    "keyword"
                }
                .into(),
                match_location,
            });
        }
        let next_offset = if pages.len() > PAGE_SIZE {
            pages.pop();
            Some(request.offset.saturating_add(PAGE_SIZE))
        } else {
            None
        };
        let available_tags = {
            let mut statement = self
                .index
                .prepare("SELECT DISTINCT label FROM page_tags ORDER BY normalized")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        let available_formats = {
            let mut statement = self.index.prepare(
                "SELECT DISTINCT format FROM page_search WHERE format != '' ORDER BY format",
            )?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(PageSearchResults {
            pages,
            next_offset,
            available_tags,
            available_formats,
            available_statuses: ["pending", "processing", "complete", "failed", "unavailable"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        })
    }

    fn page_tags(&self, page_id: &str) -> Result<Vec<String>> {
        let mut statement = self.index.prepare(
            "SELECT DISTINCT label FROM page_tags WHERE page_id = ?1 ORDER BY normalized",
        )?;
        let rows = statement.query_map([page_id], |row| row.get::<_, String>(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(GardenError::from)
    }

    fn find_match_location(
        &self,
        page_type: &str,
        page_id: &str,
        source_id: &str,
        query: &str,
        tags: &[String],
    ) -> Result<Option<SearchMatchLocation>> {
        let terms = if query.trim().is_empty() {
            tags.first().map(String::as_str).unwrap_or("")
        } else {
            query
        }
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>();
        if terms.is_empty() {
            return Ok(None);
        }
        if page_type == "source" {
            let source = read_page(&self.page_path(source_id)?)?;
            let bytes = fs::read(self.original_path(source_id)?).unwrap_or_default();
            if let Ok(text) = String::from_utf8(bytes) {
                if let Some((start, end)) = find_text_match(&text, &terms) {
                    let line_start =
                        text[..start].bytes().filter(|byte| *byte == b'\n').count() + 1;
                    let line_end = text[..end].bytes().filter(|byte| *byte == b'\n').count() + 1;
                    return Ok(Some(SearchMatchLocation {
                        record_id: format!("source:{}", source.info.page_id),
                        source_id: source.info.source_id,
                        quote: text[start..end].to_owned(),
                        byte_start: start,
                        byte_end: end,
                        line_start,
                        line_end,
                    }));
                }
            }
            return Ok(None);
        }
        let path = self.root.join("pages").join(format!("{page_id}.md"));
        let markdown = match read_bounded_text(&path, MAX_PAGE_BYTES) {
            Ok(markdown) => markdown,
            Err(_) => return Ok(None),
        };
        let header = match parse_knowledge_header(&markdown) {
            Ok(header) => header,
            Err(_) => return Ok(None),
        };
        for fact in &header.facts {
            if contains_all_terms(
                &format!("{} {} {}", fact.property, fact.value, fact.evidence.quote),
                &terms,
            ) {
                return Ok(Some(search_location(
                    &fact.fact_id,
                    &header.source_id,
                    &fact.evidence,
                )));
            }
        }
        for relationship in &header.relationships {
            if contains_all_terms(
                &format!("{} {}", relationship.kind, relationship.evidence.quote),
                &terms,
            ) {
                return Ok(Some(search_location(
                    &relationship.relationship_id,
                    &header.source_id,
                    &relationship.evidence,
                )));
            }
        }
        for tag in &header.tags {
            if tags.contains(&tag.normalized) || contains_all_terms(&tag.label, &terms) {
                if let Some(support) = tag.supports.first() {
                    return Ok(Some(search_location(
                        &format!("tag:{}", tag.normalized),
                        &support.source_id,
                        &support.evidence,
                    )));
                }
            }
        }
        Ok(None)
    }

    pub fn rebuild_index(&mut self) -> Result<()> {
        let transaction = self.index.unchecked_transaction()?;
        transaction.execute("DELETE FROM sources", [])?;
        transaction.execute("DELETE FROM page_search", [])?;
        transaction.execute("DELETE FROM page_tags", [])?;
        for entry in fs::read_dir(self.root.join("sources"))? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let page = read_page(&entry.path().join("index.md"))?;
            transaction.execute(
                "INSERT INTO sources (source_id, summary) VALUES (?1, ?2)",
                params![page.info.source_id, summary_json(&page.info)?],
            )?;
            let documents = self.search_documents_for_source(&page.info)?;
            write_search_documents(&transaction, &documents)?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn index_page(&mut self, info: &SourceInfo) -> Result<()> {
        let page = read_page(&self.page_path(&info.source_id)?)?;
        let documents = self.search_documents_for_source(&page.info)?;
        let transaction = self.index.unchecked_transaction()?;
        transaction.execute(
            "INSERT OR REPLACE INTO sources (source_id, summary) VALUES (?1, ?2)",
            params![info.source_id, summary_json(info)?],
        )?;
        transaction.execute(
            "DELETE FROM page_search WHERE source_id = ?1",
            [&info.source_id],
        )?;
        transaction.execute(
            "DELETE FROM page_tags WHERE source_id = ?1",
            [&info.source_id],
        )?;
        write_search_documents(&transaction, &documents)?;
        transaction.commit()?;
        Ok(())
    }

    fn search_documents_for_source(&self, info: &SourceInfo) -> Result<Vec<SearchDocument>> {
        let source_page = read_page(&self.page_path(&info.source_id)?)?;
        let mut knowledge = Vec::new();
        for summary in &source_page.info.knowledge_pages {
            let path = self.root.join(&summary.path);
            let markdown = match read_bounded_text(&path, MAX_PAGE_BYTES) {
                Ok(markdown) => markdown,
                Err(_) => continue,
            };
            let header = match parse_knowledge_header(&markdown) {
                Ok(header) => header,
                Err(_) => continue,
            };
            let body = strip_frontmatter(&markdown).unwrap_or(&markdown).to_owned();
            knowledge.push((summary.clone(), header, body));
        }
        let source_event_date = knowledge
            .iter()
            .filter(|(_, header, _)| header.kind.eq_ignore_ascii_case("event"))
            .find_map(|(_, header, _)| event_date_from_facts(&header.facts));
        let mut documents = Vec::new();
        let source_text = if source_page.info.extraction == ExtractionState::TextPreserved {
            fs::read(self.original_path(&source_page.info.source_id)?)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .unwrap_or_else(|| source_page.body.clone())
        } else {
            source_page.body.clone()
        };
        documents.push(SearchDocument {
            page_id: source_page.info.page_id.clone(),
            source_id: source_page.info.source_id.clone(),
            page_type: "source".into(),
            title: source_page.info.title.clone(),
            kind: "source".into(),
            content: source_text,
            format: source_page.info.format.clone(),
            extraction: source_page.info.extraction,
            processing_status: processing_status(&source_page.info),
            event_date: source_event_date,
            tags: Vec::new(),
            doc_kind: "page".into(),
            match_location: None,
        });
        for (summary, header, body) in knowledge {
            let tags = header
                .tags
                .iter()
                .filter(|tag| !tag.supports.is_empty())
                .cloned()
                .collect::<Vec<_>>();
            let event_date = event_date_from_facts(&header.facts);
            documents.push(SearchDocument {
                page_id: summary.page_id.clone(),
                source_id: header.source_id.clone(),
                page_type: "knowledge".into(),
                title: summary.title.clone(),
                kind: summary.kind.clone(),
                content: body.clone(),
                format: source_page.info.format.clone(),
                extraction: source_page.info.extraction,
                processing_status: processing_status(&source_page.info),
                event_date: event_date.clone(),
                tags: tags.clone(),
                doc_kind: "page".into(),
                match_location: None,
            });
        }
        Ok(documents)
    }

    fn source_dir(&self, source_id: &str) -> Result<PathBuf> {
        let digest = source_id.strip_prefix("source-").unwrap_or("");
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(GardenError::Invalid("Invalid source identity.".into()));
        }
        Ok(self.root.join("sources").join(digest))
    }
}

fn summary_json(info: &SourceInfo) -> Result<String> {
    serde_json::to_string(&SourceSummary {
        source_id: info.source_id.clone(),
        page_id: info.page_id.clone(),
        title: info.title.clone(),
        extraction: info.extraction,
    })
    .map_err(|e| GardenError::Invalid(e.to_string()))
}

fn write_search_documents(
    transaction: &rusqlite::Transaction<'_>,
    documents: &[SearchDocument],
) -> Result<()> {
    for document in documents {
        let tags_for_fts = document
            .tags
            .iter()
            .map(|tag| tag.normalized.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let location = document
            .match_location
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| GardenError::Invalid(error.to_string()))?;
        transaction.execute(
            "INSERT INTO page_search
             (page_id, source_id, page_type, title, kind, content, tags, format,
              extraction, processing_status, event_date, doc_kind, match_location)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                document.page_id,
                document.source_id,
                document.page_type,
                document.title,
                document.kind,
                document.content,
                tags_for_fts,
                document.format,
                extraction_string(document.extraction),
                document.processing_status,
                document.event_date,
                document.doc_kind,
                location,
            ],
        )?;
        for tag in &document.tags {
            for support in &tag.supports {
                let support_json = serde_json::to_string(support)
                    .map_err(|error| GardenError::Invalid(error.to_string()))?;
                transaction.execute(
                    "INSERT OR IGNORE INTO page_tags
                     (page_id, normalized, label, source_id, source_version_id, support_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        document.page_id,
                        tag.normalized,
                        tag.label,
                        support.source_id,
                        support.source_version_id,
                        support_json,
                    ],
                )?;
            }
        }
    }
    Ok(())
}

fn extraction_string(value: ExtractionState) -> String {
    match value {
        ExtractionState::TextPreserved => "text_preserved",
        ExtractionState::Unsupported => "unsupported",
        ExtractionState::InvalidUtf8 => "invalid_utf8",
        ExtractionState::TooLarge => "too_large",
    }
    .to_owned()
}

fn parse_extraction(value: &str) -> Result<ExtractionState> {
    match value {
        "text_preserved" => Ok(ExtractionState::TextPreserved),
        "unsupported" => Ok(ExtractionState::Unsupported),
        "invalid_utf8" => Ok(ExtractionState::InvalidUtf8),
        "too_large" => Ok(ExtractionState::TooLarge),
        _ => Err(GardenError::Invalid(
            "Search index has an unknown extraction state.".into(),
        )),
    }
}

fn processing_status(info: &SourceInfo) -> String {
    if info.extraction != ExtractionState::TextPreserved {
        "unavailable".into()
    } else {
        info.semantic_state.clone()
    }
}

fn normalize_tag(label: &str) -> Result<String> {
    let display = label.trim().trim_start_matches('#').trim();
    if display.is_empty()
        || display.chars().count() > 80
        || display.chars().any(|character| {
            !(character.is_alphanumeric() || character.is_whitespace() || "_-".contains(character))
        })
    {
        return Err(GardenError::Invalid(
            "A tag must contain 1–80 letters, numbers, spaces, underscores, or hyphens.".into(),
        ));
    }
    Ok(display
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase())
}

fn validate_tag_label(label: &str) -> Result<()> {
    normalize_tag(label).map(|_| ())
}

fn fts_expression(query: &str) -> String {
    query
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{}\"*", term.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn validate_iso_date(value: &str) -> Result<String> {
    let date = value.trim();
    let bytes = date.as_bytes();
    let valid_shape = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());
    if !valid_shape {
        return Err(GardenError::Invalid(
            "Date filters must use YYYY-MM-DD.".into(),
        ));
    }
    let month = date[5..7].parse::<u32>().unwrap_or(0);
    let day = date[8..10].parse::<u32>().unwrap_or(0);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(GardenError::Invalid(
            "Date filter is outside the calendar.".into(),
        ));
    }
    Ok(date.to_owned())
}

fn event_date_from_facts(facts: &[FactRecord]) -> Option<String> {
    facts
        .iter()
        .filter(|fact| {
            ["event_date", "occurred_on", "date"]
                .iter()
                .any(|property| fact.property.eq_ignore_ascii_case(property))
        })
        .find_map(|fact| parse_date_value(&fact.value))
}

fn search_location(
    record_id: &str,
    source_id: &str,
    evidence: &EvidenceLocation,
) -> SearchMatchLocation {
    SearchMatchLocation {
        record_id: record_id.to_owned(),
        source_id: source_id.to_owned(),
        quote: evidence.quote.clone(),
        byte_start: evidence.byte_start,
        byte_end: evidence.byte_end,
        line_start: evidence.line_start,
        line_end: evidence.line_end,
    }
}

fn contains_all_terms(text: &str, terms: &[String]) -> bool {
    let text = text.to_lowercase();
    terms.iter().all(|term| text.contains(term))
}

fn find_text_match(text: &str, terms: &[String]) -> Option<(usize, usize)> {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        if contains_all_terms(content, terms) {
            return Some((offset, offset + content.len()));
        }
        offset += line.len();
    }
    None
}

fn parse_date_value(value: &str) -> Option<String> {
    let value = value
        .trim()
        .trim_matches(|character: char| !(character.is_ascii_alphanumeric() || character == '-'));
    if value.len() == 10
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && value
            .bytes()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return Some(value.to_owned());
    }
    let parts = value
        .trim_end_matches(',')
        .split(|character: char| character.is_whitespace() || character == ',')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() < 3 {
        return None;
    }
    let month = match parts[0].to_lowercase().as_str() {
        "jan" | "january" => 1,
        "feb" | "february" => 2,
        "mar" | "march" => 3,
        "apr" | "april" => 4,
        "may" => 5,
        "jun" | "june" => 6,
        "jul" | "july" => 7,
        "aug" | "august" => 8,
        "sep" | "sept" | "september" => 9,
        "oct" | "october" => 10,
        "nov" | "november" => 11,
        "dec" | "december" => 12,
        _ => return None,
    };
    let day = parts[1].parse::<u32>().ok()?;
    let year = parts[2].parse::<u32>().ok()?;
    if !(1..=31).contains(&day) || !(1000..=9999).contains(&year) {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

fn read_bounded_text(path: &Path, max_bytes: u64) -> Result<String> {
    let mut text = String::new();
    File::open(path)?
        .take(max_bytes + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > max_bytes {
        return Err(GardenError::Invalid(
            "A knowledge page exceeds the search limit.".into(),
        ));
    }
    Ok(text)
}

fn strip_frontmatter(markdown: &str) -> Option<&str> {
    markdown
        .strip_prefix("---\n")?
        .split_once("\n---\n")
        .map(|(_, body)| body.trim_start_matches('\n'))
}

fn parse_knowledge_header(markdown: &str) -> Result<KnowledgePageHeader> {
    let header = markdown
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map(|(header, _)| header)
        .ok_or_else(|| GardenError::Invalid("Knowledge page metadata is incomplete.".into()))?;
    serde_yaml_ng::from_str(header).map_err(GardenError::from)
}

struct UnavailableProvider;

impl SemanticProvider for UnavailableProvider {
    fn form_knowledge(
        &self,
        _source_text: &str,
    ) -> std::result::Result<KnowledgeDraft, ProviderError> {
        Err(ProviderError::recoverable(
            "Jev credentials are not available to this application process.".into(),
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EvidenceLocation {
    quote: String,
    byte_start: usize,
    byte_end: usize,
    line_start: usize,
    line_end: usize,
    origin: String,
    qualifier: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FactRecord {
    fact_id: String,
    subject_page_id: String,
    property: String,
    value: String,
    qualifier: Option<String>,
    origin: String,
    evidence: EvidenceLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RelationshipRecord {
    relationship_id: String,
    from_page_id: String,
    to_page_id: String,
    kind: String,
    qualifier: Option<String>,
    origin: String,
    evidence: EvidenceLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TagSupport {
    source_id: String,
    source_version_id: String,
    value: String,
    qualifier: Option<String>,
    origin: String,
    evidence: EvidenceLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TagRecord {
    normalized: String,
    label: String,
    supports: Vec<TagSupport>,
}

#[derive(Debug, Serialize, Deserialize)]
struct KnowledgePageHeader {
    schema: u32,
    page_id: String,
    source_id: String,
    title: String,
    kind: String,
    evidence: EvidenceLocation,
    #[serde(default)]
    facts: Vec<FactRecord>,
    #[serde(default)]
    relationships: Vec<RelationshipRecord>,
    #[serde(default)]
    tags: Vec<TagRecord>,
}

struct SearchDocument {
    page_id: String,
    source_id: String,
    page_type: String,
    title: String,
    kind: String,
    content: String,
    format: String,
    extraction: ExtractionState,
    processing_status: String,
    event_date: Option<String>,
    tags: Vec<TagRecord>,
    doc_kind: String,
    match_location: Option<SearchMatchLocation>,
}

fn validate_evidence(source: &str, evidence: &EvidenceDraft) -> Result<EvidenceLocation> {
    if evidence.quote.trim().is_empty()
        || evidence.byte_start >= evidence.byte_end
        || evidence.byte_end > source.len()
        || !source.is_char_boundary(evidence.byte_start)
        || !source.is_char_boundary(evidence.byte_end)
        || source.get(evidence.byte_start..evidence.byte_end) != Some(evidence.quote.as_str())
    {
        return Err(GardenError::Invalid(
            "Semantic evidence does not match its exact source byte range.".into(),
        ));
    }
    let line_start = source[..evidence.byte_start]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    let line_end_bound =
        if source.as_bytes().get(evidence.byte_end.saturating_sub(1)) == Some(&b'\n') {
            evidence.byte_end.saturating_sub(1)
        } else {
            evidence.byte_end
        };
    let line_end = source[..line_end_bound]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    Ok(EvidenceLocation {
        quote: evidence.quote.clone(),
        byte_start: evidence.byte_start,
        byte_end: evidence.byte_end,
        line_start,
        line_end,
        origin: evidence.origin.clone(),
        qualifier: evidence.qualifier.clone(),
    })
}

fn stable_page_id(source_id: &str, entity: &EntityDraft) -> String {
    let mut digest = Sha256::new();
    digest.update(source_id.as_bytes());
    digest.update([0]);
    digest.update(entity.kind.to_lowercase().as_bytes());
    digest.update([0]);
    digest.update(entity.label.to_lowercase().as_bytes());
    digest.update([0]);
    digest.update(entity.evidence.byte_start.to_be_bytes());
    format!("page-{:x}", digest.finalize())
}

fn stable_record_id(
    prefix: &str,
    source_id: &str,
    kind: &str,
    value: &str,
    offset: usize,
) -> String {
    let mut digest = Sha256::new();
    digest.update(source_id.as_bytes());
    digest.update([0]);
    digest.update(kind.as_bytes());
    digest.update([0]);
    digest.update(value.as_bytes());
    digest.update([0]);
    digest.update(offset.to_be_bytes());
    format!("{prefix}-{:x}", digest.finalize())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        GardenError::Invalid("Cannot publish a file without a parent directory.".into())
    })?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path)
        .map_err(|error| GardenError::Io(error.error))?;
    sync_directory(parent)?;
    Ok(())
}

fn replace_semantic_section(body: &str, pages: &[KnowledgePageSummary]) -> String {
    const START: &str = "<!-- knowledge-garden:semantic:start -->";
    const END: &str = "<!-- knowledge-garden:semantic:end -->";
    let body = if let Some(start) = body.find(START) {
        if let Some(relative_end) = body[start..].find(END) {
            let end = start + relative_end + END.len();
            format!("{}{}", &body[..start], &body[end..])
        } else {
            body[..start].to_owned()
        }
    } else {
        body.to_owned()
    };
    let mut section = format!("\n\n{START}\n\n## Knowledge\n\n");
    if pages.is_empty() {
        section.push_str("No supported facts or relationships were selected from this source.\n");
    } else {
        for page in pages {
            section.push_str(&format!(
                "- [{}]({}) ({})\n",
                escape_markdown(&page.title),
                page.path,
                escape_markdown(&page.kind)
            ));
        }
    }
    section.push_str(&format!("\n{END}\n"));
    format!("{}{}", body.trim_end(), section)
}

fn escape_heading(value: &str) -> String {
    value
        .replace(['\n', '\r'], " ")
        .replace(['[', ']', '<', '>'], "")
}

fn escape_markdown(value: &str) -> String {
    value
        .replace(['\n', '\r'], " ")
        .replace(['[', ']', '<', '>', '*', '_', '`'], "")
}

fn read_knowledge_page(path: &Path) -> Result<KnowledgePage> {
    let mut markdown = String::new();
    File::open(path)?
        .take(256 * 1024)
        .read_to_string(&mut markdown)?;
    let header = markdown
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n").map(|(yaml, _)| yaml))
        .ok_or_else(|| GardenError::Invalid("Knowledge page metadata is unreadable.".into()))?;
    let header: KnowledgePageHeader = serde_yaml_ng::from_str(header)?;
    Ok(KnowledgePage {
        page_id: header.page_id,
        source_id: header.source_id,
        title: header.title,
        kind: header.kind,
        markdown,
    })
}

fn now_millis() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| GardenError::Invalid(error.to_string()))?
        .as_millis())
}

fn source_body(info: &SourceInfo, text: &str) -> String {
    let title = info
        .title
        .replace(['\n', '\r'], " ")
        .replace(['[', ']', '<', '>'], "");
    if info.extraction != ExtractionState::TextPreserved {
        return format!(
            "# {title}\n\n[Open original]({})\n\nText is unavailable. {}\n",
            info.asset, info.extraction_detail
        );
    }
    if text.is_empty() {
        return format!(
            "# {title}\n\n[Open original]({})\n\n## Source text\n\nSource text is empty.\n",
            info.asset
        );
    }
    let longest_run = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(3.max(longest_run + 1));
    format!("# {title}\n\n[Open original]({})\n\n## Source text · lines 1–{}\n\n{fence}text\n{text}\n{fence}\n",
        info.asset, info.line_count)
}

fn serialize_page(info: &SourceInfo, body: &str) -> Result<String> {
    Ok(format!(
        "---\n{}---\n\n{body}",
        serde_yaml_ng::to_string(info)?
    ))
}

fn read_page(path: &Path) -> Result<SourcePage> {
    let mut markdown = String::new();
    File::open(path)?
        .take(MAX_PAGE_BYTES + 1)
        .read_to_string(&mut markdown)?;
    if markdown.len() as u64 > MAX_PAGE_BYTES {
        return Err(GardenError::Invalid(
            "This source page exceeds the reader's size limit.".into(),
        ));
    }
    let remainder = markdown
        .strip_prefix("---\n")
        .ok_or_else(|| GardenError::Invalid("The source page has no metadata header.".into()))?;
    let (header, body) = remainder.split_once("\n---\n").ok_or_else(|| {
        GardenError::Invalid("The source page metadata header is incomplete.".into())
    })?;
    let info: SourceInfo = serde_yaml_ng::from_str(header)?;
    let body = body.trim_start_matches('\n').to_owned();
    let knowledge_pages = info.knowledge_pages.clone();
    Ok(SourcePage {
        info,
        markdown,
        body,
        knowledge_pages,
    })
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}
