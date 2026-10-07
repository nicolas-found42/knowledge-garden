//! Native, on-device audio transcription boundary. The source asset remains authoritative.

use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
    time::{Duration, Instant},
};

pub const MAX_AUDIO_SEGMENT_MS: u64 = 30_000;
const MAX_WHISPER_RUNTIME: Duration = Duration::from_secs(120);
const MAX_TRANSCRIPT_JSON_BYTES: u64 = 1024 * 1024;

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

/// Offline ASR backed by the app-bundled, statically linked whisper.cpp runner
/// and pinned large-v3-turbo model. Apple AVFoundation is still used to inspect
/// and decode source containers before Whisper sees a bounded PCM WAV segment.
#[derive(Debug)]
pub struct WhisperAudioProcessor {
    assets_dir: PathBuf,
}

impl WhisperAudioProcessor {
    pub fn new(assets_dir: impl Into<PathBuf>) -> Self {
        Self {
            assets_dir: assets_dir.into(),
        }
    }

    fn model_path(&self) -> PathBuf {
        self.assets_dir.join("ggml-large-v3-turbo.bin")
    }
}

impl AudioProcessor for WhisperAudioProcessor {
    fn inspect(&self, path: &Path) -> Result<AudioInfo, String> {
        NativeAudioProcessor.inspect(path)
    }

