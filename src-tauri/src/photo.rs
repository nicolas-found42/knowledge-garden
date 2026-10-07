//! Bounded native photo projection. Model observations never authenticate metadata.
use crate::office::{CoveragePart, CoverageScope, CoverageStatus, OfficeProjection};
use serde::Deserialize;
use std::path::Path;

pub fn is_photo(format: &str) -> bool {
    matches!(
        format,
        "jpg" | "jpeg" | "png" | "heic" | "heif" | "tif" | "tiff" | "webp"
    )
}

#[derive(Deserialize)]
struct Metadata {
    key: String,
    value: String,
    channel: String,
}
#[derive(Deserialize)]
struct Ocr {
    text: String,
    confidence: f64,
    #[serde(rename = "box")]
    bounds: [f64; 4],
}
#[derive(Deserialize)]
struct Classification {
    label: String,
    confidence: f64,
}
#[derive(Deserialize)]
struct Photo {
    width: u64,
    height: u64,
    metadata: Vec<Metadata>,
    ocr: Vec<Ocr>,
    classifications: Vec<Classification>,
    gaps: Vec<String>,
    ocr_revision: u64,
    classification_revision: u64,
}

fn line(value: &str) -> String {
    value.replace(['\n', '\r', '\0'], " ")
}

pub fn extract(path: &Path, title: &str) -> Result<OfficeProjection, String> {
    let photo: Photo =
        serde_json::from_slice(&run_helper(path, false)?).map_err(|e| e.to_string())?;
    let mut lines = vec![format!("[PHOTO whole image; channel=observed_pixels] Decoded image dimensions: {} × {} pixels. Whole-image fallback.", photo.width, photo.height)];
    lines.push("[PHOTO metadata; channel=file_metadata] Authenticated capture date and named identities: unknown. File metadata is unauthenticated and may conflict with captions.".into());
    for metadata in &photo.metadata {
        let qualification = if metadata.channel == "supplied_caption" {
            "supplied caption; unverified textual assertion, not observed pixels or authenticated capture date"
        } else {
            "unauthenticated file metadata, not an authenticated capture date"
        };
        lines.push(format!(
            "[PHOTO metadata {}; channel={}] {}: {} ({qualification})",
            line(&metadata.key),
            metadata.channel,
            line(&metadata.key),
            line(&metadata.value)
        ));
    }
    for (index, row) in photo.ocr.iter().enumerate() {
        lines.push(format!("[PHOTO OCR region {}; normalized bottom-left x={:.6},y={:.6},width={:.6},height={:.6}; channel=ocr] {} (OCR transcription; confidence {:.4}; Vision revision {}; text in an image does not verify an event or identity)", index + 1, row.bounds[0], row.bounds[1], row.bounds[2], row.bounds[3], line(&row.text), row.confidence, photo.ocr_revision));
    }
    for prediction in &photo.classifications {
        lines.push(format!("[PHOTO whole image; channel=generated_interpretation] Classifier prediction: {} (uncertain model interpretation; confidence {:.4}; Vision revision {}; whole-image fallback)", line(&prediction.label), prediction.confidence, photo.classification_revision));
    }
    let partial = !photo.gaps.is_empty();
    let semantic_text = lines.join("\n");
    let mut markdown = format!("# {}\n\n[Open original](ORIGINAL_ASSET)\n\n## Image evidence\n\n{}\n\nOCR uses a decoded preview bounded to 2048 pixels. Region coordinates refer to the upright image with a bottom-left origin. Open the on-demand preview or retained original and use the region locator; otherwise the whole image is the disclosed fallback. Named identities remain unknown. No general-purpose provider image capability has been validated; unsupported interpretation remains a visible capability gap.\n", line(title), semantic_text);
    for gap in &photo.gaps {
        markdown.push_str(&format!("\n- Extraction gap: {}\n", line(gap)));
    }
    let coverage = [CoverageScope::ImagePixels, CoverageScope::ImageMetadata, CoverageScope::ImageText, CoverageScope::ImageInterpretation].into_iter().map(|scope| CoveragePart {
        scope, status: if partial { CoverageStatus::Partial } else { CoverageStatus::Complete }, source_location: Some("whole image / normalized OCR regions".into()),
        detail: if partial { photo.gaps.join("; ") } else { "Native decoder, metadata, OCR and uncertain classifier channels inspected; classification does not establish identity or capture date.".into() },
    }).collect();
    Ok(OfficeProjection { markdown, semantic_text, line_count: lines.len(), coverage, partial, detail: "Native ImageIO/Vision photo projection; origins, uncertainties and region locators retained.".into() })
}

pub fn preview(path: &Path) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_slice(&run_helper(path, true)?).map_err(|e| e.to_string())?;
    value["data_url"]
        .as_str()
        .filter(|url| url.starts_with("data:image/jpeg;base64,"))
        .map(str::to_owned)
        .ok_or_else(|| "Native image preview was unavailable.".into())
}

