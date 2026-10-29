#![cfg(target_os = "macos")]
use knowledge_garden::application::{AcquisitionMethod, Application};
use std::{fs, path::Path};

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/video")
        .join(name)
}

#[test]
fn silent_video_keeps_frame_evidence_readable_after_reopen() {
    let collection = tempfile::tempdir().unwrap();
    let input = fixture("silent-no-audio-track.mp4");
    let mut app = Application::open(collection.path()).unwrap();
    let page = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_audio_jobs().unwrap();
    let page = app.open_source(&page.info.source_id).unwrap();
    let media = page.info.audio_processing.as_ref().unwrap();
    assert!(media.is_video());
    assert!(!media.tracks.as_ref().unwrap().audio.present);
    assert!(media.segments.is_empty());
    assert!(!media.frames.is_empty());
    assert!(
        page.body.contains("Sampled visual evidence"),
        "{}",
        page.body
    );
    assert!(page.body.contains("SURVEY CARD"), "{}", page.body);
    assert!(page.body.contains("not an authenticated capture time"));
    assert_eq!(
        fs::read(app.original_path(&page.info.source_id).unwrap()).unwrap(),
        fs::read(input).unwrap()
    );
    drop(app);
    let app = Application::open(collection.path()).unwrap();
    assert_eq!(
        app.open_source(&page.info.source_id).unwrap().body,
        page.body
    );
}

use knowledge_garden::{
    extraction::LocalExtractor,
    media::{
        AudioInfo, AudioProcessor, MediaInfo, NativeAudioProcessor, TranscriptSegment,
        TranscriptionBatch, VideoFrame,
    },
    semantic::{
        EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft, ProviderError, SemanticProvider,
        SourceUpdateDraft, SourceUpdateRole,
    },
};
use std::sync::Arc;

struct RecordedVideo {
    long: bool,
    damaged: bool,
    failures: std::sync::Mutex<std::collections::BTreeMap<u64, u32>>,
}
impl AudioProcessor for RecordedVideo {
    fn inspect_media(&self, path: &Path) -> Result<MediaInfo, String> {
        let mut media = NativeAudioProcessor.inspect_media(path)?;
        if self.long {
            media.duration_ms = 65_000;
            media.video.variable_frame_rate = Some(true);
        }
        Ok(media)
    }
    fn extract_frame(&self, path: &Path, at: u64, jpeg: &Path) -> Result<VideoFrame, String> {
        // Real decoding for the short acquired fixture; long-timeline tests repeat its
        // visual content at deterministic offsets to exercise durable window logic.
        let mut frame = NativeAudioProcessor.extract_frame(
            path,
            if self.long { at % 12_000 } else { at },
            jpeg,
        )?;
        if self.long {
            frame.actual_ms += (at / 12_000) * 12_000;
            frame.requested_ms = at;
        }
        if self.damaged && (30_000..60_000).contains(&at) {
            return Err("Recorded damaged video interval".into());
        }
        Ok(frame)
    }
    fn inspect(&self, path: &Path) -> Result<AudioInfo, String> {
        NativeAudioProcessor.inspect(path)
    }
    fn install_assets(&self) -> Result<(), String> {
        Ok(())
    }
    fn transcribe_segment(
        &self,
        _path: &Path,
        start: u64,
        duration: u64,
    ) -> Result<TranscriptionBatch, String> {
        assert!(duration <= 30_000);
        if let Some(remaining) = self.failures.lock().unwrap().get_mut(&start) {
            if *remaining > 0 {
                *remaining -= 1;
                return Err("Recorded transient decoder failure".into());
            }
        }
        if self.damaged && start == 30_000 {
            return Err("Recorded damaged audio interval".into());
        }
        Ok(TranscriptionBatch {
            schema: 1,
            requested_start_ms: start,
            requested_duration_ms: duration,
            processed_duration_ms: duration,
            media_duration_ms: if self.long { 65_000 } else { 12_000 },
            state: "complete".into(),
            coverage: "partial".into(),
            detail: "Recorded ASR for integrity testing; acoustic accuracy is not asserted.".into(),
            segments: vec![TranscriptSegment {
                segment_id: String::new(),
                start_ms: start,
                end_ms: start + 3000.min(duration),
                text: "The location is uncertain.".into(),
                confidence: None,
                alternatives: vec![],
                speaker: Some("unverified injected name".into()),
                speaker_state: "unknown".into(),
                final_result: true,
            }],
        })
    }
}