    fn install_assets(&self) -> Result<(), String> {
        verify_model_once(&self.model_path())
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
        self.install_assets()?;
        let directory = tempfile::tempdir().map_err(|error| {
            format!("A private audio segment workspace could not be created: {error}")
        })?;
        let wav_path = directory.path().join("segment.wav");
        let extraction: ExtractedAudioSegment = run_helper(vec![
            "extract".into(),
            path_string(path)?,
            start_ms.to_string(),
            duration_ms.to_string(),
            path_string(&wav_path)?,
        ])?;
        if extraction.processed_duration_ms == 0 || extraction.processed_duration_ms > duration_ms {
            return Err("The native decoder returned an invalid bounded audio segment.".into());
        }
        let output_prefix = directory.path().join("transcript");
        let whisper_log = directory.path().join("whisper.log");
        run_whisper(
            vec![
                "-m".into(),
                path_string(&self.model_path())?,
                "-f".into(),
                path_string(&wav_path)?,
                "-l".into(),
                "en".into(),
                "-t".into(),
                "4".into(),
                "-oj".into(),
                "-np".into(),
                "-of".into(),
                path_string(&output_prefix)?,
            ],
            &whisper_log,
        )?;
        let transcript_path = output_prefix.with_extension("json");
        let transcript_size = std::fs::metadata(&transcript_path)
            .map_err(|error| format!("The local speech processor did not produce a complete transcript file: {error}"))?
            .len();
        if transcript_size > MAX_TRANSCRIPT_JSON_BYTES {
            return Err("The local speech processor returned an unexpectedly large result; this segment remains resumable.".into());
        }
        let output = std::fs::read(transcript_path).map_err(|error| {
            format!(
                "The local speech processor did not produce a complete transcript file: {error}"
            )
        })?;
        let transcript: WhisperOutput = serde_json::from_slice(&output).map_err(|error| {
            format!("The bundled speech model returned invalid segment data: {error}")
        })?;
        let segments: Vec<TranscriptSegment> = transcript
            .transcription
            .into_iter()
            .filter_map(|segment| {
                if segment.offsets.to <= segment.offsets.from {
                    return None;
                }
                Some(TranscriptSegment {
                    segment_id: String::new(),
                    start_ms: start_ms.saturating_add(segment.offsets.from),
                    end_ms: start_ms.saturating_add(segment.offsets.to),
                    text: segment.text.trim().to_owned(),
                    confidence: None,
                    alternatives: Vec::new(),
                    speaker: None,
                    speaker_state: "unidentified".into(),
                    final_result: true,
                })
            })
            .filter(|segment| segment.end_ms > segment.start_ms && !segment.text.is_empty())
            .collect();
        Ok(TranscriptionBatch {
            schema: 1,
            requested_start_ms: start_ms,
            requested_duration_ms: duration_ms,
            processed_duration_ms: extraction.processed_duration_ms,
            media_duration_ms: extraction.media_duration_ms,
            state: if segments.is_empty() { "no_recognized_speech" } else { "complete" }.into(),
            coverage: "partial".into(),
            detail: "Local Whisper transcription is a best-guess interpretation; confidence and alternatives are not available in this runner output. Segment timestamps are estimated. Silence, overlap, noise, and unrecognized speech are not distinguished; speakers remain unidentified. Compare wording and times with the retained original.".into(),
            segments,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtractedAudioSegment {
    processed_duration_ms: u64,
    media_duration_ms: u64,
}

#[derive(Debug, Deserialize)]
struct WhisperOutput {
    transcription: Vec<WhisperTextSegment>,
}

#[derive(Debug, Deserialize)]
struct WhisperTextSegment {
    offsets: WhisperOffsets,
    text: String,
}

#[derive(Debug, Deserialize)]
struct WhisperOffsets {
    from: u64,
    to: u64,
}

fn verify_model_once(path: &Path) -> Result<(), String> {
    static VERIFIED_MODELS: OnceLock<std::sync::Mutex<HashSet<PathBuf>>> = OnceLock::new();
    let canonical = path.canonicalize().map_err(|error| format!("The bundled large-v3-turbo model is unavailable; transcription remains resumable and the original is retained: {error}"))?;
    let verified = VERIFIED_MODELS.get_or_init(|| std::sync::Mutex::new(HashSet::new()));
    let mut verified = verified
        .lock()
        .map_err(|_| "The local speech-model verification state is unavailable.".to_owned())?;
    if verified.contains(&canonical) {
        return Ok(());
    }
    let mut child = Command::new("/usr/bin/shasum")
        .arg("-a")
        .arg("256")
        .arg(&canonical)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("The bundled model hash could not be checked with the macOS system utility: {error}"))?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            format!("The bundled model hash check could not be monitored: {error}")
        })? {
            break status;
        }
        if started.elapsed() >= Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Verifying the bundled local speech model exceeded 30 seconds; transcription remains resumable and the original remains available.".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    if !status.success() {
        return Err(
            "The macOS model hash utility could not verify the bundled speech model.".into(),
        );
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("The bundled model hash output could not be read: {error}"))?;
    let digest = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_owned();
    const EXPECTED: &str = "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69";
    if digest != EXPECTED {
        return Err(format!("The bundled large-v3-turbo model failed its pinned SHA-256 check (found {digest}); transcription remains resumable and the original is retained."));
    }
    verified.insert(canonical);
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_whisper(arguments: Vec<String>, stderr_path: &Path) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    use std::{fs, os::unix::fs::PermissionsExt};
    let bytes: &[u8] = include_bytes!(env!("KG_WHISPER_HELPER_BIN"));
    let digest = Sha256::digest(bytes);
    let directory = std::env::temp_dir().join("knowledge-garden-native");
    fs::create_dir_all(&directory).map_err(|error| {
        format!("The local speech helper directory could not be created: {error}")
    })?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).map_err(|error| {
        format!("The local speech helper directory could not be secured: {error}")
    })?;
    let helper = directory.join("whisper-cli");
    let digest_path = directory.join("whisper-cli.sha256");
    if !helper.is_file()
        || fs::read_to_string(&digest_path).unwrap_or_default().trim() != format!("{:x}", digest)
    {
        let temporary = directory.join("whisper-cli-tmp");
        fs::write(&temporary, bytes).map_err(|error| {
            format!("The bundled local speech helper could not be extracted: {error}")
        })?;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o700)).map_err(|error| {
            format!("The bundled local speech helper could not be made executable: {error}")
        })?;
        fs::rename(&temporary, &helper).map_err(|error| {
            format!("The bundled local speech helper could not be installed: {error}")
        })?;
        fs::write(digest_path, format!("{:x}", digest)).map_err(|error| {
            format!("The local speech helper version could not be recorded: {error}")
        })?;
    }
    let stderr = std::fs::File::create(stderr_path)
        .map_err(|error| format!("The local speech processor log could not be created: {error}"))?;
    let mut child = Command::new(helper)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|error| format!("The bundled local speech processor could not start; transcription remains resumable and the original is retained: {error}"))?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            format!("The local speech processor status could not be checked: {error}")
        })? {
            break status;
        }
        if started.elapsed() >= MAX_WHISPER_RUNTIME {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Local transcription exceeded the two-minute segment limit and was stopped; the range remains resumable and the retained original remains available.".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    if status.success() {
        Ok(())
    } else {
        let detail = std::fs::read(stderr_path)
            .map(|bytes| {
                let start = bytes.len().saturating_sub(2048);
                String::from_utf8_lossy(&bytes[start..]).into_owned()
            })
            .unwrap_or_default()
            .trim()
            .to_owned();
        Err(if detail.is_empty() {
            "The local speech processor did not complete this range; it remains resumable and the original is retained.".into()
        } else {
            detail
        })
    }
}

#[cfg(not(target_os = "macos"))]
fn run_whisper(_arguments: Vec<String>, _stderr_path: &Path) -> Result<(), String> {
    Err(
        "The bundled Whisper processor is available only on macOS; the original remains retained."
            .into(),
    )
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
