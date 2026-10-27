//! Ephemeral comparisons of current acquired material, without revision playback.
use super::{
    read_bounded_text, Application, ExtractionState, GardenError, Result, MAX_PAGE_BYTES,
    MAX_TEXT_BYTES,
};
use crate::office::CoveragePart;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonRequest {
    pub source_id: String,
    #[serde(default = "first_line")]
    pub line_start: usize,
    #[serde(default)]
    pub expected_signature: Option<String>,
}
fn first_line() -> usize {
    1
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonStatus {
    Fresh,
    Stale,
    Unavailable,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonPassage {
    pub text: String,
    pub line_start: usize,
    pub line_end: usize,
    pub byte_start: usize,
    pub byte_end: usize,
    pub offset_basis: String,
    pub next_line: Option<usize>,
    pub truncated: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonSide {
    pub source_id: String,
    pub title: String,
    pub format: String,
    pub original_name: String,
    pub origin: Option<String>,
    pub source_version_id: Option<String>,
    pub asset: Option<String>,
    pub signature: Option<String>,
    pub status: ComparisonStatus,
    pub detail: String,
    pub processing_status: String,
    pub coverage: Vec<CoveragePart>,
    pub context_gaps: Vec<String>,
    pub passage: Option<ComparisonPassage>,
}
impl Application {
    pub fn comparison_original_path(
        &self,
        request: ComparisonRequest,
    ) -> Result<std::path::PathBuf> {
        let side = self.comparison_side(request)?;
        if side.status == ComparisonStatus::Stale || side.asset.is_none() {
            return Err(GardenError::Invalid(side.detail));
        }
        if let Some(version) = side.source_version_id {
            self.original_version_path(&side.source_id, &version, side.asset.as_deref().unwrap())
        } else {
            self.original_path(&side.source_id)
        }
    }

    pub fn comparison_side(&self, request: ComparisonRequest) -> Result<ComparisonSide> {
        self.source_dir(&request.source_id)?;
        if request.line_start == 0
            || request
                .expected_signature
                .as_ref()
                .is_some_and(|s| s.len() > 128)
        {
            return Err(GardenError::Invalid(
                "Invalid comparison passage selection.".into(),
            ));
        }
        let mut side = ComparisonSide {
            source_id: request.source_id.clone(),
            title: "Unavailable source".into(),
            format: String::new(),
            original_name: String::new(),
            origin: None,
            source_version_id: None,
            asset: None,
            signature: None,
            status: ComparisonStatus::Unavailable,
            detail: "This source page is unavailable; no replacement passage has been generated."
                .into(),
            processing_status: "unavailable".into(),
            coverage: Vec::new(),
            context_gaps: Vec::new(),
            passage: None,
        };
        let page = match self.open_source(&request.source_id) {
            Ok(page) => page,
            Err(_) => return Ok(side),
        };
        side.title = page.info.title.clone();
        side.format = page.info.format.clone();
        side.original_name = page.info.original_name.clone();
        side.origin = page
            .info
            .acquisitions
            .first()
            .map(|context| context.path.clone());
        side.coverage = page.info.extraction_coverage.clone().unwrap_or_default();
        side.processing_status = super::processing_status(&page.info);
        let version = page
            .info
            .current_version_id
            .as_ref()
            .unwrap_or(&page.info.sha256);
        if page
            .info
            .versions_seen
            .iter()
            .any(|v| &v.source_version_id == version && v.state == "superseded")
        {
            side.detail="This source version is superseded. Select current material from search or the source page.".into();
            return Ok(side);
        }
        let original = match self.original_path(&request.source_id) {
            Ok(path) => path,
            Err(_) => {
                side.detail="The retained original is unavailable. Its missing passage has not been replaced with generated text.".into();
                return Ok(side);
            }
        };
        side.source_version_id = page
            .info
            .versions_seen
            .iter()
            .any(|known| &known.source_version_id == version)
            .then(|| version.clone());
        side.asset = Some(page.info.asset.clone());
        let timed_without_evidence = super::is_timed_media_format(&page.info.format)
            && page
                .info
                .audio_processing
                .as_ref()
                .is_none_or(|media| !media.has_evidence());
        let (text, basis) = match page.info.extraction {
            ExtractionState::TextPreserved => {
                match read_bounded_text(&original, MAX_TEXT_BYTES as u64) {
                    Ok(text) => (text, "retained_original_text"),
                    Err(_) => {
                        side.detail="A bounded UTF-8 passage could not be read from this original. Open the retained original or source context.".into();
                        return Ok(side);
                    }
                }
            }
            ExtractionState::StructuredText | ExtractionState::PartialText
                if !timed_without_evidence =>
            {
                (page.body.clone(), "readable_source_projection")
            }
            _ => {
                side.detail="No extracted textual passage is available. Open the retained original or source preview; extraction qualifications remain visible in the source context.".into();
                return Ok(side);
            }
        };
        let mut digest = Sha256::new();
        for value in [version.as_str(), &page.info.title, &text] {
            digest.update((value.len() as u64).to_be_bytes());
            digest.update(value.as_bytes());
        }
        digest.update(serde_json::to_vec(&side.coverage)?);
        // Compare current linked knowledge too: a manual correction can change its
        // interpretation without changing the acquired source's bytes or version.
        for summary in &page.info.knowledge_pages {
            digest.update(summary.page_id.as_bytes());
            match self
                .knowledge_path(&summary.page_id)
                .and_then(|path| read_bounded_text(&path, MAX_PAGE_BYTES))
            {
                Ok(content) => {
                    digest.update([1]);
                    digest.update((content.len() as u64).to_be_bytes());
                    digest.update(content.as_bytes());
                }
                Err(_) => {
                    digest.update([0]);
                    side.context_gaps.push(format!(
                        "Current knowledge context unavailable: {}",
                        summary.title
                    ));
                }
            }
        }
        let signature = hex::encode(digest.finalize());
        side.signature = Some(signature.clone());
        if request
            .expected_signature
            .as_ref()
            .is_some_and(|expected| expected != &signature)
        {
            side.status = ComparisonStatus::Stale;
            side.detail="This source or its current knowledge changed. Refresh this selection to compare current material.".into();
            return Ok(side);
        }
        side.passage = passage(&text, request.line_start, basis);
        if side.passage.is_none() {
            side.detail="The selected passage is missing or empty. Choose an available source line or open the retained original.".into();
            return Ok(side);
        }
        side.status = ComparisonStatus::Fresh;
        side.detail=if basis=="retained_original_text" {"Retained source text with its original qualifications. Open the original for surrounding context."}else{"Readable extracted source projection; its line and byte locations refer to this projection. Use source context for media previews and precise evidence locators; machine interpretations retain their qualifications."}.into();
        Ok(side)
    }
}
fn passage(text: &str, first: usize, basis: &str) -> Option<ComparisonPassage> {
    const MAX_BYTES: usize = 8192;
    const MAX_LINES: usize = 40;
    let total = text.split_inclusive('\n').count();
    let mut start = 0;
    let mut bytes = 0;
    let mut rows = 0;
    let mut truncated = false;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        if index + 1 < first {
            start += line.len();
            continue;
        }
        if rows == MAX_LINES || bytes == MAX_BYTES {
            truncated = true;
            break;
        }
        let mut take = line.len().min(MAX_BYTES - bytes);
        while !line.is_char_boundary(take) {
            take -= 1
        }
        if take == 0 {
            truncated = true;
            break;
        }
        bytes += take;
        rows += 1;
        if take < line.len() {
            truncated = true;
            break;
        }
    }
    if bytes == 0 {
        return None;
    }
    let last = first + rows - 1;
    Some(ComparisonPassage {
        text: text.get(start..start + bytes)?.to_owned(),
        line_start: first,
        line_end: last,
        byte_start: start,
        byte_end: start + bytes,
        offset_basis: basis.into(),
        next_line: (last < total).then_some(last + 1),
        truncated,
    })
}
