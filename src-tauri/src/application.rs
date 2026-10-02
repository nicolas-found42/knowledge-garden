//! Public, disk-backed collection operations. Markdown and originals are authoritative.
use fs2::FileExt;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceList {
    pub sources: Vec<SourceSummary>,
    pub next_offset: Option<usize>,
}

pub struct Application {
    root: PathBuf,
    index: Connection,
    // The lock survives for this application's lifetime, including across commands.
    _lock: File,
}

impl Application {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
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
             );",
        )?;
        let mut app = Self {
            root,
            index,
            _lock: lock,
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
        Ok(page)
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

    pub fn rebuild_index(&mut self) -> Result<()> {
        let transaction = self.index.transaction()?;
        transaction.execute("DELETE FROM sources", [])?;
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
        }
        transaction.commit()?;
        Ok(())
    }

    fn index_page(&self, info: &SourceInfo) -> Result<()> {
        self.index.execute(
            "INSERT OR REPLACE INTO sources (source_id, summary) VALUES (?1, ?2)",
            params![info.source_id, summary_json(info)?],
        )?;
        Ok(())
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
    let info = serde_yaml_ng::from_str(header)?;
    let body = body.trim_start_matches('\n').to_owned();
    Ok(SourcePage {
        info,
        markdown,
        body,
    })
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}
