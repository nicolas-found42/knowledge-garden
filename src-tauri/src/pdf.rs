//! Local PDF page projection through the common located extraction boundary.
use crate::office::{CoveragePart, CoverageScope, CoverageStatus, OfficeProjection};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
struct Document {
    pages: Vec<Page>,
}
#[derive(Deserialize)]
struct Page {
    number: usize,
    #[serde(default)]
    rotation: i64,
    rows: Vec<Row>,
    gaps: Vec<String>,
}
#[derive(Deserialize)]
struct Row {
    text: String,
    channel: String,
    #[serde(rename = "box")]
    bounds: [f64; 4],
    #[serde(default = "first_column")]
    column: usize,
    confidence: Option<f64>,
    revision: Option<u64>,
}

fn first_column() -> usize {
    1
}

fn line(text: &str) -> String {
    text.replace(['\n', '\r', '\0'], " ")
}

pub fn extract(path: &Path, title: &str) -> Result<OfficeProjection, String> {
    let document: Document =
        serde_json::from_slice(&run_helper(path)?).map_err(|error| error.to_string())?;
    let mut sections = Vec::new();
    let mut semantic = Vec::new();
    let mut coverage = Vec::new();
    for page in document.pages {
        let mut rows = Vec::new();
        for (index, row) in page.rows.iter().enumerate() {
            let located = format!("[PDF page {} region {}; PDF points bottom-left x={:.3},y={:.3},width={:.3},height={:.3}; rotation={}; column={}; channel={}] {}", page.number, index + 1, row.bounds[0], row.bounds[1], row.bounds[2], row.bounds[3], page.rotation, row.column, row.channel, line(&row.text));
            let located = if row.channel == "ocr" {
                format!("{located} (OCR transcription; unverified; confidence {:.4}; Vision revision {})", row.confidence.unwrap_or(0.0), row.revision.unwrap_or(0))
            } else {
                located
            };
            rows.push(located.clone());
            semantic.push(located);
        }
        rows.extend(
            page.gaps
                .iter()
                .map(|gap| format!("Extraction gap on PDF page {}: {}", page.number, line(gap))),
        );
        sections.push(format!(
            "## PDF page {}\n\n{}",
            page.number,
            rows.join("\n\n")
        ));
        coverage.push(CoveragePart {
            scope: CoverageScope::PdfPages,
            status: if page.gaps.is_empty() {
                CoverageStatus::Complete
            } else {
                CoverageStatus::Partial
            },
            source_location: Some(format!("PDF page {}", page.number)),
            detail: if page.gaps.is_empty() {
                format!(
                    "Recovered {} located text regions; selectable text and rendered-page OCR inspected.",
                    page.rows.len()
                )
            } else {
                page.gaps.join("; ")
            },
        });
    }
    let partial = coverage
        .iter()
        .any(|part| part.status != CoverageStatus::Complete);
    let detail = if partial {
        coverage
            .iter()
            .filter(|part| part.status != CoverageStatus::Complete)
            .map(|part| {
                format!(
                    "{}: {}",
                    part.source_location.as_deref().unwrap_or("PDF"),
                    part.detail
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    } else {
        "Native PDFKit/Vision page text and OCR projection; original page locations and explicit missing coverage retained.".into()
    };
    Ok(OfficeProjection {
        markdown: format!("# {}\n\n[Open original](ORIGINAL_ASSET)\n\n{}\n\nPDF locators name the original page and region. The original opens at the beginning as a disclosed fallback; use the page number and PDF-point region above.\n", line(title), sections.join("\n\n")),
        line_count: semantic.len(), semantic_text: semantic.join("\n"), coverage, partial,
        detail,
    })
}

#[cfg(target_os = "macos")]
fn run_helper(path: &Path) -> Result<Vec<u8>, String> {
    use std::{
        io::{Read, Seek, SeekFrom},
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let temporary = tempfile::tempdir().map_err(|error| error.to_string())?;
    let executable = temporary.path().join("pdf-helper");
    std::fs::write(
        &executable,
        include_bytes!(concat!(env!("OUT_DIR"), "/pdf-helper")),
    )
    .map_err(|error| error.to_string())?;
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| error.to_string())?;
    let mut stdout = tempfile::tempfile().map_err(|error| error.to_string())?;
    let mut stderr = tempfile::tempfile().map_err(|error| error.to_string())?;
    let mut child = crate::processes::spawn(
        Command::new(&executable)
            .arg(path)
            .stdout(Stdio::from(
                stdout.try_clone().map_err(|error| error.to_string())?,
            ))
            .stderr(Stdio::from(
                stderr.try_clone().map_err(|error| error.to_string())?,
            )),
    )
    .map_err(|error| error.to_string())?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if started.elapsed() > Duration::from_secs(60) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Native PDF extraction exceeded its 60 second limit; the original remains retained.".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    if !status.success() {
        stderr
            .seek(SeekFrom::Start(0))
            .map_err(|error| error.to_string())?;
        let mut detail = String::new();
        stderr
            .take(8192)
            .read_to_string(&mut detail)
            .map_err(|error| error.to_string())?;
        return Err(format!("Native PDF decoder failed: {detail}"));
    }
    stdout
        .seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    stdout
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("PDF projection exceeds its 8 MiB result bound.".into());
    }
    Ok(bytes)
}

#[cfg(not(target_os = "macos"))]
fn run_helper(_: &Path) -> Result<Vec<u8>, String> {
    Err("Native PDF extraction requires macOS; the inspectable original remains retained.".into())
}

/// A model cannot turn projection offsets into PDF bytes or attach a quote to
/// another page, region, or extraction channel, even if semantic review accepts it.
pub fn validate_draft(
    source: &str,
    format: &str,
    draft: &crate::semantic::KnowledgeDraft,
) -> Result<(), String> {
    let evidence = draft
        .entities
        .iter()
        .map(|item| &item.evidence)
        .chain(draft.facts.iter().map(|item| &item.evidence))
        .chain(draft.relationships.iter().map(|item| &item.evidence))
        .chain(draft.tags.iter().map(|item| &item.evidence))
        .chain(
            draft
                .correction_candidates
                .iter()
                .map(|item| &item.evidence),
        )
        .chain(
            draft
                .correction_alignments
                .iter()
                .map(|item| &item.candidate.evidence),
        )
        .chain(draft.source_update.iter().flat_map(|item| {
            [
                Some(&item.evidence),
                item.source_date_evidence.as_ref(),
                item.source_revision_evidence.as_ref(),
            ]
            .into_iter()
            .flatten()
        }));
    for item in evidence {
        if format != "pdf" {
            if source.starts_with("[PDF ")
                || matches!(item.origin.as_str(), "pdf_text")
                || item.offset_basis.as_deref() == Some("extracted_pdf_projection")
                || item
                    .source_location
                    .as_deref()
                    .is_some_and(|location| location.starts_with("PDF "))
            {
                return Err("PDF evidence requires a retained PDF source version.".into());
            }
            continue;
        }
        let prefix = source
            .get(..item.byte_start)
            .ok_or("PDF evidence offset is outside the projection.")?;
        let start = prefix.rfind('\n').map_or(0, |index| index + 1);
        let line = source[start..].split('\n').next().unwrap_or_default();
        let (locator, _) = line
            .strip_prefix('[')
            .and_then(|line| line.split_once("] "))
            .filter(|(locator, _)| locator.starts_with("PDF page "))
            .ok_or("PDF evidence requires a located native projection region.")?;
        if item.byte_start < start + locator.len() + 3
            || item.byte_end > start + line.len()
            || item
                .source_location
                .as_deref()
                .is_some_and(|location| location != locator)
            || item
                .offset_basis
                .as_deref()
                .is_some_and(|basis| basis != "extracted_pdf_projection")
        {
            return Err("PDF evidence must retain its exact page/region locator and projection offset basis.".into());
        }
        let (_, channel) = locator
            .rsplit_once("; channel=")
            .ok_or("PDF evidence channel is missing.")?;
        if !matches!(channel, "pdf_text" | "ocr") || item.origin != channel {
            return Err("PDF evidence origin disagrees with its native extraction channel.".into());
        }
        if channel == "ocr"
            && item
                .qualifier
                .as_deref()
                .is_none_or(|value| !value.contains("OCR") || !value.contains("unverified"))
        {
            return Err(
                "PDF OCR evidence must preserve its unverified transcription qualification.".into(),
            );
        }
    }
    Ok(())
}