struct LocatedProvider;
impl SemanticProvider for LocatedProvider {
    fn form_knowledge(&self, text: &str) -> Result<KnowledgeDraft, ProviderError> {
        let evidence = |quote: &str, origin: &str| {
            let start = text.find(quote).unwrap();
            EvidenceDraft {
                quote: quote.into(),
                byte_start: start,
                byte_end: start + quote.len(),
                origin: origin.into(),
                qualifier: None,
                offset_basis: None,
                source_location: None,
            }
        };
        let speech = evidence("The location is uncertain.", "transcribed speech");
        let visual = evidence("SURVEY CARD", "OCR text");
        Ok(KnowledgeDraft {
            entities: vec![EntityDraft {
                kind: "recording".into(),
                label: "Survey video".into(),
                evidence: speech.clone(),
            }],
            facts: vec![
                FactDraft {
                    subject: "Survey video".into(),
                    property: "narrated location".into(),
                    value: "uncertain".into(),
                    evidence: speech.clone(),
                    record_key: None,
                },
                FactDraft {
                    subject: "Survey video".into(),
                    property: "title card".into(),
                    value: "SURVEY CARD".into(),
                    evidence: visual,
                    record_key: None,
                },
            ],
            source_update: Some(SourceUpdateDraft {
                role: SourceUpdateRole::CompleteReplacement,
                evidence: speech,
                certainty: 1.0,
                source_date: None,
                source_date_evidence: None,
                source_date_certainty: 0.0,
                source_revision: None,
                source_revision_evidence: None,
                source_revision_certainty: 0.0,
            }),
            ..KnowledgeDraft::default()
        })
    }
}
fn recorded_app(root: &Path, long: bool, damaged: bool) -> Application {
    Application::open_with_all_providers(
        root,
        Arc::new(LocatedProvider),
        Arc::new(LocalExtractor),
        Arc::new(RecordedVideo {
            long,
            damaged,
            failures: Default::default(),
        }),
    )
    .unwrap()
}

#[test]
fn narration_and_visuals_keep_separate_located_support_and_version_specific_previews() {
    let collection = tempfile::tempdir().unwrap();
    let mut app = recorded_app(collection.path(), false, false);
    let page = app
        .import_source(
            fixture("river-title-card-silent.mp4"),
            AcquisitionMethod::Picker,
        )
        .unwrap();
    app.resume_due_audio_jobs().unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    assert!(job.source_text.contains("[AUDIO 0–3000 ms"));
    assert!(job.source_text.contains("VIDEO FRAME 6000 ms"));
    let draft = LocatedProvider.form_knowledge(&job.source_text);
    app.finish_semantic_job(job, draft).unwrap();
    let page = app.open_source(&page.info.source_id).unwrap();
    assert_eq!(
        page.info.semantic_state, "complete",
        "{:?}",
        page.info.semantic_error
    );
    let knowledge = app
        .open_knowledge_page(&page.knowledge_pages[0].page_id)
        .unwrap();
    assert!(knowledge
        .markdown
        .contains("Narrated speech is not visual observation"));
    assert!(knowledge
        .markdown
        .contains("not an authenticated capture time"));
    let preview = app
        .preview_video_frame(
            &page.info.source_id,
            &page.info.sha256,
            "original.mp4",
            7000,
        )
        .unwrap();
    assert!(preview.data_url.starts_with("data:image/jpeg;base64,"));
    assert!(preview.frame.actual_ms.abs_diff(7000) <= 250);
    assert!(app
        .preview_video_frame(&page.info.source_id, &"f".repeat(64), "original.mp4", 7000)
        .is_err());
    assert!(app
        .preview_video_frame(
            &page.info.source_id,
            &page.info.sha256,
            "../original.mp4",
            7000
        )
        .is_err());
    assert!(app
        .preview_video_frame(
            &page.info.source_id,
            &page.info.sha256,
            "original.mp4",
            12000
        )
        .is_err());
    assert!(page
        .info
        .audio_processing
        .as_ref()
        .unwrap()
        .segments
        .iter()
        .all(|s| s.speaker.is_none()));
}

