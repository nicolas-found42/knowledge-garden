//! Public, disk-backed collection operations. Markdown and originals are authoritative.
use crate::office::{CoveragePart, CoverageScope, CoverageStatus, OfficeProjection};
use crate::semantic::{
    EntityDraft, EvidenceDraft, KnowledgeDraft, KnowledgePage, KnowledgePageSummary, ProviderError,
    SemanticDecision, SemanticJob, SemanticProvider, SourceUpdateRole,
};
use fs2::FileExt;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
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
    #[error("A recoverable publication record is unreadable: {0}")]
    Publication(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, GardenError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionMethod {
    Picker,
    Drop,
    Url,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionState {
    TextPreserved,
    StructuredText,
    PartialText,
    InvalidContainer,
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
    #[serde(default)]
    pub requested_url: Option<String>,
    #[serde(default)]
    pub final_url: Option<String>,
    #[serde(default)]
    pub http_status: Option<u16>,
    #[serde(default)]
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceVersion {
    pub source_version_id: String,
    pub sha256: String,
    pub bytes: u64,
    pub original_name: String,
    pub asset: String,
    pub format: String,
    pub received_at: String,
    #[serde(default)]
    pub source_date: Option<String>,
    #[serde(default)]
    pub source_revision: Option<u64>,
    #[serde(default)]
    pub order_basis: String,
    #[serde(default)]
    pub update_role: String,
    #[serde(default)]
    pub update_evidence: Option<EvidenceDraft>,
    #[serde(default)]
    pub update_confidence: Option<f64>,
    #[serde(default)]
    pub source_date_evidence: Option<EvidenceDraft>,
    #[serde(default)]
    pub source_date_confidence: Option<f64>,
    #[serde(default)]
    pub source_revision_evidence: Option<EvidenceDraft>,
    #[serde(default)]
    pub source_revision_confidence: Option<f64>,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub coverage: String,
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
    #[serde(default)]
    pub extraction_coverage: Option<Vec<CoveragePart>>,
    pub line_count: usize,
    pub acquisitions: Vec<Acquisition>,
    #[serde(default)]
    pub current_version_id: Option<String>,
    #[serde(default)]
    pub pending_version_id: Option<String>,
    #[serde(default)]
    pub versions_seen: Vec<SourceVersion>,
    #[serde(default)]
    pub update_status: Option<String>,
    #[serde(default)]
    pub ordering_uncertain: bool,
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
    #[serde(default)]
    pub source_version_id: Option<String>,
    pub quote: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub line_start: usize,
    pub line_end: usize,
    #[serde(default = "preserved_text_basis")]
    pub offset_basis: String,
    #[serde(default)]
    pub source_location: Option<String>,
}

pub struct Application {
    root: PathBuf,
    index: Connection,
    // The lock survives for this application's lifetime, including across commands.
    _lock: File,
    semantic_provider: Arc<dyn SemanticProvider>,
    staged_publication: Vec<(PathBuf, Vec<u8>)>,
    fail_after_publication_files: Option<usize>,
}

fn semantic_pending() -> String {
    "pending".into()
}

impl Application {
    /// Injects a one-shot failure after the specified number of destination
    /// files have been atomically replaced. Intended for public-application
    /// recovery verification.
    pub fn set_publication_failpoint_for_test(&mut self, after_files: usize) {
        self.fail_after_publication_files = Some(after_files);
    }

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
        recover_publications(&root)?;
        fs::create_dir_all(root.join(".derived"))?;
        let index = Connection::open(root.join(".derived/lookup.sqlite"))?;
        index.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS sources (
                 source_id TEXT PRIMARY KEY, summary TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS source_origins (
                 path TEXT PRIMARY KEY, source_id TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS source_versions (
                 source_id TEXT NOT NULL, source_version_id TEXT NOT NULL, format TEXT NOT NULL,
                 PRIMARY KEY(source_id, source_version_id)
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
            staged_publication: Vec::new(),
            fail_after_publication_files: None,
        };
        app.rebuild_index()?;
        Ok(app)
    }

    pub fn import_source(
        &mut self,
        path: impl AsRef<Path>,
        method: AcquisitionMethod,
    ) -> Result<SourcePage> {
        self.import_source_with_acquisition(path, method, None, None)
    }

    pub fn import_url(&mut self, input: &str) -> Result<SourcePage> {
        let requested = reqwest::Url::parse(input)
            .map_err(|_| GardenError::Invalid("Enter a valid HTTP or HTTPS URL.".into()))?;
        if !matches!(requested.scheme(), "http" | "https")
            || requested.host_str().is_none()
            || !requested.username().is_empty()
            || requested.password().is_some()
        {
            return Err(GardenError::Invalid(
                "Enter a valid HTTP or HTTPS URL.".into(),
            ));
        }
        let requested = requested.to_string();
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(8))
            .build()
            .map_err(|_| GardenError::Invalid("Could not prepare the URL request.".into()))?;
        let mut response = client
            .get(&requested)
            .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml,text/plain,application/vnd.openxmlformats-officedocument.*,application/octet-stream;q=0.5,*/*;q=0.1")
            .send()
            .map_err(|_| GardenError::Invalid("The URL could not be retrieved. Check it and retry.".into()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(GardenError::Invalid(format!(
                "URL acquisition returned HTTP {}; no source material was added.",
                status.as_u16()
            )));
        }
        let final_url = response.url().to_string();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| {
                value
                    .split(';')
                    .next()
                    .unwrap_or(value)
                    .trim()
                    .to_ascii_lowercase()
            });
        let disposition_name = response
            .headers()
            .get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|value| value.to_str().ok())
            .and_then(content_disposition_filename);
        let (original_name, format) = url_name_and_format(
            &final_url,
            content_type.as_deref(),
            disposition_name.as_deref(),
        );
        let staging = tempfile::tempdir_in(self.root.join(".staging"))?;
        let remote_file = staging.path().join(&original_name);
        let mut output = File::create(&remote_file)?;
        let mut chunk = [0_u8; 64 * 1024];
        loop {
            let read = response.read(&mut chunk).map_err(|_| {
                GardenError::Invalid(
                    "The URL response could not be read completely; no source material was added."
                        .into(),
                )
            })?;
            if read == 0 {
                break;
            }
            output.write_all(&chunk[..read])?;
        }
        output.sync_all()?;
        let acquisition = Acquisition {
            path: requested.clone(),
            method: AcquisitionMethod::Url,
            received_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| GardenError::Invalid(e.to_string()))?
                .as_millis()
                .to_string(),
            requested_url: Some(requested),
            final_url: Some(final_url),
            http_status: Some(status.as_u16()),
            content_type,
        };
        self.import_source_with_acquisition(
            &remote_file,
            AcquisitionMethod::Url,
            Some(acquisition),
            Some(format),
        )
    }

    fn import_source_with_acquisition(
        &mut self,
        path: impl AsRef<Path>,
        method: AcquisitionMethod,
        acquisition_override: Option<Acquisition>,
        format_override: Option<String>,
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
        let format = format_override.unwrap_or_else(|| extension.to_lowercase());
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
        let received_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| GardenError::Invalid(e.to_string()))?
            .as_millis()
            .to_string();
        let acquisition = acquisition_override.unwrap_or_else(|| Acquisition {
            path: supplied_path.to_string_lossy().into_owned(),
            method,
            received_at,
            requested_url: None,
            final_url: None,
            http_status: None,
            content_type: None,
        });
        let path_key = acquisition.path.clone();
        let mut identity = Sha256::new();
        identity.update(format.as_bytes());
        identity.update([0]);
        identity.update(digest.as_bytes());
        let content_source_id = format!("source-{:x}", identity.finalize());
        let existing_origin = self.source_for_origin(&path_key)?;
        let source_id = if method == AcquisitionMethod::Url {
            existing_origin.unwrap_or_else(|| {
                let identity = format!("url-origin:{path_key}");
                format!("source-{:x}", Sha256::digest(identity.as_bytes()))
            })
        } else {
            existing_origin
                .or(self.source_for_version(&digest, &format)?)
                .unwrap_or(content_source_id)
        };
        let destination = self.source_dir(&source_id)?;
        if destination.exists() {
            let mut existing = self.open_source(&source_id)?;
            let acquisition_exists = existing.info.acquisitions.iter().any(|old| {
                old.path == acquisition.path
                    && old.method == acquisition.method
                    && old.requested_url == acquisition.requested_url
                    && old.final_url == acquisition.final_url
            });
            if existing
                .info
                .versions_seen
                .iter()
                .any(|version| version.source_version_id == digest)
            {
                if !acquisition_exists {
                    write_acquisition(&destination, &acquisition)?;
                }
                self.index_acquisition(&acquisition.path, &source_id)?;
                return self.open_source(&source_id);
            }
            let (extraction, extraction_detail, _text) =
                if method == AcquisitionMethod::Url && format == "html" {
                    match crate::web::project(
                        &text_bytes,
                        acquisition.final_url.as_deref().unwrap_or(""),
                    ) {
                        Ok(web) if count > MAX_TEXT_BYTES as u64 => {
                            (ExtractionState::PartialText, web.detail, web.semantic_text)
                        }
                        Ok(web) => (
                            ExtractionState::StructuredText,
                            web.detail,
                            web.semantic_text,
                        ),
                        Err(detail) => (ExtractionState::InvalidUtf8, detail, String::new()),
                    }
                } else {
                    let (extraction, detail, text) =
                        classify_source(&format, count, text_bytes.clone());
                    (extraction, detail.to_owned(), text)
                };
            if !matches!(
                extraction,
                ExtractionState::TextPreserved
                    | ExtractionState::StructuredText
                    | ExtractionState::PartialText
            ) {
                if !acquisition_exists {
                    write_acquisition(&destination, &acquisition)?;
                }
                existing.info.update_status = Some("incomplete".into());
                existing.info.semantic_error = Some(format!(
                    "The changed source version was retained, but {} prevented a complete replacement.",
                    extraction_detail
                ));
                self.write_source_page(&existing)?;
                self.index_page(&existing.info)?;
                return self.open_source(&source_id);
            }
            let version = source_version(
                &digest,
                count,
                &original_name,
                &asset,
                &format,
                &acquisition.received_at,
                "pending",
            );
            let version_path = self.version_original_path(&source_id, &version)?;
            write_atomic_from_file(&version_path, &staging.path().join(&asset))?;
            existing.info.versions_seen.push(version);
            existing.info.pending_version_id = Some(digest);
            existing.info.update_status = Some("pending".into());
            existing.info.semantic_error = None;
            existing.info.semantic_retry_at = None;
            if !acquisition_exists {
                write_acquisition(&destination, &acquisition)?;
            }
            self.index_acquisition(&acquisition.path, &source_id)?;
            self.write_source_page(&existing)?;
            self.index_page(&existing.info)?;
            return self.open_source(&source_id);
        }
        let web_result = if method == AcquisitionMethod::Url && format == "html" {
            let mut projection_bytes = text_bytes.clone();
            if count > MAX_TEXT_BYTES as u64 {
                if let Err(error) = std::str::from_utf8(&projection_bytes) {
                    if error.error_len().is_none() {
                        projection_bytes.truncate(error.valid_up_to());
                    }
                }
            }
            Some(crate::web::project(
                &projection_bytes,
                acquisition.final_url.as_deref().unwrap_or(""),
            ))
        } else {
            None
        };
        let web_projection = web_result.as_ref().and_then(|result| result.as_ref().ok());
        let (extraction, extraction_detail, text, office_projection, extraction_coverage) =
            if let Some(Err(_)) = &web_result {
                (ExtractionState::InvalidUtf8, "The downloaded HTML is not valid UTF-8. The exact original is retained without lossy extraction.".into(), String::new(), None, None)
            } else if let Some(web) = web_projection {
                if count > MAX_TEXT_BYTES as u64 {
                    (ExtractionState::PartialText, format!("{} Only the first 2 MiB of the response was projected; the complete original is retained.", web.detail), web.semantic_text.clone(), None, None)
                } else {
                    (
                        ExtractionState::StructuredText,
                        web.detail.clone(),
                        web.semantic_text.clone(),
                        None,
                        None,
                    )
                }
            } else if method == AcquisitionMethod::Url
                && acquisition.content_type.as_deref() == Some("text/plain")
                && count <= MAX_TEXT_BYTES as u64
            {
                match String::from_utf8(text_bytes.clone()) {
                Err(_) => (ExtractionState::InvalidUtf8, "The source is not valid UTF-8. The original is retained without lossy decoding.".into(), String::new(), None, None),
                Ok(text) => (ExtractionState::TextPreserved, "The complete UTF-8 response text is preserved; the original remains available.".into(), text.trim_start_matches('\u{feff}').to_owned(), None, None),
            }
            } else if matches!(format.as_str(), "docx" | "pptx") {
                match crate::office::extract(
                    &staging.path().join(&asset),
                    &format,
                    path.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .as_ref(),
                ) {
                    Ok(projection) => {
                        let extraction = if projection.partial {
                            ExtractionState::PartialText
                        } else {
                            ExtractionState::StructuredText
                        };
                        (
                            extraction,
                            projection.detail.clone(),
                            projection.semantic_text.clone(),
                            Some(projection.clone()),
                            Some(projection.coverage.clone()),
                        )
                    }
                    Err(detail) => {
                        let scope = if format == "docx" {
                            CoverageScope::MainDocument
                        } else {
                            CoverageScope::SlideText
                        };
                        (
                            ExtractionState::InvalidContainer,
                            format!("Office container could not be read. {detail}"),
                            String::new(),
                            None,
                            Some(vec![CoveragePart {
                                scope,
                                status: CoverageStatus::Failed,
                                source_location: Some("package".into()),
                                detail,
                            }]),
                        )
                    }
                }
            } else if !matches!(format.as_str(), "" | "txt" | "text" | "md" | "markdown") {
                (
                    ExtractionState::Unsupported,
                    "This format is not supported by the text importer. The original is retained."
                        .into(),
                    String::new(),
                    None,
                    None,
                )
            } else if count > MAX_TEXT_BYTES as u64 {
                (
                    ExtractionState::TooLarge,
                    "Text exceeds the 2 MiB preview limit. The complete original is retained."
                        .into(),
                    String::new(),
                    None,
                    None,
                )
            } else {
                match String::from_utf8(text_bytes) {
                Err(_) => (ExtractionState::InvalidUtf8, "The source is not valid UTF-8. The original is retained without lossy decoding.".into(), String::new(), None, None),
                Ok(text) if text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) =>
                    (ExtractionState::Unsupported, "The source contains binary control characters. The original is retained.".into(), String::new(), None, None),
                Ok(text) => (ExtractionState::TextPreserved, "The complete UTF-8 source text is preserved; the original remains available.".into(), text.trim_start_matches('\u{feff}').to_owned(), None, None),
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
            title: web_projection
                .as_ref()
                .map(|web| {
                    if web.title.is_empty() {
                        original_name.clone()
                    } else {
                        web.title.clone()
                    }
                })
                .unwrap_or_else(|| {
                    path.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                }),
            original_name: original_name.clone(),
            asset: asset.clone(),
            sha256: digest.clone(),
            bytes: count,
            format: format.clone(),
            extraction,
            extraction_detail,
            extraction_coverage,
            line_count,
            acquisitions: vec![acquisition.clone()],
            current_version_id: None,
            pending_version_id: matches!(
                extraction,
                ExtractionState::TextPreserved
                    | ExtractionState::StructuredText
                    | ExtractionState::PartialText
            )
            .then(|| digest.clone()),
            versions_seen: vec![source_version(
                &digest,
                count,
                &original_name,
                &asset,
                &format,
                &acquisition.received_at,
                if matches!(
                    extraction,
                    ExtractionState::TextPreserved
                        | ExtractionState::StructuredText
                        | ExtractionState::PartialText
                ) {
                    "pending"
                } else {
                    "unavailable"
                },
            )],
            update_status: matches!(
                extraction,
                ExtractionState::TextPreserved
                    | ExtractionState::StructuredText
                    | ExtractionState::PartialText
            )
            .then(|| "pending".into()),
            ordering_uncertain: false,
            semantic_state: if matches!(
                extraction,
                ExtractionState::TextPreserved
                    | ExtractionState::StructuredText
                    | ExtractionState::PartialText
            ) {
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
        let body = web_projection
            .as_ref()
            .map(|web| web.markdown.replace("ORIGINAL_ASSET", &info.asset))
            .unwrap_or_else(|| source_body(&info, &text, office_projection.as_ref()));
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
        self.index_acquisition(&page.info.acquisitions[0].path, &page.info.source_id)?;
        self.index_version(&page.info.source_id, &digest, &page.info.format)?;
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
            let source_id = read_page(&entry.path().join("index.md"))?.info.source_id;
            let page = self.open_source(&source_id)?;
            if page.info.semantic_state == "processing"
                || page.info.update_status.as_deref() == Some("processing")
            {
                let mut interrupted = page;
                if interrupted.info.current_version_id.is_none() {
                    interrupted.info.semantic_state = "pending".into();
                } else {
                    interrupted.info.update_status = Some("pending".into());
                }
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
            let has_pending_version = page.info.pending_version_id.is_some();
            if (page.info.semantic_state == "pending"
                || (has_pending_version && page.info.update_status.as_deref() == Some("pending")))
                && retry_at <= now
            {
                due.push(page.info.source_id);
            }
        }
        let mut jobs = Vec::new();
        for source_id in due {
            let mut page = read_page(&self.page_path(&source_id)?)?;
            let Some(source_version_id) = page.info.pending_version_id.clone() else {
                continue;
            };
            let initial = page.info.current_version_id.is_none();
            if !matches!(
                page.info.extraction,
                ExtractionState::TextPreserved
                    | ExtractionState::StructuredText
                    | ExtractionState::PartialText
            ) || (initial && page.info.semantic_state != "pending")
                || (!initial && page.info.update_status.as_deref() != Some("pending"))
            {
                continue;
            }
            let version = page
                .info
                .versions_seen
                .iter_mut()
                .find(|version| version.source_version_id == source_version_id)
                .ok_or_else(|| GardenError::Invalid("Pending source version is missing.".into()))?;
            let candidate_path = if initial {
                self.source_dir(&source_id)?.join(&version.asset)
            } else {
                self.source_dir(&source_id)?
                    .join("versions")
                    .join(&source_version_id)
                    .join(&version.asset)
            };
            let text = if page.info.format == "html"
                && page
                    .info
                    .acquisitions
                    .iter()
                    .any(|acquisition| acquisition.method == AcquisitionMethod::Url)
            {
                let bytes = fs::read(&candidate_path)?;
                let final_url = page
                    .info
                    .acquisitions
                    .iter()
                    .find_map(|acquisition| acquisition.final_url.as_deref())
                    .unwrap_or("");
                let mut prefix = bytes[..bytes.len().min(MAX_TEXT_BYTES)].to_vec();
                if let Err(error) = std::str::from_utf8(&prefix) {
                    if error.error_len().is_none() {
                        prefix.truncate(error.valid_up_to());
                    }
                }
                crate::web::project(&prefix, final_url)
                    .map_err(GardenError::Invalid)?
                    .semantic_text
            } else if matches!(
                page.info.extraction,
                ExtractionState::StructuredText | ExtractionState::PartialText
            ) {
                crate::office::extract(&candidate_path, &page.info.format, &page.info.title)
                    .map_err(GardenError::Invalid)?
                    .semantic_text
            } else {
                let bytes = fs::read(candidate_path)?;
                String::from_utf8(bytes).map_err(|_| {
                    GardenError::Invalid("The retained text source is no longer UTF-8.".into())
                })?
            };
            if initial {
                page.info.semantic_state = "processing".into();
            } else {
                page.info.update_status = Some("processing".into());
            }

            page.info.semantic_attempts = page.info.semantic_attempts.saturating_add(1);
            page.info.semantic_error = None;
            page.info.semantic_retry_at = None;
            let attempt = page.info.semantic_attempts;
            version.state = "processing".into();
            self.write_source_page(&page)?;
            self.index_page(&page.info)?;
            jobs.push(SemanticJob {
                source_id,
                source_version_id,
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
        self.staged_publication.clear();
        let mut page = read_page(&self.page_path(&job.source_id)?)?;
        let initial = page.info.current_version_id.is_none();
        let active = if initial {
            page.info.semantic_state == "processing"
        } else {
            page.info.update_status.as_deref() == Some("processing")
        };
        if !active
            || page.info.pending_version_id.as_deref() != Some(&job.source_version_id)
            || page.info.semantic_attempts != job.attempt
        {
            return Err(GardenError::Invalid(
                "The semantic job is no longer the active source attempt.".into(),
            ));
        }
        let web_projection = page.info.format == "html"
            && page
                .info
                .acquisitions
                .iter()
                .any(|acquisition| acquisition.method == AcquisitionMethod::Url);
        let result = result.map(|mut draft| {
            if web_projection {
                mark_web_projection_evidence(&mut draft);
            }
            draft
        });
        match result {
            Ok(draft) => {
                let update = draft.source_update.clone();
                let result = self
                    .store_source_update(
                        &mut page,
                        &job.source_version_id,
                        &job.source_text,
                        update.as_ref(),
                    )
                    .and_then(|()| {
                        if initial {
                            // Classification controls replacement of existing current facts.
                            // On an initial acquisition there is nothing to withdraw, so keep
                            // grounded facts and preserve any uncertainty in their qualifiers.
                            self.publish_knowledge(&mut page, &job.source_text, draft)
                        } else {
                            self.apply_replacement(
                                &mut page,
                                &job.source_version_id,
                                &job.source_text,
                                draft,
                            )
                        }
                    });
                match result {
                    Ok(()) => {
                        page.info.semantic_state = "complete".into();
                        page.info.semantic_error = None;
                        page.info.semantic_retry_at = None;
                    }
                    Err(error) => {
                        self.record_semantic_failure(&mut page, error.to_string(), true)?
                    }
                }
            }
            Err(error) => {
                if initial {
                    self.record_semantic_failure(&mut page, error.message, error.retryable)?
                } else {
                    let retryable = error.retryable;
                    page.info.update_status =
                        Some(if retryable { "pending" } else { "failed" }.into());
                    page.info.semantic_error = Some(error.message.chars().take(300).collect());
                    page.info.semantic_retry_at = retryable.then(|| {
                        let delay = (1_u64 << page.info.semantic_attempts.min(12)).min(3600);
                        (now_millis().unwrap_or(0) + u128::from(delay * 1000)).to_string()
                    });
                    if let Some(version) = page
                        .info
                        .versions_seen
                        .iter_mut()
                        .find(|version| version.source_version_id == job.source_version_id)
                    {
                        version.state = page.info.update_status.clone().unwrap_or_default();
                    }
                }
            }
        }
        if self.staged_publication.is_empty() {
            self.write_source_page(&page)?;
        } else {
            let markdown = serialize_page(&page.info, &page.body)?;
            self.staged_publication
                .push((self.page_path(&page.info.source_id)?, markdown.into_bytes()));
            let files = std::mem::take(&mut self.staged_publication);
            let fail_after = self.fail_after_publication_files.take();
            commit_publication(&self.root, files, fail_after)?;
        }
        self.index_page(&page.info)?;
        Ok(())
    }

    fn store_source_update(
        &self,
        page: &mut SourcePage,
        version_id: &str,
        source_text: &str,
        decision: Option<&crate::semantic::SourceUpdateDraft>,
    ) -> Result<()> {
        let Some(version) = page
            .info
            .versions_seen
            .iter_mut()
            .find(|version| version.source_version_id == version_id)
        else {
            return Err(GardenError::Invalid(
                "The semantic result references a source version that is no longer retained."
                    .into(),
            ));
        };
        let Some(decision) = decision else {
            version.update_role = "unknown".into();
            version.update_evidence = None;
            version.update_confidence = None;
            version.source_date = None;
            version.source_revision = None;
            version.source_date_evidence = None;
            version.source_date_confidence = None;
            version.source_revision_evidence = None;
            version.source_revision_confidence = None;
            version.order_basis = "arrival_fallback".into();
            return Ok(());
        };
        if !decision.certainty.is_finite() || !(0.0..=1.0).contains(&decision.certainty) {
            return Err(GardenError::Invalid(
                "Jev returned an invalid source-update confidence.".into(),
            ));
        }
        for confidence in [
            decision.source_date_certainty,
            decision.source_revision_certainty,
        ] {
            if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
                return Err(GardenError::Invalid(
                    "Jev returned an invalid source-order confidence.".into(),
                ));
            }
        }
        validate_evidence(source_text, &decision.evidence)?;
        let judged_source_date_evidence = decision
            .source_date_evidence
            .as_ref()
            .map(|evidence| validate_evidence(source_text, evidence))
            .transpose()?;
        let judged_source_revision_evidence = decision
            .source_revision_evidence
            .as_ref()
            .map(|evidence| validate_evidence(source_text, evidence))
            .transpose()?;
        let judged_source_date = decision
            .source_date
            .as_deref()
            .map(validate_iso_date)
            .transpose()?;
        if judged_source_date.is_some() != judged_source_date_evidence.is_some()
            || decision.source_revision.is_some() != judged_source_revision_evidence.is_some()
            || decision.source_revision == Some(0)
        {
            return Err(GardenError::Invalid(
                "Jev returned incomplete source-order evidence.".into(),
            ));
        }
        let accept_source_date = decision.source_date_certainty >= 0.8;
        let accept_source_revision = decision.source_revision_certainty >= 0.8;
        version.update_role = decision.role.as_str().into();
        version.update_evidence = Some(decision.evidence.clone());
        version.update_confidence = Some(decision.certainty);
        version.source_date = accept_source_date.then_some(judged_source_date).flatten();
        version.source_revision = accept_source_revision
            .then_some(decision.source_revision)
            .flatten();
        version.source_date_evidence = if version.source_date.is_some() {
            decision.source_date_evidence.clone()
        } else {
            None
        };
        version.source_date_confidence = version
            .source_date
            .is_some()
            .then_some(decision.source_date_certainty);
        version.source_revision_evidence = if version.source_revision.is_some() {
            decision.source_revision_evidence.clone()
        } else {
            None
        };
        version.source_revision_confidence = version
            .source_revision
            .is_some()
            .then_some(decision.source_revision_certainty);
        version.order_basis = if version.source_date.is_some() {
            "provider_judged_source_date".into()
        } else if version.source_revision.is_some() {
            "provider_judged_revision".into()
        } else {
            "arrival_fallback".into()
        };
        Ok(())
    }

    fn retain_non_authoritative_update(
        &self,
        page: &mut SourcePage,
        version_id: &str,
        decision: Option<&crate::semantic::SourceUpdateDraft>,
    ) -> Result<()> {
        let role = decision
            .map(|decision| decision.role.as_str())
            .unwrap_or("unknown");
        page.info.pending_version_id = None;
        page.info.update_status = Some("uncertain".into());
        page.info.semantic_error = Some(format!(
            "The new source was classified as {role}; its retained text and update evidence were recorded, while current knowledge remains unchanged."
        ));
        if let Some(version) = page
            .info
            .versions_seen
            .iter_mut()
            .find(|version| version.source_version_id == version_id)
        {
            version.state = "retained_without_current_change".into();
        }
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
        &mut self,
        source: &mut SourcePage,
        source_text: &str,
        draft: KnowledgeDraft,
    ) -> Result<()> {
        let prior_pages = source.info.knowledge_pages.clone();
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
            let page_id = stable_page_id(entity, &source.info.source_id);
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
                        fact_id: stable_fact_id(
                            &summary.page_id,
                            &fact.property,
                            fact.record_key.as_deref(),
                        ),
                        subject_page_id: summary.page_id.clone(),
                        property: fact.property.clone(),
                        record_key: fact.record_key.clone(),
                        value: fact.value.clone(),
                        qualifier: evidence.qualifier.clone(),
                        origin: evidence.origin.clone(),
                        evidence,
                        supports: vec![SupportRecord {
                            source_id: source.info.source_id.clone(),
                            source_version_id: source
                                .info
                                .pending_version_id
                                .clone()
                                .unwrap_or_else(|| source.info.sha256.clone()),
                            value: fact.value.clone(),
                            qualifier: fact.evidence.qualifier.clone(),
                            origin: fact.evidence.origin.clone(),
                            evidence: validate_evidence(source_text, &fact.evidence)?,
                        }],
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
                        relationship_id: stable_relationship_id(
                            &from.page_id,
                            &to.page_id,
                            &relationship.kind,
                        ),
                        from_page_id: from.page_id.clone(),
                        to_page_id: to.page_id.clone(),
                        kind: relationship.kind.clone(),
                        qualifier: relationship.qualifier.clone(),
                        origin: evidence.origin.clone(),
                        evidence,
                        supports: vec![SupportRecord {
                            source_id: source.info.source_id.clone(),
                            source_version_id: source
                                .info
                                .pending_version_id
                                .clone()
                                .unwrap_or_else(|| source.info.sha256.clone()),
                            value: relationship.kind.clone(),
                            qualifier: relationship.qualifier.clone(),
                            origin: relationship.evidence.origin.clone(),
                            evidence: validate_evidence(source_text, &relationship.evidence)?,
                        }],
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let mut tag_records = Vec::<TagRecord>::new();
            {
                let mut add_tag = |label: &str, evidence: EvidenceLocation| -> Result<()> {
                    let normalized = normalize_tag(label)?;
                    let support = TagSupport {
                        source_id: source.info.source_id.clone(),
                        source_version_id: source
                            .info
                            .pending_version_id
                            .clone()
                            .unwrap_or_else(|| source.info.sha256.clone()),
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
                let basis = match fact.evidence.offset_basis {
                    EvidenceOffsetBasis::PreservedText => "preserved source text",
                    EvidenceOffsetBasis::ExtractedOfficeProjection => {
                        "extracted Office projection; offsets are not original package byte offsets"
                    }
                    EvidenceOffsetBasis::WebVisibleText => {
                        "extracted web visible-text projection; offsets are not downloaded HTML byte offsets"
                    }
                };
                let locator = fact
                    .evidence
                    .source_location
                    .as_deref()
                    .map(|value| format!(" · original locator: {value}"))
                    .unwrap_or_default();
                let (line_basis, byte_basis) = match fact.evidence.offset_basis {
                    EvidenceOffsetBasis::PreservedText => ("source lines", "source bytes"),
                    EvidenceOffsetBasis::ExtractedOfficeProjection => {
                        ("extracted projection lines", "extracted projection bytes")
                    }
                    EvidenceOffsetBasis::WebVisibleText => (
                        "visible-text projection lines",
                        "visible-text projection bytes",
                    ),
                };
                let guidance = if fact.evidence.offset_basis != EvidenceOffsetBasis::PreservedText {
                    "The retained original opens as a fallback; the offsets above refer to the stated extracted projection."
                } else {
                    "Original opens at the beginning; use these source lines and byte offsets to locate this passage."
                };
                body.push_str(&format!(
                    "\n- **{}:** {}{}\n  - Fact identity: `{}`\n  - Evidence: “{}”\n  - Origin: {} · {} {}–{}, {} {}–{} · {}{}\n  - {}\n  - Links: [Source page](../sources/{}/index.md) · [Retained original](../sources/{}/{})\n",
                    escape_markdown(&fact.property.replace('_', " ")), escape_markdown(&fact.value), fact.qualifier.as_deref().map(|q| format!(" ({})", escape_markdown(q))).unwrap_or_default(), fact.fact_id,
                    escape_markdown(&fact.evidence.quote), escape_markdown(&fact.origin), line_basis, fact.evidence.line_start, fact.evidence.line_end,
                    byte_basis, fact.evidence.byte_start, fact.evidence.byte_end, basis, locator, guidance, &source.info.source_id["source-".len()..],
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
                let (line_basis, byte_basis, locator, guidance) = match relationship.evidence.offset_basis {
                    EvidenceOffsetBasis::PreservedText => ("source lines", "source bytes", String::new(), "Original opens at the beginning; use these source lines and byte offsets to locate this passage."),
                    EvidenceOffsetBasis::ExtractedOfficeProjection => (
                        "extracted projection lines",
                        "extracted projection bytes",
                        relationship.evidence.source_location.as_deref().map(|location| format!(" · original locator: {location}")).unwrap_or_default(),
                        "The original opens as a fallback; use the OOXML part and location above to find the extracted passage.",
                    ),
                    EvidenceOffsetBasis::WebVisibleText => (
                        "visible-text projection lines",
                        "visible-text projection bytes",
                        relationship.evidence.source_location.as_deref().map(|location| format!(" · original locator: {location}")).unwrap_or_default(),
                        "The retained original opens as a fallback; offsets refer to the stated web visible-text projection.",
                    ),
                };
                body.push_str(&format!(
                    "\n- [{}](../pages/{}.md) — **{} →** — [{}](../pages/{}.md){}\n  - Relationship identity: `{}`\n  - Evidence: “{}”\n  - Origin: {} · {} {}–{}, {} {}–{}{}\n  - {}\n  - Links: [Source page](../sources/{}/index.md) · [Retained original](../sources/{}/{})\n",
                    escape_markdown(&from.0.label), from.1.page_id, escape_markdown(&relationship.kind.replace('_', " ")), escape_markdown(&to.0.label), to.1.page_id,
                    relationship.qualifier.as_deref().map(|q| format!(" (qualifier: {})", escape_markdown(q))).unwrap_or_default(), relationship.relationship_id,
                    escape_markdown(&relationship.evidence.quote), escape_markdown(&relationship.origin), line_basis, relationship.evidence.line_start, relationship.evidence.line_end,
                    byte_basis, relationship.evidence.byte_start, relationship.evidence.byte_end, locator, guidance, &source.info.source_id["source-".len()..],
                    &source.info.source_id["source-".len()..], source.info.asset
                ));
            }
            body.push_str("\n## Tags\n");
            if tag_records.is_empty() {
                body.push_str("\nNo supported tags were selected for this page.\n");
            }
            for tag in &tag_records {
                let support = &tag.supports[0];
                let (basis, line_label, byte_label, guidance) =
                    evidence_display_basis(support.evidence.offset_basis);
                let locator = support
                    .evidence
                    .source_location
                    .as_deref()
                    .map(|value| format!(" · source locator: {value}"))
                    .unwrap_or_default();
                body.push_str(&format!(
                    "\n- **#{}** · {}\n  - Evidence: “{}” · {} {}–{}, {} {}–{} · {}{}\n  - {}\n",
                    escape_markdown(&tag.label),
                    escape_markdown(&tag.normalized),
                    escape_markdown(&support.evidence.quote),
                    line_label,
                    support.evidence.line_start,
                    support.evidence.line_end,
                    byte_label,
                    support.evidence.byte_start,
                    support.evidence.byte_end,
                    basis,
                    locator,
                    guidance,
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
        for page in &prior_pages {
            if contents.contains_key(&page.page_id) {
                continue;
            }
            let path = pages_dir.join(format!("{}.md", page.page_id));
            if !path.exists() {
                continue;
            }
            let mut old = read_knowledge_header(&path)?;
            ensure_legacy_supports(&mut old);
            let replaced_sources = self.replacement_sources(&old, source)?;
            mark_arrival_fallback(source, &replaced_sources);
            old.facts = self.reconcile_facts(old.facts, Vec::new(), &replaced_sources)?;
            old.relationships =
                self.reconcile_relationships(old.relationships, Vec::new(), &replaced_sources)?;
            old.tags = self.reconcile_tags(old.tags, Vec::new(), &replaced_sources);
            contents.insert(
                page.page_id.clone(),
                self.render_knowledge_page(old, &std::collections::HashMap::new())?,
            );
        }
        let draft_labels = page_records
            .iter()
            .map(|(_, page, _)| (page.page_id.clone(), page.title.clone()))
            .collect::<std::collections::HashMap<_, _>>();
        for (page_id, markdown) in &mut contents {
            let path = pages_dir.join(format!("{page_id}.md"));
            let mut header = parse_knowledge_header(markdown)?;
            if path.exists() {
                let mut old = read_knowledge_header(&path)?;
                ensure_legacy_supports(&mut old);
                let replaced_sources = self.replacement_sources(&old, source)?;
                mark_arrival_fallback(source, &replaced_sources);
                header.facts = self.reconcile_facts(old.facts, header.facts, &replaced_sources)?;
                header.relationships = self.reconcile_relationships(
                    old.relationships,
                    header.relationships,
                    &replaced_sources,
                )?;
                header.tags = self.reconcile_tags(old.tags, header.tags, &replaced_sources);
            }
            *markdown = self.render_knowledge_page(header, &draft_labels)?;
        }
        for (page_id, markdown) in contents {
            self.staged_publication.push((
                pages_dir.join(format!("{page_id}.md")),
                markdown.into_bytes(),
            ));
        }
        for tag in &draft.tags {
            let _ = page_for(&tag.subject)?;
            validate_tag_label(&tag.label)?;
            validate_evidence(source_text, &tag.evidence)?;
        }
        source.info.knowledge_pages = page_records
            .iter()
            .map(|(_, summary, _)| summary.clone())
            .collect();
        source.knowledge_pages = source.info.knowledge_pages.clone();
        source.info.semantic_decisions = draft.decisions;
        if let Some(version_id) = source.info.pending_version_id.take() {
            source.info.current_version_id = Some(version_id.clone());
            let coverage_complete = source
                .info
                .extraction_coverage
                .as_ref()
                .is_none_or(|parts| {
                    parts
                        .iter()
                        .all(|part| part.status == CoverageStatus::Complete)
                });
            if let Some(version) = source
                .info
                .versions_seen
                .iter_mut()
                .find(|version| version.source_version_id == version_id)
            {
                version.state = "complete".into();
                version.coverage = if coverage_complete {
                    "complete"
                } else {
                    "partial"
                }
                .into();
            }
        }
        source.info.update_status = None;
        if source.info.extraction_coverage.is_none() {
            source.info.extraction_detail = format!(
                "Semantic facts and relationships are current from source version {}.",
                source
                    .info
                    .current_version_id
                    .as_deref()
                    .unwrap_or("unknown")
            );
        }
        source.info.semantic_state = "complete".into();
        source.body = replace_semantic_section(&source.body, &source.info.knowledge_pages);
        source.markdown = serialize_page(&source.info, &source.body)?;
        Ok(())
    }

    fn apply_replacement(
        &mut self,
        source: &mut SourcePage,
        source_version_id: &str,
        source_text: &str,
        draft: KnowledgeDraft,
    ) -> Result<()> {
        let Some(candidate) = source
            .info
            .versions_seen
            .iter()
            .find(|version| version.source_version_id == source_version_id)
            .cloned()
        else {
            return Err(GardenError::Invalid(
                "The pending source version is missing.".into(),
            ));
        };
        let same_entity = draft.entities.iter().any(|entity| {
            source.info.knowledge_pages.iter().any(|page| {
                page.kind.eq_ignore_ascii_case(&entity.kind) && page.title == entity.label
            })
        });
        let complete_replacement = draft.source_update.as_ref().is_some_and(|decision| {
            decision.role == SourceUpdateRole::CompleteReplacement
                && decision.certainty >= 0.8
                && decision.certainty.is_finite()
        });
        if !complete_replacement || !same_entity {
            return self.retain_non_authoritative_update(
                source,
                source_version_id,
                draft.source_update.as_ref(),
            );
        }
        let current_id = source.info.current_version_id.clone().ok_or_else(|| {
            GardenError::Invalid("A replacement has no established current version.".into())
        })?;
        let current_version = source
            .info
            .versions_seen
            .iter()
            .find(|version| version.source_version_id == current_id)
            .cloned()
            .ok_or_else(|| GardenError::Invalid("The current source version is missing.".into()))?;
        match compare_source_order(&current_version, &candidate) {
            Some(std::cmp::Ordering::Less) => {
                if let Some(version) = source
                    .info
                    .versions_seen
                    .iter_mut()
                    .find(|version| version.source_version_id == source_version_id)
                {
                    version.state = "superseded_before_publish".into();
                    version.coverage = "complete".into();
                }
                source.info.pending_version_id = None;
                source.info.update_status = None;
                source.info.semantic_error = None;
                source.info.semantic_retry_at = None;
                return Ok(());
            }
            Some(std::cmp::Ordering::Greater) => {}
            Some(std::cmp::Ordering::Equal) | None => {
                source.info.ordering_uncertain = true;
                if let Some(version) = source
                    .info
                    .versions_seen
                    .iter_mut()
                    .find(|version| version.source_version_id == source_version_id)
                {
                    version.order_basis = "arrival_fallback".into();
                }
            }
        }

        let current_asset = source.info.asset.clone();
        let current_original = self
            .source_dir(&source.info.source_id)?
            .join(&current_asset);
        let previous_original = self
            .source_dir(&source.info.source_id)?
            .join("versions")
            .join(&current_id)
            .join(&current_asset);
        if !previous_original.exists() {
            self.staged_publication
                .push((previous_original, fs::read(&current_original)?));
        }
        let candidate_path = self
            .source_dir(&source.info.source_id)?
            .join("versions")
            .join(source_version_id)
            .join(&candidate.asset);
        let candidate_bytes = fs::read(candidate_path)?;
        let destination_original = self
            .source_dir(&source.info.source_id)?
            .join(&candidate.asset);
        self.staged_publication
            .push((destination_original, candidate_bytes));

        source.info.original_name = candidate.original_name.clone();
        source.info.asset = candidate.asset.clone();
        source.info.sha256 = candidate.sha256.clone();
        source.info.bytes = candidate.bytes;
        source.info.format = candidate.format.clone();
        source.info.extraction = ExtractionState::TextPreserved;
        source.info.extraction_coverage = None;
        source.info.line_count = source_text.lines().count();
        source.info.extraction_detail =
            "Source text preserved. Semantic fact extraction has not run.".into();
        source.info.current_version_id = Some(source_version_id.into());
        source.info.pending_version_id = None;
        source.info.update_status = None;
        source.info.semantic_error = None;
        source.info.semantic_retry_at = None;
        if let Some(previous) = source
            .info
            .versions_seen
            .iter_mut()
            .find(|version| version.source_version_id == current_id)
        {
            previous.state = "superseded".into();
        }
        if let Some(version) = source
            .info
            .versions_seen
            .iter_mut()
            .find(|version| version.source_version_id == source_version_id)
        {
            version.state = "complete".into();
            version.coverage = "complete".into();
        }
        source.body = source_body(&source.info, source_text, None);
        self.publish_knowledge(source, source_text, draft)
    }

    fn reconcile_facts(
        &self,
        previous: Vec<FactRecord>,
        incoming: Vec<FactRecord>,
        remove_source_ids: &std::collections::HashSet<String>,
    ) -> Result<Vec<FactRecord>> {
        let replacement_values = incoming
            .iter()
            .map(|record| (record.fact_id.clone(), record.value.clone()))
            .collect::<std::collections::HashMap<_, _>>();
        let mut records = std::collections::HashMap::<String, FactRecord>::new();
        for (mut record, is_previous) in previous
            .into_iter()
            .map(|record| (record, true))
            .chain(incoming.into_iter().map(|record| (record, false)))
        {
            if record.supports.is_empty() {
                record.supports.push(SupportRecord {
                    source_id: String::new(),
                    source_version_id: "legacy".into(),
                    value: record.value.clone(),
                    qualifier: record.qualifier.clone(),
                    origin: record.origin.clone(),
                    evidence: record.evidence.clone(),
                });
            }
            if is_previous {
                if let Some(replacement_value) = replacement_values.get(&record.fact_id) {
                    record.supports.retain(|support| {
                        !remove_source_ids.contains(&support.source_id)
                            && (remove_source_ids.is_empty() || support.value == *replacement_value)
                    });
                } else {
                    record
                        .supports
                        .retain(|support| !remove_source_ids.contains(&support.source_id));
                }
            }
            if record.supports.is_empty() {
                continue;
            }
            let mut merged = records
                .remove(&record.fact_id)
                .unwrap_or_else(|| record.clone());
            for support in record.supports {
                if !merged.supports.iter().any(|existing| {
                    existing.source_id == support.source_id
                        && existing.source_version_id == support.source_version_id
                        && existing.value == support.value
                        && existing.evidence.byte_start == support.evidence.byte_start
                        && existing.evidence.byte_end == support.evidence.byte_end
                }) {
                    merged.supports.push(support);
                }
            }
            if let Some(current) = self.latest_support(&merged.supports)? {
                merged.value = current.value.clone();
                merged.qualifier = current.qualifier.clone();
                merged.origin = current.origin.clone();
                merged.evidence = current.evidence.clone();
            }
            records.insert(merged.fact_id.clone(), merged);
        }
        Ok(records.into_values().collect())
    }

    fn replacement_sources(
        &self,
        page: &KnowledgePageHeader,
        source: &SourcePage,
    ) -> Result<std::collections::HashSet<String>> {
        let Some(version_id) = source
            .info
            .pending_version_id
            .as_deref()
            .or(source.info.current_version_id.as_deref())
        else {
            return Ok(std::collections::HashSet::new());
        };
        let Some(incoming) = source
            .info
            .versions_seen
            .iter()
            .find(|version| version.source_version_id == version_id)
        else {
            return Ok(std::collections::HashSet::new());
        };
        if incoming.update_role != "complete_replacement"
            || incoming.update_confidence.is_none_or(|value| value < 0.8)
            || incoming.update_evidence.is_none()
        {
            return Ok(std::collections::HashSet::new());
        }

        let support_sources = page
            .facts
            .iter()
            .flat_map(|fact| fact.supports.iter())
            .chain(
                page.relationships
                    .iter()
                    .flat_map(|rel| rel.supports.iter()),
            )
            .map(|support| support.source_id.clone())
            .chain(
                page.tags
                    .iter()
                    .flat_map(|tag| tag.supports.iter())
                    .map(|support| support.source_id.clone()),
            )
            .filter(|source_id| !source_id.is_empty() && source_id != &source.info.source_id)
            .collect::<std::collections::HashSet<_>>();
        let mut replacements = std::collections::HashSet::from([source.info.source_id.clone()]);
        let mut candidates = Vec::new();
        for source_id in support_sources.iter().cloned() {
            let path = self.source_dir(&source_id)?.join("index.md");
            if !path.exists() {
                continue;
            }
            let info = read_page(&path)?.info;
            let Some(previous) = info.versions_seen.iter().find(|version| {
                Some(version.source_version_id.as_str()) == info.current_version_id.as_deref()
            }) else {
                continue;
            };
            // A matching revision number does not make a known supplement or
            // tentative update part of the report being replaced.
            if matches!(
                previous.update_role.as_str(),
                "supplement" | "conditional" | "targeted_correction"
            ) {
                continue;
            }
            if incoming.source_revision.is_some()
                && previous.source_revision.is_some_and(|revision| {
                    revision.saturating_add(1) == incoming.source_revision.unwrap()
                })
            {
                candidates.push((source_id, previous.clone()));
            }
        }
        if candidates.len() == 1 {
            replacements.extend(candidates.into_iter().map(|(source_id, _)| source_id));
            return Ok(replacements);
        }

        if incoming.source_date.is_none() && incoming.source_revision.is_none() {
            let mut undated = Vec::new();
            for source_id in support_sources.iter().cloned() {
                let path = self.source_dir(&source_id)?.join("index.md");
                if !path.exists() {
                    continue;
                }
                let info = read_page(&path)?.info;
                let Some(previous) = info.versions_seen.iter().find(|version| {
                    Some(version.source_version_id.as_str()) == info.current_version_id.as_deref()
                }) else {
                    continue;
                };
                if matches!(
                    previous.update_role.as_str(),
                    "supplement" | "conditional" | "targeted_correction"
                ) {
                    continue;
                }
                if previous.source_date.is_none() && previous.source_revision.is_none() {
                    undated.push((source_id, previous.received_at.clone()));
                }
            }
            undated.sort_by(|a, b| a.1.cmp(&b.1));
            if undated.len() == 1 {
                let (source_id, _) = undated.pop().expect("exactly one candidate");
                replacements.insert(source_id);
            }
            return Ok(replacements);
        }
        Ok(replacements)
    }

    fn reconcile_relationships(
        &self,
        previous: Vec<RelationshipRecord>,
        incoming: Vec<RelationshipRecord>,
        remove_source_ids: &std::collections::HashSet<String>,
    ) -> Result<Vec<RelationshipRecord>> {
        let mut records = std::collections::HashMap::<String, RelationshipRecord>::new();
        for (mut record, is_previous) in previous
            .into_iter()
            .map(|record| (record, true))
            .chain(incoming.into_iter().map(|record| (record, false)))
        {
            if record.supports.is_empty() {
                record.supports.push(SupportRecord {
                    source_id: String::new(),
                    source_version_id: "legacy".into(),
                    value: record.kind.clone(),
                    qualifier: record.qualifier.clone(),
                    origin: record.origin.clone(),
                    evidence: record.evidence.clone(),
                });
            }
            if is_previous {
                record
                    .supports
                    .retain(|support| !remove_source_ids.contains(&support.source_id));
            }
            if record.supports.is_empty() {
                continue;
            }
            let mut merged = records
                .remove(&record.relationship_id)
                .unwrap_or_else(|| record.clone());
            for support in record.supports {
                if !merged.supports.iter().any(|existing| {
                    existing.source_id == support.source_id
                        && existing.source_version_id == support.source_version_id
                        && existing.value == support.value
                        && existing.evidence.byte_start == support.evidence.byte_start
                        && existing.evidence.byte_end == support.evidence.byte_end
                }) {
                    merged.supports.push(support);
                }
            }
            if let Some(current) = self.latest_support(&merged.supports)? {
                merged.kind = current.value.clone();
                merged.qualifier = current.qualifier.clone();
                merged.origin = current.origin.clone();
                merged.evidence = current.evidence.clone();
            }
            records.insert(merged.relationship_id.clone(), merged);
        }
        Ok(records.into_values().collect())
    }

    fn reconcile_tags(
        &self,
        previous: Vec<TagRecord>,
        incoming: Vec<TagRecord>,
        remove_source_ids: &std::collections::HashSet<String>,
    ) -> Vec<TagRecord> {
        let mut records = std::collections::HashMap::<String, TagRecord>::new();
        for (mut record, is_previous) in previous
            .into_iter()
            .map(|record| (record, true))
            .chain(incoming.into_iter().map(|record| (record, false)))
        {
            if is_previous {
                record
                    .supports
                    .retain(|support| !remove_source_ids.contains(&support.source_id));
            }
            if record.supports.is_empty() {
                continue;
            }
            let mut merged = records
                .remove(&record.normalized)
                .unwrap_or_else(|| record.clone());
            for support in record.supports {
                if !merged.supports.iter().any(|existing| {
                    existing.source_id == support.source_id
                        && existing.source_version_id == support.source_version_id
                        && existing.evidence.byte_start == support.evidence.byte_start
                        && existing.evidence.byte_end == support.evidence.byte_end
                }) {
                    merged.supports.push(support);
                }
            }
            records.insert(merged.normalized.clone(), merged);
        }
        let mut tags = records.into_values().collect::<Vec<_>>();
        tags.sort_by(|left, right| left.normalized.cmp(&right.normalized));
        tags
    }

    fn latest_support<'a>(
        &self,
        supports: &'a [SupportRecord],
    ) -> Result<Option<&'a SupportRecord>> {
        let mut ranked = supports
            .iter()
            .map(|support| {
                Ok((
                    self.support_order(&support.source_id, &support.source_version_id)?,
                    support,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        ranked.sort_by(|(a, _), (b, _)| a.cmp(b));
        Ok(ranked.last().map(|(_, support)| *support))
    }

    fn latest_relationship_support<'a>(
        &self,
        supports: &'a [SupportRecord],
    ) -> Result<Option<&'a SupportRecord>> {
        self.latest_support(supports)
    }

    fn latest_tag_support<'a>(&self, supports: &'a [TagSupport]) -> Result<Option<&'a TagSupport>> {
        let mut ranked = supports
            .iter()
            .map(|support| {
                Ok((
                    self.support_order(&support.source_id, &support.source_version_id)?,
                    support,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        ranked.sort_by(|(a, _), (b, _)| a.cmp(b));
        Ok(ranked.last().map(|(_, support)| *support))
    }

    fn support_order(
        &self,
        source_id: &str,
        source_version_id: &str,
    ) -> Result<(Option<String>, Option<u64>, String, String)> {
        if source_id.is_empty() {
            return Ok((None, None, String::new(), source_version_id.to_owned()));
        }
        let path = self.source_dir(source_id)?.join("index.md");
        if !path.exists() {
            return Ok((None, None, String::new(), source_version_id.to_owned()));
        }
        let info = read_page(&path)?.info;
        let version = info
            .versions_seen
            .iter()
            .find(|version| version.source_version_id == source_version_id);
        Ok((
            version.and_then(|version| version.source_date.clone()),
            version.and_then(|version| version.source_revision),
            version
                .map(|version| version.received_at.clone())
                .unwrap_or_default(),
            source_version_id.to_owned(),
        ))
    }

    fn support_links(&self, support: &SupportRecord) -> Result<(String, String)> {
        let Some(short_id) = support.source_id.strip_prefix("source-") else {
            return Ok(("unavailable".into(), "unavailable".into()));
        };
        let source_dir = self.source_dir(&support.source_id)?;
        let source_page = read_page(&source_dir.join("index.md"))?;
        let version = source_page
            .info
            .versions_seen
            .iter()
            .find(|version| version.source_version_id == support.source_version_id);
        let (asset, current) = if let Some(version) = version {
            (
                version.asset.clone(),
                source_page.info.current_version_id.as_deref()
                    == Some(support.source_version_id.as_str())
                    || (source_page.info.current_version_id.is_none()
                        && source_page.info.pending_version_id.as_deref()
                            == Some(support.source_version_id.as_str())),
            )
        } else if support.source_version_id == source_page.info.sha256 {
            (source_page.info.asset.clone(), true)
        } else {
            return Ok((
                format!("../sources/{short_id}/index.md"),
                format!("../sources/{short_id}/{}", source_page.info.asset),
            ));
        };
        let asset_path = if current {
            format!("../sources/{short_id}/{asset}")
        } else {
            format!(
                "../sources/{short_id}/versions/{}/{asset}",
                support.source_version_id
            )
        };
        Ok((format!("../sources/{short_id}/index.md"), asset_path))
    }

    fn render_knowledge_page(
        &self,
        header: KnowledgePageHeader,
        additional_labels: &std::collections::HashMap<String, String>,
    ) -> Result<String> {
        let pages_dir = self.root.join("pages");
        let mut labels = fs::read_dir(&pages_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "md"))
            .filter_map(|entry| read_knowledge_page(&entry.path()).ok())
            .map(|page| (page.page_id, page.title))
            .collect::<std::collections::HashMap<_, _>>();
        for (page_id, title) in additional_labels {
            labels.insert(page_id.clone(), title.clone());
        }
        let mut body = format!("# {}\n\n## Facts\n", escape_heading(&header.title));
        for fact in &header.facts {
            let support = self.latest_support(&fact.supports)?;
            let (source_link, original_link) = support
                .map(|support| self.support_links(support))
                .transpose()?
                .unwrap_or_else(|| ("unavailable".into(), "unavailable".into()));
            let evidence = support
                .map(|support| &support.evidence)
                .unwrap_or(&fact.evidence);
            let origin = support
                .map(|support| support.origin.as_str())
                .unwrap_or(&fact.origin);
            let locator = evidence
                .source_location
                .as_deref()
                .map(|value| format!(" · source locator: {value}"))
                .unwrap_or_default();
            let (basis, line_label, byte_label, guidance) =
                evidence_display_basis(evidence.offset_basis);
            body.push_str(&format!(
                "\n- **{}:** {}{}\n  - Fact identity: `{}`\n  - Evidence: “{}”\n  - Origin: {} · {} {}–{}, {} {}–{} · {}{}\n  - {}\n  - Source support count: {}\n  - Links: [Source page]({}) · [Retained original]({})\n",
                escape_markdown(&fact.property.replace('_', " ")),
                escape_markdown(&fact.value),
                fact.qualifier.as_deref().map(|q| format!(" ({})", escape_markdown(q))).unwrap_or_default(),
                fact.fact_id,
                escape_markdown(&evidence.quote),
                escape_markdown(origin),
                line_label,
                evidence.line_start,
                evidence.line_end,
                byte_label,
                evidence.byte_start,
                evidence.byte_end,
                basis,
                locator,
                guidance,
                fact.supports.len(),
                source_link,
                original_link,
            ));
        }
        if header.facts.is_empty() {
            body.push_str("\nNo current facts are supported by this page.\n");
        }
        body.push_str("\n## Relationships\n");
        for rel in &header.relationships {
            let support = self.latest_support(&rel.supports)?;
            let (source_link, original_link) = support
                .map(|support| self.support_links(support))
                .transpose()?
                .unwrap_or_else(|| ("unavailable".into(), "unavailable".into()));
            let evidence = support
                .map(|support| &support.evidence)
                .unwrap_or(&rel.evidence);
            let origin = support
                .map(|support| support.origin.as_str())
                .unwrap_or(&rel.origin);
            let locator = evidence
                .source_location
                .as_deref()
                .map(|value| format!(" · source locator: {value}"))
                .unwrap_or_default();
            let (basis, line_label, byte_label, guidance) =
                evidence_display_basis(evidence.offset_basis);
            body.push_str(&format!(
                "\n- [{}](../pages/{}.md) — **{} →** — [{}](../pages/{}.md){}\n  - Relationship identity: `{}`\n  - Evidence: “{}”\n  - Origin: {} · {} {}–{}, {} {}–{} · {}{}\n  - {}\n  - Source support count: {}\n  - Links: [Source page]({}) · [Retained original]({})\n",
                escape_markdown(labels.get(&rel.from_page_id).map(String::as_str).unwrap_or(&rel.from_page_id)), rel.from_page_id,
                escape_markdown(&rel.kind.replace('_', " ")),
                escape_markdown(labels.get(&rel.to_page_id).map(String::as_str).unwrap_or(&rel.to_page_id)), rel.to_page_id,
                rel.qualifier.as_deref().map(|q| format!(" (qualifier: {})", escape_markdown(q))).unwrap_or_default(),
                rel.relationship_id, escape_markdown(&evidence.quote), escape_markdown(origin),
                line_label, evidence.line_start, evidence.line_end, byte_label, evidence.byte_start, evidence.byte_end,
                basis,
                locator,
                guidance,
                rel.supports.len(),
                source_link,
                original_link,
            ));
        }
        if header.relationships.is_empty() {
            body.push_str("\nNo current relationships are supported by this page.\n");
        }
        let yaml = serde_yaml_ng::to_string(&header)?;
        Ok(format!("---\n{yaml}---\n\n{body}"))
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
                        source_version_id: source
                            .info
                            .current_version_id
                            .clone()
                            .or(source.info.pending_version_id.clone())
                            .or_else(|| Some(source.info.sha256.clone())),
                        quote: text[start..end].to_owned(),
                        byte_start: start,
                        byte_end: end,
                        line_start,
                        line_end,
                        offset_basis: preserved_text_basis(),
                        source_location: None,
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
                let location = if let Some(support) = self.latest_support(&fact.supports)? {
                    search_location(
                        &fact.fact_id,
                        &support.source_id,
                        Some(&support.source_version_id),
                        &support.evidence,
                    )
                } else {
                    search_location(&fact.fact_id, &header.source_id, None, &fact.evidence)
                };
                return Ok(Some(location));
            }
        }
        for relationship in &header.relationships {
            if contains_all_terms(
                &format!("{} {}", relationship.kind, relationship.evidence.quote),
                &terms,
            ) {
                let location = if let Some(support) =
                    self.latest_relationship_support(&relationship.supports)?
                {
                    search_location(
                        &relationship.relationship_id,
                        &support.source_id,
                        Some(&support.source_version_id),
                        &support.evidence,
                    )
                } else {
                    search_location(
                        &relationship.relationship_id,
                        &header.source_id,
                        None,
                        &relationship.evidence,
                    )
                };
                return Ok(Some(location));
            }
        }
        for tag in &header.tags {
            if tags.contains(&tag.normalized) || contains_all_terms(&tag.label, &terms) {
                if let Some(support) = self.latest_tag_support(&tag.supports)? {
                    return Ok(Some(search_location(
                        &format!("tag:{}", tag.normalized),
                        &support.source_id,
                        Some(&support.source_version_id),
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
        transaction.execute("DELETE FROM source_origins", [])?;
        transaction.execute("DELETE FROM source_versions", [])?;
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
            for acquisition in &page.info.acquisitions {
                transaction.execute(
                    "INSERT OR REPLACE INTO source_origins(path, source_id) VALUES (?1, ?2)",
                    params![acquisition.path, page.info.source_id],
                )?;
            }
            for version in &page.info.versions_seen {
                transaction.execute(
                    "INSERT OR REPLACE INTO source_versions(source_id, source_version_id, format) VALUES (?1, ?2, ?3)",
                    params![page.info.source_id, version.source_version_id, version.format],
                )?;
            }
            let documents = self.search_documents_for_source(&page.info)?;
            write_search_documents(&transaction, &documents)?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn index_page(&mut self, _info: &SourceInfo) -> Result<()> {
        self.rebuild_index()
    }

    fn source_for_origin(&self, path: &str) -> Result<Option<String>> {
        Ok(self
            .index
            .query_row(
                "SELECT source_id FROM source_origins WHERE path=?1",
                [path],
                |row| row.get(0),
            )
            .optional()?)
    }

    fn source_for_version(&self, version_id: &str, format: &str) -> Result<Option<String>> {
        Ok(self
            .index
            .query_row(
                "SELECT source_id FROM source_versions WHERE source_version_id=?1 AND format=?2 ORDER BY source_id LIMIT 1",
                params![version_id, format],
                |row| row.get(0),
            )
            .optional()?)
    }

    fn index_acquisition(&self, path: &str, source_id: &str) -> Result<()> {
        self.index.execute(
            "INSERT OR REPLACE INTO source_origins(path, source_id) VALUES (?1, ?2)",
            params![path, source_id],
        )?;
        Ok(())
    }

    fn index_version(&self, source_id: &str, version_id: &str, format: &str) -> Result<()> {
        self.index.execute(
            "INSERT OR REPLACE INTO source_versions(source_id, source_version_id, format) VALUES (?1, ?2, ?3)",
            params![source_id, version_id, format],
        )?;
        Ok(())
    }

    fn version_original_path(&self, source_id: &str, version: &SourceVersion) -> Result<PathBuf> {
        Ok(self
            .source_dir(source_id)?
            .join("versions")
            .join(&version.source_version_id)
            .join(&version.asset))
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
            // A reconciled page is linked from every source that supports it.
            // Index the canonical page once, under its stable owning source,
            // while its Markdown already contains the current support set.
            if header.source_id != source_page.info.source_id {
                continue;
            }
            let body = knowledge_search_content(&header);
            knowledge.push((summary.clone(), header, body));
        }
        let source_event_date = knowledge
            .iter()
            .filter(|(_, header, _)| header.kind.eq_ignore_ascii_case("event"))
            .find_map(|(_, header, _)| event_date_from_facts(&header.facts));
        let mut documents = Vec::new();
        let current_version_is_superseded = source_page
            .info
            .current_version_id
            .as_deref()
            .and_then(|version_id| {
                source_page
                    .info
                    .versions_seen
                    .iter()
                    .find(|version| version.source_version_id == version_id)
            })
            .is_some_and(|version| version.state == "superseded");
        let source_text = if current_version_is_superseded {
            String::new()
        } else if source_page.info.extraction == ExtractionState::TextPreserved {
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

fn classify_source(
    format: &str,
    count: u64,
    text_bytes: Vec<u8>,
) -> (ExtractionState, &'static str, String) {
    if !matches!(format, "" | "txt" | "text" | "md" | "markdown") {
        return (
            ExtractionState::Unsupported,
            "This format is not supported by the text importer. The original is retained.",
            String::new(),
        );
    }
    if count > MAX_TEXT_BYTES as u64 {
        return (
            ExtractionState::TooLarge,
            "Text exceeds the 2 MiB preview limit. The complete original is retained.",
            String::new(),
        );
    }
    match String::from_utf8(text_bytes) {
        Err(_) => (
            ExtractionState::InvalidUtf8,
            "The source is not valid UTF-8. The original is retained without lossy decoding.",
            String::new(),
        ),
        Ok(text)
            if text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) =>
        {
            (
                ExtractionState::Unsupported,
                "The source contains binary control characters. The original is retained.",
                String::new(),
            )
        }
        Ok(text) => (
            ExtractionState::TextPreserved,
            "Source text preserved. Semantic fact extraction has not run.",
            text.trim_start_matches('\u{feff}').to_owned(),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn source_version(
    digest: &str,
    bytes: u64,
    original_name: &str,
    asset: &str,
    format: &str,
    received_at: &str,
    state: &str,
) -> SourceVersion {
    SourceVersion {
        source_version_id: digest.to_owned(),
        sha256: digest.to_owned(),
        bytes,
        original_name: original_name.to_owned(),
        asset: asset.to_owned(),
        format: format.to_owned(),
        received_at: received_at.to_owned(),
        source_date: None,
        source_revision: None,
        order_basis: "awaiting_provider_judgment".into(),
        update_role: "unknown".into(),
        update_evidence: None,
        update_confidence: None,
        source_date_evidence: None,
        source_date_confidence: None,
        source_revision_evidence: None,
        source_revision_confidence: None,
        state: state.to_owned(),
        coverage: if state == "complete" {
            "complete"
        } else if state == "unavailable" {
            "incomplete"
        } else {
            "pending"
        }
        .into(),
    }
}

fn compare_source_order(
    current: &SourceVersion,
    incoming: &SourceVersion,
) -> Option<std::cmp::Ordering> {
    if let (Some(current_date), Some(incoming_date)) = (&current.source_date, &incoming.source_date)
    {
        let by_date = incoming_date.cmp(current_date);
        if by_date != std::cmp::Ordering::Equal {
            return Some(by_date);
        }
        if let (Some(current_revision), Some(incoming_revision)) =
            (current.source_revision, incoming.source_revision)
        {
            return Some(incoming_revision.cmp(&current_revision));
        }
        return None;
    }
    if let (Some(current_revision), Some(incoming_revision)) =
        (current.source_revision, incoming.source_revision)
    {
        return Some(incoming_revision.cmp(&current_revision));
    }
    None
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
        ExtractionState::StructuredText => "structured_text",
        ExtractionState::PartialText => "partial_text",
        ExtractionState::InvalidContainer => "invalid_container",
        ExtractionState::Unsupported => "unsupported",
        ExtractionState::InvalidUtf8 => "invalid_utf8",
        ExtractionState::TooLarge => "too_large",
    }
    .to_owned()
}

fn parse_extraction(value: &str) -> Result<ExtractionState> {
    match value {
        "text_preserved" => Ok(ExtractionState::TextPreserved),
        "structured_text" => Ok(ExtractionState::StructuredText),
        "partial_text" => Ok(ExtractionState::PartialText),
        "invalid_container" => Ok(ExtractionState::InvalidContainer),
        "unsupported" => Ok(ExtractionState::Unsupported),
        "invalid_utf8" => Ok(ExtractionState::InvalidUtf8),
        "too_large" => Ok(ExtractionState::TooLarge),
        _ => Err(GardenError::Invalid(
            "Search index has an unknown extraction state.".into(),
        )),
    }
}

fn processing_status(info: &SourceInfo) -> String {
    if !matches!(
        info.extraction,
        ExtractionState::TextPreserved
            | ExtractionState::StructuredText
            | ExtractionState::PartialText
    ) {
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

fn knowledge_search_content(header: &KnowledgePageHeader) -> String {
    let mut fields = vec![header.title.clone(), header.kind.clone()];
    for fact in &header.facts {
        fields.extend([
            fact.property.clone(),
            fact.value.clone(),
            fact.qualifier.clone().unwrap_or_default(),
            fact.evidence.quote.clone(),
        ]);
    }
    for relationship in &header.relationships {
        fields.extend([
            relationship.kind.clone(),
            relationship.qualifier.clone().unwrap_or_default(),
            relationship.evidence.quote.clone(),
        ]);
    }
    for tag in &header.tags {
        if !tag.supports.is_empty() {
            fields.push(tag.label.clone());
        }
    }
    fields.join(" ")
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
    let year = date[0..4].parse::<u32>().unwrap_or(0);
    let leap = year != 0 && year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    };
    if !(1..=12).contains(&month) || day == 0 || day > max_day || year == 0 {
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

fn preserved_text_basis() -> String {
    "preserved_text".into()
}

fn evidence_offset_basis_name(basis: EvidenceOffsetBasis) -> &'static str {
    match basis {
        EvidenceOffsetBasis::PreservedText => "preserved_text",
        EvidenceOffsetBasis::ExtractedOfficeProjection => "extracted_office_projection",
        EvidenceOffsetBasis::WebVisibleText => "web_visible_text",
    }
}

fn search_location(
    record_id: &str,
    source_id: &str,
    source_version_id: Option<&str>,
    evidence: &EvidenceLocation,
) -> SearchMatchLocation {
    SearchMatchLocation {
        record_id: record_id.to_owned(),
        source_id: source_id.to_owned(),
        source_version_id: source_version_id.map(str::to_owned),
        quote: evidence.quote.clone(),
        byte_start: evidence.byte_start,
        byte_end: evidence.byte_end,
        line_start: evidence.line_start,
        line_end: evidence.line_end,
        offset_basis: evidence_offset_basis_name(evidence.offset_basis).into(),
        source_location: evidence.source_location.clone(),
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

fn write_acquisition(destination: &Path, acquisition: &Acquisition) -> Result<()> {
    let contexts = destination.join("acquisitions");
    fs::create_dir_all(&contexts)?;
    let key = format!(
        "{:x}",
        Sha256::digest(format!("{}:{:?}", acquisition.path, acquisition.method).as_bytes())
    );
    let mut record = tempfile::NamedTempFile::new_in(&contexts)?;
    write!(
        record,
        "---\n{}---\n\n# Source acquisition\n\n[Source page](../index.md)\n",
        serde_yaml_ng::to_string(acquisition)?
    )?;
    record.as_file().sync_all()?;
    record
        .persist_noclobber(contexts.join(format!("{key}.md")))
        .map_err(|error| GardenError::Io(error.error))?;
    sync_directory(&contexts)
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
    #[serde(default)]
    offset_basis: EvidenceOffsetBasis,
    #[serde(default)]
    source_location: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum EvidenceOffsetBasis {
    #[default]
    PreservedText,
    ExtractedOfficeProjection,
    WebVisibleText,
}

fn evidence_display_basis(
    basis: EvidenceOffsetBasis,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match basis {
        EvidenceOffsetBasis::PreservedText => (
            "preserved source text",
            "source lines",
            "source bytes",
            "Original opens at the beginning; use the source lines and byte offsets to locate this passage.",
        ),
        EvidenceOffsetBasis::ExtractedOfficeProjection => (
            "extracted Office projection; offsets are not original package byte offsets",
            "extracted projection lines",
            "extracted projection bytes",
            "The retained original opens as a fallback; use the OOXML part and locator above to find this passage.",
        ),
        EvidenceOffsetBasis::WebVisibleText => (
            "extracted web visible-text projection; offsets are not downloaded HTML byte offsets",
            "visible-text projection lines",
            "visible-text projection bytes",
            "The retained original opens as a fallback; offsets refer to the stated web visible-text projection.",
        ),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SupportRecord {
    source_id: String,
    source_version_id: String,
    value: String,
    qualifier: Option<String>,
    origin: String,
    evidence: EvidenceLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FactRecord {
    fact_id: String,
    subject_page_id: String,
    property: String,
    #[serde(default)]
    record_key: Option<String>,
    value: String,
    qualifier: Option<String>,
    origin: String,
    evidence: EvidenceLocation,
    #[serde(default)]
    supports: Vec<SupportRecord>,
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
    #[serde(default)]
    supports: Vec<SupportRecord>,
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

fn mark_web_projection_evidence(draft: &mut KnowledgeDraft) {
    fn mark(evidence: &mut EvidenceDraft) {
        evidence.offset_basis = Some("web_visible_text".into());
        evidence.source_location =
            Some("HTML visible-text projection; no stable DOM locator is available".into());
    }

    if let Some(update) = &mut draft.source_update {
        mark(&mut update.evidence);
        if let Some(evidence) = &mut update.source_date_evidence {
            mark(evidence);
        }
        if let Some(evidence) = &mut update.source_revision_evidence {
            mark(evidence);
        }
    }
    for entity in &mut draft.entities {
        mark(&mut entity.evidence);
    }
    for fact in &mut draft.facts {
        mark(&mut fact.evidence);
    }
    for relationship in &mut draft.relationships {
        mark(&mut relationship.evidence);
    }
    for tag in &mut draft.tags {
        mark(&mut tag.evidence);
    }
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
    let line_start_byte = source[..evidence.byte_start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let line_end_byte = source[evidence.byte_start..]
        .find('\n')
        .map(|index| evidence.byte_start + index)
        .unwrap_or(source.len());
    let evidence_line = &source[line_start_byte..line_end_byte];
    let inferred_location = evidence_line
        .strip_prefix('[')
        .and_then(|line| line.find(']').map(|end| line[..end].to_owned()));
    let source_location = evidence
        .source_location
        .clone()
        .or(inferred_location.clone());
    let offset_basis = match evidence.offset_basis.as_deref() {
        None if inferred_location.as_deref().is_some_and(|locator| {
            locator.starts_with("DOCX ") || locator.starts_with("PPTX ")
        }) =>
        {
            EvidenceOffsetBasis::ExtractedOfficeProjection
        }
        None => EvidenceOffsetBasis::PreservedText,
        Some("preserved_text") => EvidenceOffsetBasis::PreservedText,
        Some("office_projection") | Some("extracted_office_projection") => {
            EvidenceOffsetBasis::ExtractedOfficeProjection
        }
        Some("web_visible_text") => EvidenceOffsetBasis::WebVisibleText,
        Some(_) => {
            return Err(GardenError::Invalid(
                "Semantic evidence uses an unsupported offset basis.".into(),
            ));
        }
    };
    Ok(EvidenceLocation {
        quote: evidence.quote.clone(),
        byte_start: evidence.byte_start,
        byte_end: evidence.byte_end,
        line_start,
        line_end,
        origin: evidence.origin.clone(),
        qualifier: evidence.qualifier.clone(),
        offset_basis,
        source_location,
    })
}

fn stable_page_id(entity: &EntityDraft, source_id: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(entity.kind.to_lowercase().as_bytes());
    digest.update([0]);
    digest.update(entity.label.to_lowercase().as_bytes());
    if matches!(
        entity.kind.to_ascii_lowercase().as_str(),
        "document" | "presentation"
    ) {
        digest.update([0]);
        digest.update(source_id.as_bytes());
    }
    format!("page-{:x}", digest.finalize())
}

fn stable_fact_id(subject_page_id: &str, property: &str, record_key: Option<&str>) -> String {
    let mut digest = Sha256::new();
    digest.update(subject_page_id.as_bytes());
    digest.update([0]);
    digest.update(property.trim().to_lowercase().as_bytes());
    if let Some(record_key) = record_key {
        digest.update([0]);
        digest.update(record_key.trim().to_lowercase().as_bytes());
    }
    format!("fact-{:x}", digest.finalize())
}

fn stable_relationship_id(from_page_id: &str, to_page_id: &str, kind: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(from_page_id.as_bytes());
    digest.update([0]);
    digest.update(kind.trim().to_lowercase().as_bytes());
    digest.update([0]);
    digest.update(to_page_id.as_bytes());
    format!("relationship-{:x}", digest.finalize())
}

fn write_atomic_from_file(path: &Path, source: &Path) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        GardenError::Invalid("Cannot publish a file without a parent directory.".into())
    })?;
    fs::create_dir_all(parent)?;
    let mut input = File::open(source)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    std::io::copy(&mut input, file.as_file_mut())?;
    file.as_file().sync_all()?;
    file.persist(path)
        .map_err(|error| GardenError::Io(error.error))?;
    sync_directory(parent)?;
    Ok(())
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

fn parse_knowledge_header(markdown: &str) -> Result<KnowledgePageHeader> {
    let header = markdown
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n").map(|(yaml, _)| yaml))
        .ok_or_else(|| GardenError::Invalid("Knowledge page metadata is unreadable.".into()))?;
    Ok(serde_yaml_ng::from_str(header)?)
}

fn read_knowledge_header(path: &Path) -> Result<KnowledgePageHeader> {
    let mut markdown = String::new();
    File::open(path)?
        .take(256 * 1024)
        .read_to_string(&mut markdown)?;
    parse_knowledge_header(&markdown)
}

fn ensure_legacy_supports(header: &mut KnowledgePageHeader) {
    for fact in &mut header.facts {
        if fact.supports.is_empty() {
            fact.supports.push(SupportRecord {
                source_id: header.source_id.clone(),
                source_version_id: "legacy".into(),
                value: fact.value.clone(),
                qualifier: fact.qualifier.clone(),
                origin: fact.origin.clone(),
                evidence: fact.evidence.clone(),
            });
        }
    }
    for relationship in &mut header.relationships {
        if relationship.supports.is_empty() {
            relationship.supports.push(SupportRecord {
                source_id: header.source_id.clone(),
                source_version_id: "legacy".into(),
                value: relationship.kind.clone(),
                qualifier: relationship.qualifier.clone(),
                origin: relationship.origin.clone(),
                evidence: relationship.evidence.clone(),
            });
        }
    }
}

fn mark_arrival_fallback(
    source: &mut SourcePage,
    replaced_sources: &std::collections::HashSet<String>,
) {
    if replaced_sources.is_empty() {
        return;
    }
    let Some(version_id) = source
        .info
        .pending_version_id
        .clone()
        .or(source.info.current_version_id.clone())
    else {
        return;
    };
    let Some(version) = source
        .info
        .versions_seen
        .iter_mut()
        .find(|version| version.source_version_id == version_id)
    else {
        return;
    };
    if version.source_date.is_none() && version.source_revision.is_none() {
        source.info.ordering_uncertain = true;
        version.order_basis = "arrival_fallback".into();
    }
}

fn now_millis() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| GardenError::Invalid(error.to_string()))?
        .as_millis())
}

fn source_body(info: &SourceInfo, text: &str, office: Option<&OfficeProjection>) -> String {
    let title = info
        .title
        .replace(['\n', '\r'], " ")
        .replace(['[', ']', '<', '>'], "");
    if let Some(projection) = office {
        return projection.markdown.replace("ORIGINAL_ASSET", &info.asset);
    }
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

fn content_disposition_filename(header: &str) -> Option<String> {
    let value = header.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        key.eq_ignore_ascii_case("filename")
            .then(|| value.trim().trim_matches('"').to_owned())
    })?;
    safe_file_name(&value)
}

fn safe_file_name(value: &str) -> Option<String> {
    let name = value.rsplit(['/', '\\']).next()?.trim();
    if name.is_empty() || name == "." || name == ".." || name.chars().any(char::is_control) {
        return None;
    }
    Some(name.to_owned())
}

fn url_name_and_format(
    final_url: &str,
    content_type: Option<&str>,
    disposition: Option<&str>,
) -> (String, String) {
    let url_name = reqwest::Url::parse(final_url).ok().and_then(|url| {
        url.path_segments()?
            .rfind(|segment| !segment.is_empty())
            .and_then(safe_file_name)
    });
    let mut name = disposition
        .and_then(safe_file_name)
        .or(url_name)
        .unwrap_or_else(|| "download".into());
    let ext = Path::new(&name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime_format = match content_type.unwrap_or("") {
        "text/html" | "application/xhtml+xml" => Some("html"),
        "text/plain" => Some("txt"),
        "text/markdown" => Some("md"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Some("docx"),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => Some("pptx"),
        _ => None,
    };
    let format = mime_format.map(str::to_owned).unwrap_or(ext);
    if Path::new(&name).extension().is_none() && !format.is_empty() {
        name.push('.');
        name.push_str(&format);
    }
    (name, format)
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
    let body = body.trim_start_matches('\n').to_owned();
    let mut info: SourceInfo = serde_yaml_ng::from_str(header)?;
    if info.versions_seen.is_empty() {
        let received_at = info
            .acquisitions
            .first()
            .map(|acquisition| acquisition.received_at.clone())
            .unwrap_or_else(|| "0".into());
        let mut legacy = source_version(
            &info.sha256,
            info.bytes,
            &info.original_name,
            &info.asset,
            &info.format,
            &received_at,
            if info.semantic_state == "complete" {
                "complete"
            } else {
                "pending"
            },
        );
        legacy.coverage = if info.extraction == ExtractionState::TextPreserved
            && info.semantic_state == "complete"
        {
            "complete".into()
        } else {
            "incomplete".into()
        };
        info.current_version_id = (info.semantic_state == "complete").then(|| info.sha256.clone());
        info.pending_version_id = (info.semantic_state != "complete"
            && info.extraction == ExtractionState::TextPreserved)
            .then(|| info.sha256.clone());
        info.versions_seen.push(legacy);
    }
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

#[derive(Serialize, Deserialize)]
struct PublicationManifest {
    entries: Vec<PublicationEntry>,
}

#[derive(Serialize, Deserialize)]
struct PublicationEntry {
    destination: String,
    staged: String,
}

fn commit_publication(
    root: &Path,
    files: Vec<(PathBuf, Vec<u8>)>,
    fail_after_files: Option<usize>,
) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }
    let transactions = root.join(".staging/transactions");
    fs::create_dir_all(&transactions)?;
    let staging = tempfile::tempdir_in(&transactions)?;
    let staged_files = staging.path().join("files");
    fs::create_dir_all(&staged_files)?;
    let mut entries = Vec::with_capacity(files.len());
    for (index, (destination, bytes)) in files.into_iter().enumerate() {
        let relative = destination.strip_prefix(root).map_err(|_| {
            GardenError::Invalid("A publication path escaped the collection.".into())
        })?;
        validate_relative_collection_path(relative)?;
        let staged = format!("files/{index:04}");
        let path = staging.path().join(&staged);
        let mut file = File::create(&path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        entries.push(PublicationEntry {
            destination: relative.to_string_lossy().into_owned(),
            staged,
        });
    }
    sync_directory(&staged_files)?;
    let manifest = serde_json::to_vec(&PublicationManifest { entries })?;
    let manifest_path = staging.path().join("manifest.json");
    let mut file = File::create(&manifest_path)?;
    file.write_all(&manifest)?;
    file.sync_all()?;
    sync_directory(staging.path())?;
    let transaction_path = staging.keep();
    sync_directory(&transactions)?;
    apply_publication(root, &transaction_path, fail_after_files)?;
    fs::remove_dir_all(&transaction_path)?;
    sync_directory(&transactions)
}

fn recover_publications(root: &Path) -> Result<()> {
    let transactions = root.join(".staging/transactions");
    fs::create_dir_all(&transactions)?;
    for entry in fs::read_dir(&transactions)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        if !path.join("manifest.json").exists() {
            fs::remove_dir_all(path)?;
            continue;
        }
        apply_publication(root, &path, None)?;
        fs::remove_dir_all(path)?;
    }
    sync_directory(&transactions)
}

fn apply_publication(
    root: &Path,
    transaction: &Path,
    fail_after_files: Option<usize>,
) -> Result<()> {
    let manifest: PublicationManifest =
        serde_json::from_slice(&fs::read(transaction.join("manifest.json"))?)?;
    for (index, entry) in manifest.entries.into_iter().enumerate() {
        let destination = Path::new(&entry.destination);
        let staged = Path::new(&entry.staged);
        validate_relative_collection_path(destination)?;
        validate_relative_staging_path(staged)?;
        let bytes = fs::read(transaction.join(staged))?;
        write_atomic(&root.join(destination), &bytes)?;
        if fail_after_files == Some(index + 1) {
            return Err(GardenError::Io(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Injected publication interruption after an atomic file replacement.",
            )));
        }
    }
    Ok(())
}

fn validate_relative_collection_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        })
    {
        return Err(GardenError::Invalid(
            "A publication path is outside the collection.".into(),
        ));
    }
    Ok(())
}

fn validate_relative_staging_path(path: &Path) -> Result<()> {
    validate_relative_collection_path(path)?;
    if path.components().next() != Some(std::path::Component::Normal("files".as_ref())) {
        return Err(GardenError::Invalid(
            "A publication staging path is invalid.".into(),
        ));
    }
    Ok(())
}
