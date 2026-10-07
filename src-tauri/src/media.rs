//! Native, on-device audio transcription boundary. The source asset remains authoritative.

use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command};

pub const MAX_AUDIO_SEGMENT_MS: u64 = 30_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioInfo {
    pub format: String,
    pub duration_ms: u64,
    pub sample_rate: f64,
    pub channels: u32,
    pub codec: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptSegment {
    #[serde(default)]
    pub segment_id: String,
    #[serde(alias = "startMs")]
    pub start_ms: u64,
    #[serde(alias = "endMs")]
    pub end_ms: u64,
    pub text: String,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub alternatives: Vec<String>,
    #[serde(default)]
    pub speaker: Option<String>,
    #[serde(default)]
    #[serde(alias = "speakerState")]
    pub speaker_state: String,
    #[serde(default)]
    #[serde(alias = "final")]
    pub final_result: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionBatch {
    pub schema: u32,
    #[serde(alias = "requestedStartMs")]
    pub requested_start_ms: u64,
    #[serde(alias = "requestedDurationMs")]
    pub requested_duration_ms: u64,
    #[serde(alias = "processedDurationMs")]
    pub processed_duration_ms: u64,
    #[serde(alias = "mediaDurationMs")]
    pub media_duration_ms: u64,
    pub state: String,
    pub coverage: String,
    pub detail: String,
    /// Native helper timestamps are absolute offsets in the retained media asset.
    #[serde(default)]
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioProcessingInfo {
    pub state: String,
    pub duration_ms: u64,
    pub next_start_ms: u64,
    pub segment_duration_ms: u64,
    pub attempts: u32,
    pub retry_at_ms: Option<u64>,
    pub detail: String,
    pub segments: Vec<TranscriptSegment>,
}

/// Kept as an application-level seam so recovery tests can supply deterministic ASR outcomes.
pub trait AudioProcessor: Send + Sync {
    fn inspect(&self, path: &Path) -> Result<AudioInfo, String>;
    fn install_assets(&self) -> Result<(), String>;
    fn transcribe_segment(
        &self,
        path: &Path,
        start_ms: u64,
        duration_ms: u64,
    ) -> Result<TranscriptionBatch, String>;
}

#[derive(Debug, Default)]
pub struct NativeAudioProcessor;

impl AudioProcessor for NativeAudioProcessor {
    fn inspect(&self, path: &Path) -> Result<AudioInfo, String> {
        run_helper(vec!["inspect".into(), path_string(path)?])
    }

    fn install_assets(&self) -> Result<(), String> {
        let report: CapabilityReport = run_helper(vec!["install".into()])?;
        if report.asset_status == "installed" && report.module_available {
            Ok(())
        } else {
            Err(format!(
                "The local {} speech assets are not ready ({}).",
                report.locale, report.asset_status
            ))
        }
    }

    fn transcribe_segment(
        &self,
        path: &Path,
        start_ms: u64,
        duration_ms: u64,
    ) -> Result<TranscriptionBatch, String> {
        if duration_ms == 0 || duration_ms > MAX_AUDIO_SEGMENT_MS {
            return Err("Audio work must be limited to a non-empty 30-second segment.".into());
        }
        run_helper(vec![
            "transcribe".into(),
            path_string(path)?,
            start_ms.to_string(),
            duration_ms.to_string(),
        ])
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CapabilityReport {
    locale: String,
    module_available: bool,
    asset_status: String,
}

fn path_string(path: &Path) -> Result<String, String> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        "The audio file path cannot be represented by the native media helper.".into()
    })
}

fn run_helper<T: for<'de> Deserialize<'de>>(arguments: Vec<String>) -> Result<T, String> {
    #[cfg(target_os = "macos")]
    {
        let helper = native_helper_path()?;
        let output = Command::new(helper)
            .args(arguments)
            .output()
            .map_err(|error| format!("The native audio helper could not start: {error}"))?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(if detail.is_empty() {
                "The native audio helper did not complete this operation.".into()
            } else {
                detail
            });
        }
        serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("The native audio helper returned invalid data: {error}"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = arguments;
        Err("Native audio decoding and transcription are available only on macOS; the original remains retained.".into())
    }
}

#[cfg(target_os = "macos")]
fn native_helper_path() -> Result<std::path::PathBuf, String> {
    use sha2::{Digest, Sha256};
    use std::{fs, os::unix::fs::PermissionsExt};

    let bytes: &[u8] = include_bytes!(env!("KG_MEDIA_HELPER_BIN"));
    let digest = Sha256::digest(bytes);
    let directory = std::env::temp_dir().join("knowledge-garden-native");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("The native helper directory could not be created: {error}"))?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("The native helper directory could not be secured: {error}"))?;
    // Keep one stable executable identity across application rebuilds so Apple's
    // AssetInventory does not treat every helper hash as a fresh speech client.
    let path = directory.join("media-helper");
    let digest_path = directory.join("media-helper.sha256");
    let installed_digest = fs::read_to_string(&digest_path).unwrap_or_default();
    if !path.is_file() || installed_digest.trim() != format!("{:x}", digest) {
        let temporary = directory.join("media-helper-tmp");
        fs::write(&temporary, bytes)
            .map_err(|error| format!("The native audio helper could not be extracted: {error}"))?;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o700)).map_err(|error| {
            format!("The native audio helper could not be made executable: {error}")
        })?;
        fs::rename(&temporary, &path)
            .map_err(|error| format!("The native audio helper could not be installed: {error}"))?;
        fs::write(digest_path, format!("{:x}", digest)).map_err(|error| {
            format!("The native audio helper version could not be recorded: {error}")
        })?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transcription_segment_carries_time_confidence_alternatives_and_unidentified_speaker() {
        let segment = TranscriptSegment {
            segment_id: "stable-segment".into(),
            start_ms: 1_200,
            end_ms: 2_400,
            text: "It could be fifteen.".into(),
            confidence: Some(0.42),
            alternatives: vec!["It could be fifty.".into()],
            speaker: None,
            speaker_state: "unidentified".into(),
            final_result: true,
        };
        let encoded = serde_json::to_string(&segment).unwrap();
        let decoded: TranscriptSegment = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, segment);
        assert!(decoded.speaker.is_none());
    }

    #[test]
    fn native_camel_case_output_decodes_and_persisted_api_remains_snake_case() {
        let encoded_native = r#"{"schema":1,"requestedStartMs":1200,"requestedDurationMs":30000,"processedDurationMs":28000,"mediaDurationMs":60000,"state":"complete","coverage":"partial","detail":"local","segments":[{"startMs":1200,"endMs":2400,"text":"It could be fifteen.","confidence":0.42,"alternatives":["It could be fifty."],"speaker":null,"speakerState":"unidentified","final":true}]}"#;
        let decoded: TranscriptionBatch = serde_json::from_str(encoded_native).unwrap();
        assert_eq!(decoded.requested_start_ms, 1200);
        assert_eq!(decoded.segments[0].start_ms, 1200);
        assert!(decoded.segments[0].speaker.is_none());
        let persisted = serde_json::to_string(&decoded).unwrap();
        assert!(persisted.contains("requested_start_ms"));
        assert!(persisted.contains("speaker_state"));
        assert!(persisted.contains("final_result"));
    }

    #[test]
    fn transcription_batches_reject_no_schema_fields_and_preserve_partial_coverage() {
        let batch = TranscriptionBatch {
            schema: 1,
            requested_start_ms: 30_000,
            requested_duration_ms: 30_000,
            processed_duration_ms: 30_000,
            media_duration_ms: 90_000,
            state: "complete".into(),
            coverage: "partial".into(),
            detail: "Overlapping voices are not separated; speakers are unidentified.".into(),
            segments: vec![],
        };
        let encoded = serde_json::to_string(&batch).unwrap();
        let decoded: TranscriptionBatch = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, batch);
        assert_eq!(decoded.coverage, "partial");
    }
}