#[test]
fn interrupted_long_video_resumes_without_duplicate_frames_or_facts() {
    let collection = tempfile::tempdir().unwrap();
    let mut app = recorded_app(collection.path(), true, false);
    let page = app
        .import_source(
            fixture("river-title-card-silent.mp4"),
            AcquisitionMethod::Picker,
        )
        .unwrap();
    app.resume_due_audio_jobs().unwrap();
    let first = app
        .open_source(&page.info.source_id)
        .unwrap()
        .info
        .audio_processing
        .unwrap();
    assert_eq!(first.next_start_ms, 30_000);
    let path = app.page_path(&page.info.source_id).unwrap();
    let markdown = fs::read_to_string(&path).unwrap();
    fs::write(
        path,
        markdown.replace("state: pending", "state: processing"),
    )
    .unwrap();
    drop(app);
    let mut app = recorded_app(collection.path(), true, false);
    assert_eq!(
        app.open_source(&page.info.source_id)
            .unwrap()
            .info
            .audio_processing
            .unwrap()
            .next_start_ms,
        30_000
    );
    app.resume_due_audio_jobs().unwrap();
    app.resume_due_audio_jobs().unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    let draft = LocatedProvider.form_knowledge(&job.source_text);
    app.finish_semantic_job(job, draft).unwrap();
    let completed = app.open_source(&page.info.source_id).unwrap();
    let media = completed.info.audio_processing.as_ref().unwrap();
    assert_eq!(media.next_start_ms, 65_000);
    assert_eq!(media.processing_interruptions, 1);
    assert_eq!(
        media
            .frames
            .iter()
            .map(|f| &f.frame_id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        media.frames.len()
    );
    assert_eq!(media.segments.len(), 3);
    for frame in first.frames {
        assert!(media.frames.iter().any(|f| f.frame_id == frame.frame_id));
    }
    assert_eq!(completed.info.semantic_state, "complete");
    let knowledge = app
        .open_knowledge_page(&completed.knowledge_pages[0].page_id)
        .unwrap();
    assert_eq!(knowledge.markdown.matches("Fact identity:").count(), 2);
    app.resume_due_audio_jobs().unwrap();
    assert_eq!(
        app.open_source(&page.info.source_id).unwrap().markdown,
        completed.markdown
    );
}

#[test]
fn title_card_date_cannot_become_authenticated_capture_time() {
    let collection = tempfile::tempdir().unwrap();
    let mut app = recorded_app(collection.path(), false, false);
    let page = app
        .import_source(
            fixture("river-title-card-silent.mp4"),
            AcquisitionMethod::Picker,
        )
        .unwrap();
    app.resume_due_audio_jobs().unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    let mut draft = LocatedProvider.form_knowledge(&job.source_text).unwrap();
    draft.facts[1].property = "capture_date".into();
    draft.facts[1].value = "2019-05-04".into();
    app.finish_semantic_job(job, Ok(draft)).unwrap();
    let page = app.open_source(&page.info.source_id).unwrap();
    assert_ne!(page.info.semantic_state, "complete");
    assert!(page.info.semantic_error.unwrap().contains("capture time"));
    assert!(page.knowledge_pages.is_empty());
    assert!(app.original_path(&page.info.source_id).unwrap().is_file());
}

#[test]
fn damaged_window_discloses_both_track_gaps_and_continues_without_losing_prior_evidence() {
    let collection = tempfile::tempdir().unwrap();
    let mut app = recorded_app(collection.path(), true, true);
    let page = app
        .import_source(
            fixture("river-title-card-silent.mp4"),
            AcquisitionMethod::Picker,
        )
        .unwrap();
    app.resume_due_audio_jobs().unwrap();
    let retry_pattern = regex::Regex::new(r"(?m)^(\s*retry_at_ms:) \d+").unwrap();
    for _ in 0..3 {
        app.resume_due_audio_jobs().unwrap();
        let path = app.page_path(&page.info.source_id).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        let reset = retry_pattern.replace_all(&content, "$1 0");
        fs::write(path, reset.as_bytes()).unwrap();
    }
    let partial = app.open_source(&page.info.source_id).unwrap();
    assert_eq!(
        partial
            .info
            .audio_processing
            .as_ref()
            .unwrap()
            .next_start_ms,
        60_000
    );
    assert!(partial.body.contains("audio coverage gap · 30000–60000 ms"));
    assert!(partial.body.contains("video coverage gap"));
    app.resume_due_audio_jobs().unwrap();
    let completed = app.open_source(&page.info.source_id).unwrap();
    let media = completed.info.audio_processing.unwrap();
    assert_eq!(media.next_start_ms, 65_000);
    assert_eq!(media.segments.len(), 2);
    assert!(media.frames.iter().any(|f| f.time_ms < 30_000));
    assert!(media.frames.iter().any(|f| f.time_ms >= 60_000));
    assert!(completed.body.contains("Recorded damaged video interval"));
    assert!(app.original_path(&page.info.source_id).unwrap().is_file());
}

#[test]
fn native_damaged_video_retains_exact_original_and_inspectable_coverage() {
    let collection = tempfile::tempdir().unwrap();
    let input = fixture("truncated-damaged-tail.mp4");
    let mut app = Application::open(collection.path()).unwrap();
    let page = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(
        fs::read(app.original_path(&page.info.source_id).unwrap()).unwrap(),
        fs::read(input).unwrap()
    );
    assert!(serde_json::to_string(&page.info.extraction_coverage)
        .unwrap()
        .contains("video_frames"));
    assert!(!page.info.extraction_detail.is_empty());
}

#[test]
fn native_variable_frame_rate_mov_uses_decoded_times_instead_of_frame_numbers() {
    let input = fixture("variable-frame-rate.mov");
    let media = NativeAudioProcessor.inspect_media(&input).unwrap();
    assert_eq!(media.video.variable_frame_rate, Some(true));
    assert!(!media.audio.present);
    let collection = tempfile::tempdir().unwrap();
    let mut app = Application::open(collection.path()).unwrap();
    let page = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    let preview = app
        .preview_video_frame(&page.info.source_id, &page.info.sha256, "original.mov", 800)
        .unwrap();
    assert_eq!(preview.frame.actual_ms, 800);
    assert!(preview.data_url.starts_with("data:image/jpeg;base64,"));
    app.resume_due_audio_jobs().unwrap();
    let page = app.open_source(&page.info.source_id).unwrap();
    assert!(page.body.contains("Variable frame timing was detected"));
    assert_eq!(
        fs::read(app.original_path(&page.info.source_id).unwrap()).unwrap(),
        fs::read(input).unwrap()
    );
}

struct RecordedVisualTransport;
impl knowledge_garden::providers::SystemOneTransport for RecordedVisualTransport {
    fn complete(
        &self,
        _key: &str,
        request: &serde_json::Value,
    ) -> Result<serde_json::Value, ProviderError> {
        let mut answers = serde_json::Map::new();
        for (key, question) in request["questions"].as_object().unwrap() {
            let answer = if question["type"] == "noul" {
                serde_json::json!({"type":"noul", "noul":0.99})
            } else {
                let criteria = question["criteria"].as_object().unwrap();
                let choice = if key == "source_update_role" {
                    "complete_replacement"
                } else if key == "source_update_evidence" {
                    criteria.keys().find(|s| s.starts_with("span_")).unwrap()
                } else if criteria.contains_key("none") {
                    "none"
                } else {
                    criteria.keys().next().unwrap()
                };
                serde_json::json!({"type":"choice","choice":choice,"probabilities":{choice:0.99}})
            };
            answers.insert(key.clone(), answer);
        }
        Ok(serde_json::json!({"model":"typesafe/jev-recorded-video-integrity", "answers":answers}))
    }
}

#[test]
fn jev_visual_channels_form_qualified_video_knowledge_without_capture_metadata() {
    let collection = tempfile::tempdir().unwrap();
    let provider = knowledge_garden::providers::JevSemanticProvider::with_transport(
        "recorded-only".into(),
        Arc::new(RecordedVisualTransport),
    );
    let mut app =
        Application::open_with_semantic_provider(collection.path(), Arc::new(provider)).unwrap();
    let page = app
        .import_source(
            fixture("silent-no-audio-track.mp4"),
            AcquisitionMethod::Picker,
        )
        .unwrap();
    app.resume_due_audio_jobs().unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let page = app.open_source(&page.info.source_id).unwrap();
    assert_eq!(
        page.info.semantic_state, "complete",
        "{:?}",
        page.info.semantic_error
    );
    assert_eq!(page.knowledge_pages.len(), 1);
    let knowledge = app
        .open_knowledge_page(&page.knowledge_pages[0].page_id)
        .unwrap();
    assert!(knowledge.markdown.contains("video frame text"));
    assert!(knowledge.markdown.contains("SURVEY CARD"));
    assert!(knowledge
        .markdown
        .contains("not an authenticated capture time"));
    assert!(!knowledge.markdown.contains("**capture date:**"));
}

#[test]
fn pasted_video_projection_cannot_claim_acquired_frame_provenance() {
    let collection = tempfile::tempdir().unwrap();
    let input = collection.path().join("pasted.txt");
    fs::write(&input, "Machine-generated transcript from retained audio.\n[VIDEO FRAME 6000 ms; channel=ocr] SURVEY CARD (OCR transcription; not an authenticated capture time)\n").unwrap();
    let provider = knowledge_garden::providers::JevSemanticProvider::with_transport(
        "recorded-only".into(),
        Arc::new(RecordedVisualTransport),
    );
    let mut app = Application::open_with_semantic_provider(
        collection.path().join("collection"),
        Arc::new(provider),
    )
    .unwrap();
    let page = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let page = app.open_source(&page.info.source_id).unwrap();
    assert_ne!(page.info.semantic_state, "complete");
    assert!(page.knowledge_pages.is_empty());
    assert!(page.info.semantic_error.unwrap().contains("retained"));
}

#[test]
fn successful_video_window_resets_failures_before_the_next_interval() {
    let collection = tempfile::tempdir().unwrap();
    let processor = RecordedVideo {
        long: true,
        damaged: false,
        failures: std::sync::Mutex::new([(0, 2), (30_000, 1)].into_iter().collect()),
    };
    let mut app = Application::open_with_all_providers(
        collection.path(),
        Arc::new(LocatedProvider),
        Arc::new(LocalExtractor),
        Arc::new(processor),
    )
    .unwrap();
    let page = app
        .import_source(
            fixture("river-title-card-silent.mp4"),
            AcquisitionMethod::Picker,
        )
        .unwrap();
    let pattern = regex::Regex::new(r"(?m)^(\s*retry_at_ms:) \d+").unwrap();
    for _ in 0..3 {
        app.resume_due_audio_jobs().unwrap();
        let path = app.page_path(&page.info.source_id).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        fs::write(path, pattern.replace_all(&content, "$1 0").as_bytes()).unwrap();
    }
    let first = app.open_source(&page.info.source_id).unwrap();
    let media = first.info.audio_processing.as_ref().unwrap();
    assert_eq!(media.next_start_ms, 30_000);
    assert_eq!(
        media.failed_attempts, 0,
        "a successfully resolved video window needs its own retry budget"
    );
    app.resume_due_audio_jobs().unwrap();
    let next = app.open_source(&page.info.source_id).unwrap();
    let media = next.info.audio_processing.as_ref().unwrap();
    assert_eq!(
        media.next_start_ms, 30_000,
        "one failure must not skip the next window"
    );
    assert_eq!(media.failed_attempts, 1);
    assert!(media.retry_at_ms.is_some());
    assert!(!media
        .gaps
        .iter()
        .any(|gap| gap.track == "audio" && gap.start_ms == 30_000));
}
