//! Bounded macOS ImageIO/Vision support for common still photographs.
//! The original asset remains authoritative; this module creates only projections.

use crate::semantic::ProviderError;
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path, process::Command};
use tempfile::NamedTempFile;

const NATIVE_HELPER: &str = include_str!("photo_native.swift");
const MAX_IMAGE_BYTES: u64 = 100 * 1024 * 1024;
const MAX_INSPECTION_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrRegion {
    pub text: String,
    pub confidence: f32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoInspection {
    pub width: u32,
    pub height: u32,
    pub capture_date: Option<String>,
    pub caption: Option<String>,
    pub ocr: Vec<OcrRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualStatement {
    pub text: String,
    #[serde(default)]
    pub region: Option<VisualRegion>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VisualRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub trait PhotoAnalysisProvider: Send + Sync {
    fn analyze(&self, bytes: &[u8], mime_type: &str) -> Result<Vec<VisualStatement>, ProviderError>;
}

pub fn visual_projection(statements: Vec<VisualStatement>) -> Result<String, ProviderError> {
    if statements.len() > 64 {
        return Err(ProviderError::permanent("Photo analysis returned too many visual statements.".into()));
    }
    statements.into_iter().enumerate().map(|(index, statement)| {
        let text = clean_field(statement.text, 500);
        if text.is_empty() {
            return Err(ProviderError::permanent("Photo analysis returned an empty visual statement.".into()));
        }
        let location = statement.region.filter(valid_visual_region).map(|region| {
            format!("x={:.5},y={:.5},width={:.5},height={:.5}", region.x, region.y, region.width, region.height)
        }).unwrap_or_else(|| "location=whole_image_fallback".into());
        Ok(format!("[PHOTO visual_inference {index} {location}] {text}"))
    }).collect::<Result<Vec<_>, _>>().map(|lines| lines.join("\n"))
}

fn valid_visual_region(region: &VisualRegion) -> bool {
    let (x, y, width, height) = (region.x, region.y, region.width, region.height);
    x.is_finite() && y.is_finite() && width.is_finite() && height.is_finite()
        && x >= 0.0 && y >= 0.0 && width > 0.0 && height > 0.0
        && x + width <= 1.0 && y + height <= 1.0
}

impl PhotoInspection {
    /// A tagged projection used for searching and exact evidence spans. Each line is
    /// deliberately labeled so downstream judgments cannot confuse metadata, captions,
    /// or OCR with visual observation.
    pub fn semantic_text(&self) -> String {
        let mut lines = vec![format!("[PHOTO dimensions] {} x {} pixels", self.width, self.height)];
        if let Some(date) = self.capture_date.as_deref().filter(|value| !value.trim().is_empty()) {
            lines.push(format!("[PHOTO metadata.capture_datetime] {date}"));
        }
        if let Some(caption) = self.caption.as_deref().filter(|value| !value.trim().is_empty()) {
            lines.push(format!("[PHOTO supplied_caption] {caption}"));
        }
        for (index, line) in self.ocr.iter().enumerate() {
            let rect = valid_rect(line).map(|(x, y, width, height)| {
                format!("x={x:.5},y={y:.5},width={width:.5},height={height:.5}")
            }).unwrap_or_else(|| "location=whole_image_fallback".into());
            lines.push(format!("[PHOTO OCR {index} {rect}] {}", line.text.trim()));
        }
        lines.join("\n")
    }
}

pub fn is_supported_format(format: &str) -> bool {
    matches!(format.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png" | "heic" | "heif" | "tif" | "tiff")
}

pub fn mime_type(format: &str) -> &'static str {
    match format.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "heic" | "heif" => "image/heic",
        "tif" | "tiff" => "image/tiff",
        _ => "application/octet-stream",
    }
}

pub fn inspect(path: &Path) -> std::result::Result<PhotoInspection, ProviderError> {
    run_helper("inspect", path).and_then(|bytes| {
        if bytes.len() > MAX_INSPECTION_BYTES {
            return Err(ProviderError::permanent("Image inspection exceeded its output limit.".into()));
        }
        let value: PhotoInspection = serde_json::from_slice(&bytes)
            .map_err(|_| ProviderError::permanent("Image inspection returned unreadable data.".into()))?;
        validate_inspection(value)
    })
}

pub fn preview_data_url(path: &Path) -> std::result::Result<String, ProviderError> {
    let bytes = run_helper("preview", path)?;
    let encoded = String::from_utf8(bytes)
        .map_err(|_| ProviderError::permanent("Image preview returned unreadable data.".into()))?;
    let compact = encoded.trim();
    if compact.is_empty() || compact.len() > 12 * 1024 * 1024 || !compact.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=')) {
        return Err(ProviderError::permanent("Image preview exceeded its safe size or format limit.".into()));
    }
    Ok(format!("data:image/jpeg;base64,{compact}"))
}

fn run_helper(operation: &str, path: &Path) -> std::result::Result<Vec<u8>, ProviderError> {
    let metadata = std::fs::metadata(path)
        .map_err(|_| ProviderError::permanent("The retained photo could not be read.".into()))?;
    if !metadata.is_file() || metadata.len() > MAX_IMAGE_BYTES {
        return Err(ProviderError::permanent("Photo exceeds the 100 MiB inspection limit.".into()));
    }
    let mut script = NamedTempFile::new()
        .map_err(|_| ProviderError::recoverable("Could not prepare the macOS photo helper.".into()))?;
    script.write_all(NATIVE_HELPER.as_bytes())
        .map_err(|_| ProviderError::recoverable("Could not prepare the macOS photo helper.".into()))?;
    let output = Command::new("/usr/bin/swift")
        .arg(script.path())
        .arg(operation)
        .arg(path)
        .output()
        .map_err(|_| ProviderError::recoverable("The macOS ImageIO/Vision helper is unavailable; the original photo remains available.".into()))?;
    if !output.status.success() {
        return Err(ProviderError::recoverable("The macOS ImageIO/Vision helper could not complete this photo operation.".into()));
    }
    if output.stdout.len() > MAX_INSPECTION_BYTES {
        return Err(ProviderError::permanent("Image helper output exceeded its safe size limit.".into()));
    }
    Ok(output.stdout)
}

fn validate_inspection(mut inspection: PhotoInspection) -> std::result::Result<PhotoInspection, ProviderError> {
    if inspection.width == 0 || inspection.height == 0 || inspection.ocr.len() > 10_000 {
        return Err(ProviderError::permanent("Image inspection returned invalid dimensions or too many OCR regions.".into()));
    }
    inspection.capture_date = inspection.capture_date.map(|text| clean_field(text, 128)).filter(|s| !s.is_empty());
    inspection.caption = inspection.caption.map(|text| clean_field(text, 8_000)).filter(|s| !s.is_empty());
    inspection.ocr.retain(|line| !clean_field(line.text.clone(), 2_000).is_empty());
    for line in &mut inspection.ocr {
        line.text = clean_field(line.text.clone(), 2_000);
        if !line.confidence.is_finite() || !(0.0..=1.0).contains(&line.confidence) {
            return Err(ProviderError::permanent("Vision returned an invalid OCR confidence.".into()));
        }
    }
    Ok(inspection)
}

fn clean_field(text: String, max_chars: usize) -> String {
    text.chars().filter(|character| !character.is_control() || *character == '\t')
        .take(max_chars).collect::<String>().trim().to_owned()
}

fn valid_rect(line: &OcrRegion) -> Option<(f64, f64, f64, f64)> {
    let (x, y, width, height) = (line.x, line.y, line.width, line.height);
    (x.is_finite() && y.is_finite() && width.is_finite() && height.is_finite()
        && x >= 0.0 && y >= 0.0 && width > 0.0 && height > 0.0
        && x + width <= 1.0 && y + height <= 1.0)
        .then_some((x, y, width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_keeps_metadata_caption_and_ocr_channels_separate() {
        let inspection = PhotoInspection {
            width: 480, height: 360,
            capture_date: Some("2026:04:11 12:30:00".into()),
            caption: Some("May 17, 2024 Riverside visit".into()),
            ocr: vec![OcrRegion { text: "RIVER STAGE 2.4 M".into(), confidence: 0.99,
                x: 0.1, y: 0.2, width: 0.5, height: 0.1 }],
        };
        let text = inspection.semantic_text();
        assert!(text.contains("[PHOTO metadata.capture_datetime] 2026:04:11 12:30:00"));
        assert!(text.contains("[PHOTO supplied_caption] May 17, 2024 Riverside visit"));
        assert!(text.contains("[PHOTO OCR 0 x=0.10000,y=0.20000,width=0.50000,height=0.10000] RIVER STAGE 2.4 M"));
        assert_eq!(text.lines().count(), 4);
    }

    #[test]
    fn invalid_ocr_boxes_use_whole_image_fallback() {
        let text = PhotoInspection {
            width: 1, height: 1, capture_date: None, caption: None,
            ocr: vec![OcrRegion { text: "text".into(), confidence: 0.5,
                x: f64::NAN, y: 0.2, width: 0.5, height: 0.3 }],
        }.semantic_text();
        assert!(text.contains("location=whole_image_fallback"));
    }

    #[test]
    fn invalid_visual_regions_are_disclosed_as_whole_image_fallback() {
        let text = visual_projection(vec![VisualStatement {
            text: "A red apple on a saucer.".into(),
            region: Some(VisualRegion { x: 0.8, y: 0.5, width: 0.4, height: 0.2 }),
        }]).unwrap();
        assert!(text.contains("location=whole_image_fallback"));
    }
}