/// Reject provenance laundering even when an external semantic judgment accepted it.
pub fn validate_draft(
    source: &str,
    source_format: &str,
    draft: &crate::semantic::KnowledgeDraft,
) -> Result<(), String> {
    if !is_photo(source_format) {
        let native_image_evidence = draft
            .entities
            .iter()
            .map(|entity| &entity.evidence)
            .chain(draft.facts.iter().map(|fact| &fact.evidence))
            .chain(
                draft
                    .relationships
                    .iter()
                    .map(|relationship| &relationship.evidence),
            )
            .chain(draft.tags.iter().map(|tag| &tag.evidence))
            .chain(
                draft
                    .correction_candidates
                    .iter()
                    .map(|candidate| &candidate.evidence),
            )
            .chain(
                draft
                    .correction_alignments
                    .iter()
                    .map(|alignment| &alignment.candidate.evidence),
            )
            .chain(draft.source_update.iter().flat_map(|update| {
                [
                    Some(&update.evidence),
                    update.source_date_evidence.as_ref(),
                    update.source_revision_evidence.as_ref(),
                ]
                .into_iter()
                .flatten()
            }))
            .any(|evidence| {
                evidence.origin.eq_ignore_ascii_case("observed_pixels")
                    || evidence.offset_basis.as_deref() == Some("extracted_image_projection")
                    || evidence
                        .source_location
                        .as_deref()
                        .is_some_and(|location| location.starts_with("PHOTO "))
            });
        if source.starts_with("[PHOTO ")
            || native_image_evidence
            || draft.facts.iter().any(|fact| {
                matches!(
                    fact.property.to_ascii_lowercase().as_str(),
                    "image_pixels"
                        | "image_metadata"
                        | "image_caption"
                        | "image_text"
                        | "image_interpretation"
                )
            })
        {
            return Err("Native photo evidence requires a retained image source version; supplied text cannot establish pixel provenance.".into());
        }
        return Ok(());
    }
    if !source.starts_with("[PHOTO ") {
        return Err(
            "The retained image has no native photo projection; processing remains recoverable."
                .into(),
        );
    }
    fn channel<'a>(
        source: &'a str,
        evidence: &crate::semantic::EvidenceDraft,
    ) -> Result<&'a str, String> {
        let prefix = source
            .get(..evidence.byte_start)
            .ok_or("Photo evidence offset is outside its projection.")?;
        let start = prefix.rfind('\n').map_or(0, |index| index + 1);
        let line = source[start..].split('\n').next().unwrap_or_default();
        let closing = line
            .find("] ")
            .ok_or("Photo evidence has no channel locator.")?;
        let locator = &line[1..closing];
        if evidence
            .source_location
            .as_deref()
            .is_some_and(|value| value != locator)
        {
            return Err("Photo evidence locator disagrees with its decoded region; processing remains recoverable.".into());
        }
        let (_, channel) = line[..closing]
            .rsplit_once("; channel=")
            .ok_or("Photo evidence has no channel.")?;
        if evidence.origin != channel {
            return Err("Photo evidence origin disagrees with the decoded projection channel; processing remains recoverable.".into());
        }
        if channel != "observed_pixels"
            && evidence
                .qualifier
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return Err("Photo metadata, caption, OCR and generated interpretations require preserved uncertainty qualifications.".into());
        }
        if evidence
            .offset_basis
            .as_deref()
            .is_some_and(|basis| basis != "extracted_image_projection")
        {
            return Err(
                "Photo evidence offsets must refer to the extracted image projection.".into(),
            );
        }
        Ok(channel)
    }
    for entity in &draft.entities {
        channel(source, &entity.evidence)?;
        if entity.kind != "document" || entity.label != "Imported photo" {
            return Err("Named photo identities are uncertain; they cannot be published as authenticated entities.".into());
        }
    }
    for fact in &draft.facts {
        let channel = channel(source, &fact.evidence)?;
        let property = match channel {
            "observed_pixels" => "image_pixels",
            "file_metadata" => "image_metadata",
            "supplied_caption" => "image_caption",
            "ocr" => "image_text",
            "generated_interpretation" => "image_interpretation",
            _ => return Err("Unsupported image evidence channel.".into()),
        };
        if fact.property != property || fact.value != fact.evidence.quote {
            return Err("Photo facts must preserve the exact qualified channel statement; dates, counts and identities cannot be promoted beyond that evidence.".into());
        }
    }
    if !draft.relationships.is_empty()
        || !draft.tags.is_empty()
        || !draft.correction_candidates.is_empty()
        || !draft.correction_alignments.is_empty()
        || draft
            .source_update
            .as_ref()
            .is_some_and(|update| update.source_date.is_some() || update.source_revision.is_some())
    {
        return Err("Photo relationships and reliable source order have not been established by the decoded evidence.".into());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_helper(path: &Path, preview: bool) -> Result<Vec<u8>, String> {
    use std::{
        io::Read,
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let workspace = tempfile::tempdir().map_err(|e| e.to_string())?;
    let executable = workspace.path().join("photo-helper");
    std::fs::write(
        &executable,
        include_bytes!(concat!(env!("OUT_DIR"), "/photo-helper")),
    )
    .map_err(|e| e.to_string())?;
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    let mut stdout = tempfile::tempfile().map_err(|e| e.to_string())?;
    let mut stderr = tempfile::tempfile().map_err(|e| e.to_string())?;
    let mut command = Command::new(executable);
    command.arg(path);
    if preview {
        command.arg("preview");
    }
    let mut child = command
        .stdout(Stdio::from(stdout.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(stderr.try_clone().map_err(|e| e.to_string())?))
        .spawn()
        .map_err(|e| e.to_string())?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() > Duration::from_secs(60) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Native photo extraction exceeded its 60 second limit; the original remains retained.".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    use std::io::{Seek, SeekFrom};
    if !status.success() {
        stderr.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut detail = String::new();
        stderr
            .take(8192)
            .read_to_string(&mut detail)
            .map_err(|e| e.to_string())?;
        return Err(format!("Native image decoder failed: {detail}"));
    }
    stdout.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    stdout
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("Native photo result exceeded the 8 MiB projection limit.".into());
    }
    Ok(bytes)
}

#[cfg(not(target_os = "macos"))]
fn run_helper(_: &Path, _: bool) -> Result<Vec<u8>, String> {
    Err(
        "Native photo extraction is available on macOS; the retained original remains inspectable."
            .into(),
    )
}
