use knowledge_garden::{
    application::{AcquisitionMethod, Application, PageSearchRequest},
    extraction::LocalExtractor,
    media::{
        AudioInfo, AudioProcessor, TranscriptSegment, TranscriptionBatch, WhisperAudioProcessor,
    },
    semantic::{
        EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft, ProviderError, SemanticProvider,
        SourceUpdateDraft, SourceUpdateRole,
    },
};
use sha2::Digest;
use std::{
    fs,
    panic::{catch_unwind, AssertUnwindSafe},
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tempfile::tempdir;

static LOCAL_ASR_TEST_LOCK: Mutex<()> = Mutex::new(());

struct EmptyProvider;
impl SemanticProvider for EmptyProvider {
    fn form_knowledge(&self, _text: &str) -> Result<KnowledgeDraft, ProviderError> {
        Ok(KnowledgeDraft::default())
    }
}

struct VersionedAudioProvider;
impl SemanticProvider for VersionedAudioProvider {
    fn form_knowledge(&self, text: &str) -> Result<KnowledgeDraft, ProviderError> {
        let quote = if text.contains("new recording words") {
            "new recording words"
        } else {
            "old recording words"
        };
        let start = text.find(quote).unwrap();
        let evidence = EvidenceDraft {
            quote: quote.into(),
            byte_start: start,
            byte_end: start + quote.len(),
            origin: "transcribed speech".into(),
            qualifier: None,
            offset_basis: None,
            source_location: None,
        };
        Ok(KnowledgeDraft {
            entities: vec![EntityDraft {
                kind: "recording".into(),
                label: "visit recording".into(),
                evidence: evidence.clone(),
            }],
            facts: vec![FactDraft {
                subject: "visit recording".into(),
                property: "spoken words".into(),
                value: quote.into(),
                evidence: evidence.clone(),
                record_key: None,
            }],
            source_update: Some(SourceUpdateDraft {
                role: SourceUpdateRole::CompleteReplacement,
                evidence,
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

struct FixedAudio;
impl AudioProcessor for FixedAudio {
    fn inspect(&self, _path: &Path) -> Result<AudioInfo, String> {
        Ok(AudioInfo {
            format: "m4a".into(),
            duration_ms: 65_000,
            sample_rate: 48_000.0,
            channels: 2,
            codec: "AAC".into(),
        })
    }
    fn install_assets(&self) -> Result<(), String> {
        Ok(())
    }
    fn transcribe_segment(
        &self,
        _path: &Path,
        start_ms: u64,
        duration_ms: u64,
    ) -> Result<TranscriptionBatch, String> {
        assert!(duration_ms <= 30_000);
        let text = if start_ms == 0 {
            "The first fixture phrase."
        } else {
            "The later fixture phrase."
        };
        Ok(TranscriptionBatch {
            schema: 1,
            requested_start_ms: start_ms,
            requested_duration_ms: duration_ms,
            processed_duration_ms: duration_ms,
            media_duration_ms: 65_000,
            state: "complete".into(),
            coverage: "partial".into(),
            detail: "Synthetic public-boundary result; speaker unknown.".into(),
            segments: vec![TranscriptSegment {
                segment_id: String::new(),
                start_ms: start_ms + 1_000,
                end_ms: start_ms + 2_000,
                text: text.into(),
                confidence: Some(0.51),
                alternatives: vec!["An alternative phrase.".into()],
                speaker: None,
                speaker_state: "unidentified".into(),
                final_result: true,
            }],
        })
    }
}

struct VersionedAudio {
    transcribed_assets: Arc<Mutex<Vec<Vec<u8>>>>,
}

struct SilentReplacementAudio;
impl AudioProcessor for SilentReplacementAudio {
    fn inspect(&self, _path: &Path) -> Result<AudioInfo, String> {
        Ok(AudioInfo {
            format: "wav".into(),
            duration_ms: 1_000,
            sample_rate: 48_000.0,
            channels: 1,
            codec: "test audio fixture".into(),
        })
    }
    fn install_assets(&self) -> Result<(), String> {
        Ok(())
    }
    fn transcribe_segment(
        &self,
        path: &Path,
        start_ms: u64,
        duration_ms: u64,
    ) -> Result<TranscriptionBatch, String> {
        let candidate = fs::read(path).map_err(|error| error.to_string())? == b"silent replacement";
        Ok(TranscriptionBatch {
            schema: 1,
            requested_start_ms: start_ms,
            requested_duration_ms: duration_ms,
            processed_duration_ms: duration_ms,
            media_duration_ms: 1_000,
            state: "complete".into(),
            coverage: "partial".into(),
            detail: "Synthetic original-bound no-recognized-speech result.".into(),
            segments: if candidate {
                Vec::new()
            } else {
                vec![TranscriptSegment {
                    segment_id: String::new(),
                    start_ms,
                    end_ms: start_ms + duration_ms,
                    text: "old recording words".into(),
                    confidence: None,
                    alternatives: Vec::new(),
                    speaker: None,
                    speaker_state: "unidentified".into(),
                    final_result: true,
                }]
            },
        })
    }
}

struct FailingAudio;
impl AudioProcessor for FailingAudio {
    fn inspect(&self, _path: &Path) -> Result<AudioInfo, String> {
        Ok(AudioInfo {
            format: "wav".into(),
            duration_ms: 1_000,
            sample_rate: 48_000.0,
            channels: 1,
            codec: "test audio fixture".into(),
        })
    }
    fn install_assets(&self) -> Result<(), String> {
        Ok(())
    }
    fn transcribe_segment(
        &self,
        _path: &Path,
        _start_ms: u64,
        _duration_ms: u64,
    ) -> Result<TranscriptionBatch, String> {
        Err("deterministic fixture failure".into())
    }
}

struct InterruptedAudio;
impl AudioProcessor for InterruptedAudio {
    fn inspect(&self, _path: &Path) -> Result<AudioInfo, String> {
        Ok(AudioInfo {
            format: "wav".into(),
            duration_ms: 1_000,
            sample_rate: 48_000.0,
            channels: 1,
            codec: "test audio fixture".into(),
        })
    }
    fn install_assets(&self) -> Result<(), String> {
        Ok(())
    }
    fn transcribe_segment(
        &self,
        _path: &Path,
        _start_ms: u64,
        _duration_ms: u64,
    ) -> Result<TranscriptionBatch, String> {
        panic!("fixture process interruption")
    }
}

impl AudioProcessor for VersionedAudio {
    fn inspect(&self, path: &Path) -> Result<AudioInfo, String> {
        let bytes = fs::read(path).map_err(|error| error.to_string())?;
        let duration_ms = if bytes == b"new waveform" {
            2_000
        } else {
            1_000
        };
        Ok(AudioInfo {
            format: "wav".into(),
            duration_ms,
            sample_rate: 48_000.0,
            channels: 1,
            codec: "test audio fixture".into(),
        })
    }

    fn install_assets(&self) -> Result<(), String> {
        Ok(())
    }

    fn transcribe_segment(
        &self,
        path: &Path,
        start_ms: u64,
        duration_ms: u64,
    ) -> Result<TranscriptionBatch, String> {
        let bytes = fs::read(path).map_err(|error| error.to_string())?;
        self.transcribed_assets.lock().unwrap().push(bytes.clone());
        let media_duration_ms = if bytes == b"new waveform" {
            2_000
        } else {
            1_000
        };
        Ok(TranscriptionBatch {
            schema: 1,
            requested_start_ms: start_ms,
            requested_duration_ms: duration_ms,
            processed_duration_ms: duration_ms,
            media_duration_ms,
            state: "complete".into(),
            coverage: "partial".into(),
            detail: "Synthetic version-binding fixture.".into(),
            segments: vec![TranscriptSegment {
                segment_id: String::new(),
                start_ms,
                end_ms: start_ms + duration_ms,
                text: if bytes == b"new waveform" {
                    "new recording words".into()
                } else {
                    "old recording words".into()
                },
                confidence: None,
                alternatives: Vec::new(),
                speaker: None,
                speaker_state: "unidentified".into(),
                final_result: true,
            }],
        })
    }
}

fn app(root: &Path) -> Application {
    Application::open_with_all_providers(
        root,
        Arc::new(EmptyProvider),
        Arc::new(LocalExtractor),
        Arc::new(FixedAudio),
    )
    .unwrap()
}

#[test]
fn imports_original_then_processes_bounded_resumable_audio_with_stable_passages() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("visit.m4a");
    let bytes = b"synthetic retained audio asset";
    fs::write(&source, bytes).unwrap();
    let collection = workspace.path().join("collection");
    let mut collection_app = app(&collection);

    let imported = collection_app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    assert!(imported.info.audio_processing.as_ref().unwrap().state == "pending");
    assert!(imported.body.contains("Play retained original"));
    assert!(imported.body.contains("Speaker identity is unavailable"));
    assert_eq!(
        fs::read(
            collection_app
                .original_path(&imported.info.source_id)
                .unwrap()
        )
        .unwrap(),
        bytes
    );

    collection_app.resume_due_audio_jobs().unwrap();
    let first = collection_app
        .open_source(&imported.info.source_id)
        .unwrap();
    assert_eq!(
        first.info.audio_processing.as_ref().unwrap().next_start_ms,
        30_000
    );
    assert_eq!(
        first.info.audio_processing.as_ref().unwrap().segments.len(),
        1
    );
    let stable_first_id = first.info.audio_processing.as_ref().unwrap().segments[0]
        .segment_id
        .clone();

    // Model a process loss after the segment claim was persisted but before a result was
    // accepted. Startup must retry the same window; the segment ID and knowledge stay stable.
    let path = collection_app.page_path(&imported.info.source_id).unwrap();
    let markdown = fs::read_to_string(&path).unwrap();
    assert!(markdown.contains("state: pending"));
    fs::write(
        &path,
        markdown.replace("state: pending", "state: processing"),
    )
    .unwrap();
    drop(collection_app);
    let mut restarted = app(&collection);
    let recovered = restarted.open_source(&imported.info.source_id).unwrap();
    assert_eq!(
        recovered.info.audio_processing.as_ref().unwrap().state,
        "pending"
    );
    assert_eq!(
        recovered
            .info
            .audio_processing
            .as_ref()
            .unwrap()
            .next_start_ms,
        30_000
    );
    restarted.resume_due_audio_jobs().unwrap();
    let repeated = restarted.open_source(&imported.info.source_id).unwrap();
    assert_eq!(
        repeated
            .info
            .audio_processing
            .as_ref()
            .unwrap()
            .segments
            .len(),
        2
    );
    assert_eq!(
        repeated.info.audio_processing.as_ref().unwrap().segments[0].segment_id,
        stable_first_id
    );

    restarted.resume_due_audio_jobs().unwrap();
    let second = restarted.open_source(&imported.info.source_id).unwrap();
    let audio = second.info.audio_processing.as_ref().unwrap();
    assert_eq!(audio.state, "complete", "{}", audio.detail);
    assert_eq!(audio.segments.len(), 3);
    assert!(audio
        .segments
        .iter()
        .all(|segment| segment.speaker.is_none()));
    assert!(second.body.contains("confidence 0.51"));
    assert!(second.body.contains("audio_seek=source-"));
    let jobs = restarted.claim_due_semantic_jobs(1).unwrap();
    assert_eq!(jobs.len(), 1);
    let job = jobs.into_iter().next().unwrap();
    assert!(job
        .source_text
        .contains("[AUDIO 1000–2000 ms confidence=0.51 speaker=unidentified]"));
    let quote = "The first fixture phrase.";
    let offset = job.source_text.find(quote).unwrap();
    let draft = KnowledgeDraft {
        entities: vec![EntityDraft {
            kind: "document".into(),
            label: "Fixture recording".into(),
            evidence: EvidenceDraft {
                quote: job.source_text.clone(),
                byte_start: 0,
                byte_end: job.source_text.len(),
                origin: "observed".into(),
                qualifier: None,
                offset_basis: None,
                source_location: None,
            },
        }],
        facts: vec![FactDraft {
            subject: "Fixture recording".into(),
            property: "spoken passage".into(),
            value: quote.into(),
            evidence: EvidenceDraft {
                quote: quote.into(),
                byte_start: offset,
                byte_end: offset + quote.len(),
                origin: "transcribed speech".into(),
                qualifier: None,
                offset_basis: None,
                source_location: None,
            },
            record_key: None,
        }],
        ..Default::default()
    };
    restarted.finish_semantic_job(job, Ok(draft)).unwrap();
    let page_after_knowledge = restarted.open_source(&imported.info.source_id).unwrap();
    assert_eq!(page_after_knowledge.info.semantic_state, "complete");
    let knowledge_page = restarted
        .open_knowledge_page(&page_after_knowledge.info.knowledge_pages[0].page_id)
        .unwrap();
    assert!(knowledge_page
        .markdown
        .contains("Machine transcription; wording and speaker identity are unverified."));
    assert!(knowledge_page.markdown.contains("AUDIO 1000–2000 ms"));
    assert!(knowledge_page
        .markdown
        .contains("Open audio at this timestamp"));
    assert!(knowledge_page
        .markdown
        .contains("cannot guarantee precise seeking"));
    let found = restarted
        .search_pages(PageSearchRequest {
            query: "later fixture phrase".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(found
        .pages
        .iter()
        .any(|page| page.source_id == imported.info.source_id));
    assert_eq!(
        fs::read(restarted.original_path(&imported.info.source_id).unwrap()).unwrap(),
        bytes
    );
}

#[test]
fn replacement_recording_resets_transcript_and_transcribes_the_pending_original_version() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("visit.wav");
    let old_bytes = b"old waveform";
    let new_bytes = b"new waveform";
    fs::write(&source, old_bytes).unwrap();
    let collection = workspace.path().join("collection");
    let transcribed_assets = Arc::new(Mutex::new(Vec::new()));
    let processor = Arc::new(VersionedAudio {
        transcribed_assets: Arc::clone(&transcribed_assets),
    });
    let mut app = Application::open_with_all_providers(
        &collection,
        Arc::new(VersionedAudioProvider),
        Arc::new(LocalExtractor),
        processor,
    )
    .unwrap();

    let first = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_audio_jobs().unwrap();
    let first_job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    let first_draft = VersionedAudioProvider.form_knowledge(&first_job.source_text);
    app.finish_semantic_job(first_job, first_draft).unwrap();
    assert_eq!(
        app.open_source(&first.info.source_id)
            .unwrap()
            .info
            .current_version_id
            .as_deref(),
        Some(first.info.sha256.as_str())
    );
    assert_eq!(
        app.open_source(&first.info.source_id)
            .unwrap()
            .info
            .audio_processing
            .as_ref()
            .unwrap()
            .state,
        "complete"
    );

    fs::write(&source, new_bytes).unwrap();
    let replacement = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(replacement.info.source_id, first.info.source_id);
    let replacement_version_id = format!("{:x}", sha2::Sha256::digest(new_bytes));
    let prior_audio = replacement.info.audio_processing.as_ref().unwrap();
    assert_eq!(prior_audio.state, "complete");
    assert_eq!(
        prior_audio.source_version_id.as_deref(),
        Some(first.info.sha256.as_str())
    );
    assert!(prior_audio.segments[0].text.contains("old recording words"));
    let audio = replacement.info.pending_audio_processing.as_ref().unwrap();
    assert_eq!(audio.state, "pending");
    assert_eq!(audio.duration_ms, 2_000);
    assert_eq!(audio.next_start_ms, 0);
    assert!(audio.segments.is_empty());
    assert_eq!(
        audio.source_version_id.as_deref(),
        Some(replacement_version_id.as_str())
    );
    assert_eq!(
        replacement.info.pending_version_id.as_deref(),
        Some(replacement_version_id.as_str())
    );
    assert_eq!(
        replacement.info.current_version_id.as_deref(),
        Some(first.info.sha256.as_str())
    );
    assert_eq!(
        fs::read(app.original_path(&first.info.source_id).unwrap()).unwrap(),
        old_bytes
    );
    let replacement_version = replacement
        .info
        .versions_seen
        .iter()
        .find(|version| version.source_version_id == replacement_version_id)
        .unwrap();
    assert_eq!(
        fs::read(
            app.original_version_path(
                &replacement.info.source_id,
                &replacement_version_id,
                &replacement_version.asset
            )
            .unwrap()
        )
        .unwrap(),
        new_bytes
    );

    app.resume_due_audio_jobs().unwrap();
    assert_eq!(
        transcribed_assets.lock().unwrap().as_slice(),
        &[old_bytes.to_vec(), new_bytes.to_vec()]
    );
    let after_transcription = app.open_source(&replacement.info.source_id).unwrap();
    assert_eq!(after_transcription.info.asset, first.info.asset);
    assert_eq!(replacement_version.asset, first.info.asset);
    assert!(after_transcription.body.contains(&format!(
        "[Play retained original]({})",
        after_transcription.info.asset
    )));
    assert!(after_transcription.body.contains("old recording words"));
    assert!(after_transcription
        .body
        .contains("[Open this retained replacement original](../sources/"));
    assert!(after_transcription.body.contains(&format!(
        "/versions/{replacement_version_id}/{})",
        replacement_version.asset
    )));
    assert!(after_transcription
        .body
        .contains("Candidate transcript · not published"));
    assert!(after_transcription.body.contains("new recording words"));
    assert_eq!(
        after_transcription
            .info
            .audio_processing
            .as_ref()
            .unwrap()
            .segments[0]
            .text,
        "old recording words"
    );
    assert_eq!(
        after_transcription
            .info
            .pending_audio_processing
            .as_ref()
            .unwrap()
            .segments[0]
            .text,
        "new recording words"
    );
    assert_eq!(
        after_transcription.info.current_version_id.as_deref(),
        Some(first.info.sha256.as_str()),
        "the old recording remains the current version until replacement semantics publish"
    );
    let new_job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    assert_eq!(new_job.source_version_id, replacement_version_id);
    assert!(new_job.source_text.contains("new recording words"));
    let replacement_draft = VersionedAudioProvider.form_knowledge(&new_job.source_text);
    app.finish_semantic_job(new_job, replacement_draft).unwrap();
    let published = app.open_source(&replacement.info.source_id).unwrap();
    assert_eq!(
        published.info.current_version_id.as_deref(),
        Some(replacement_version_id.as_str())
    );
    assert_eq!(
        published
            .info
            .audio_processing
            .as_ref()
            .unwrap()
            .source_version_id
            .as_deref(),
        Some(replacement_version_id.as_str())
    );
    assert!(published.info.pending_audio_processing.is_none());
    assert_eq!(
        fs::read(app.original_path(&replacement.info.source_id).unwrap()).unwrap(),
        new_bytes
    );
    assert_eq!(
        fs::read(
            app.original_version_path(
                &replacement.info.source_id,
                &first.info.sha256,
                "original.wav"
            )
            .unwrap()
        )
        .unwrap(),
        old_bytes
    );
}

#[test]
fn zero_recognized_replacement_stays_unpublished_with_the_prior_original_current() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("visit.wav");
    let original = b"spoken original";
    let candidate_bytes = b"silent replacement";
    fs::write(&source, original).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open_with_all_providers(
        &collection,
        Arc::new(VersionedAudioProvider),
        Arc::new(LocalExtractor),
        Arc::new(SilentReplacementAudio),
    )
    .unwrap();
    let first = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_audio_jobs().unwrap();
    let first_job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    let first_draft = VersionedAudioProvider.form_knowledge(&first_job.source_text);
    app.finish_semantic_job(first_job, first_draft).unwrap();
    let current = app.open_source(&first.info.source_id).unwrap();
    assert_eq!(
        current.info.current_version_id.as_deref(),
        Some(first.info.sha256.as_str())
    );
    assert!(current.body.contains("old recording words"));

    fs::write(&source, candidate_bytes).unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    let candidate_id = imported.info.pending_version_id.clone().unwrap();
    app.resume_due_audio_jobs().unwrap();

    let after = app.open_source(&first.info.source_id).unwrap();
    assert_eq!(after.info.asset, first.info.asset);
    let candidate_version = after
        .info
        .versions_seen
        .iter()
        .find(|version| version.source_version_id == candidate_id)
        .unwrap();
    assert_eq!(
        after.info.current_version_id.as_deref(),
        Some(first.info.sha256.as_str())
    );
    assert_eq!(
        after.info.pending_version_id.as_deref(),
        Some(candidate_id.as_str())
    );
    assert_eq!(
        after
            .info
            .audio_processing
            .as_ref()
            .unwrap()
            .source_version_id
            .as_deref(),
        Some(first.info.sha256.as_str())
    );
    let candidate = after.info.pending_audio_processing.as_ref().unwrap();
    assert_eq!(
        candidate.source_version_id.as_deref(),
        Some(candidate_id.as_str())
    );
    assert_eq!(candidate.state, "complete");
    assert!(candidate.segments.is_empty());
    assert!(after.body.contains("old recording words"));
    assert!(after
        .body
        .contains(&format!("[Play retained original]({})", after.info.asset)));
    assert!(after
        .body
        .contains("[Open this retained replacement original](../sources/"));
    assert!(after.body.contains(&format!(
        "/versions/{candidate_id}/{})",
        candidate_version.asset
    )));
    assert!(after
        .body
        .contains("previously published transcript and current original remain available"));
    assert_eq!(
        fs::read(app.original_path(&first.info.source_id).unwrap()).unwrap(),
        original
    );
    assert_eq!(
        fs::read(
            app.original_version_path(
                &first.info.source_id,
                &candidate_id,
                &candidate_version.asset
            )
            .unwrap()
        )
        .unwrap(),
        candidate_bytes
    );
    assert!(
        app.claim_due_semantic_jobs(1).unwrap().is_empty(),
        "no transcript means no semantic evidence to publish"
    );
}

#[test]
fn failed_audio_attempts_stop_at_a_durable_per_version_limit_and_new_version_resets_it() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("visit.wav");
    fs::write(&source, b"first recording").unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open_with_all_providers(
        &collection,
        Arc::new(EmptyProvider),
        Arc::new(LocalExtractor),
        Arc::new(FailingAudio),
    )
    .unwrap();
    let first = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();

    for expected_attempt in 1..=3 {
        if expected_attempt > 1 {
            let current = app.open_source(&first.info.source_id).unwrap();
            let retry_at = current
                .info
                .audio_processing
                .as_ref()
                .unwrap()
                .retry_at_ms
                .unwrap();
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            thread::sleep(Duration::from_millis(retry_at.saturating_sub(now)));
        }
        app.resume_due_audio_jobs().unwrap();
        let current = app.open_source(&first.info.source_id).unwrap();
        let audio = current.info.audio_processing.as_ref().unwrap();
        assert_eq!(audio.attempts, expected_attempt);
        assert_eq!(audio.failed_attempts, expected_attempt);
        assert_eq!(
            audio.source_version_id.as_deref(),
            Some(first.info.sha256.as_str())
        );
        assert_eq!(
            audio.state,
            if expected_attempt == 3 {
                "failed"
            } else {
                "pending"
            }
        );
    }
    app.resume_due_audio_jobs().unwrap();
    let exhausted = app.open_source(&first.info.source_id).unwrap();
    let exhausted_audio = exhausted.info.audio_processing.as_ref().unwrap();
    assert_eq!(
        exhausted_audio.attempts, 3,
        "exhausted work is not repeated"
    );
    assert_eq!(exhausted_audio.retry_at_ms, None);
    assert_eq!(
        fs::read(app.original_path(&first.info.source_id).unwrap()).unwrap(),
        b"first recording",
        "the exact original stays playable after retries are exhausted"
    );

    fs::write(&source, b"second recording").unwrap();
    let replacement = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    let new_audio = replacement.info.audio_processing.as_ref().unwrap();
    assert_ne!(new_audio.source_version_id, Some(first.info.sha256.clone()));
    assert_eq!(new_audio.attempts, 0);
    assert_eq!(new_audio.failed_attempts, 0);
    assert_eq!(new_audio.state, "pending");
    assert_eq!(
        fs::read(app.original_path(&first.info.source_id).unwrap()).unwrap(),
        b"first recording"
    );
}

#[test]
fn interrupted_audio_segments_stop_after_a_durable_per_version_restart_budget() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("visit.wav");
    let original = b"interrupted recording";
    fs::write(&source, original).unwrap();
    let collection = workspace.path().join("collection");
    let first = {
        let mut app = Application::open_with_all_providers(
            &collection,
            Arc::new(EmptyProvider),
            Arc::new(LocalExtractor),
            Arc::new(InterruptedAudio),
        )
        .unwrap();
        app.import_source(&source, AcquisitionMethod::Picker)
            .unwrap()
    };

    for expected_interruption in 1..=3 {
        let mut app = Application::open_with_all_providers(
            &collection,
            Arc::new(EmptyProvider),
            Arc::new(LocalExtractor),
            Arc::new(InterruptedAudio),
        )
        .unwrap();
        let interrupted = catch_unwind(AssertUnwindSafe(|| app.resume_due_audio_jobs()));
        assert!(
            interrupted.is_err(),
            "fixture simulates an interrupted segment"
        );
        drop(app);

        let reopened = Application::open_with_all_providers(
            &collection,
            Arc::new(EmptyProvider),
            Arc::new(LocalExtractor),
            Arc::new(InterruptedAudio),
        )
        .unwrap();
        let page = reopened.open_source(&first.info.source_id).unwrap();
        let audio = page.info.audio_processing.as_ref().unwrap();
        assert_eq!(audio.attempts, expected_interruption);
        assert_eq!(audio.processing_interruptions, expected_interruption);
        assert_eq!(
            audio.state,
            if expected_interruption == 3 {
                "failed"
            } else {
                "pending"
            }
        );
        assert_eq!(
            fs::read(reopened.original_path(&first.info.source_id).unwrap()).unwrap(),
            original
        );
    }
}

struct AudioCorrectionProvider;
impl SemanticProvider for AudioCorrectionProvider {
    fn form_knowledge(&self, text: &str) -> Result<KnowledgeDraft, ProviderError> {
        let quote = "I may have said 12, but actually the count could be 15.";
        let Some(start) = text.find(quote) else {
            return Ok(KnowledgeDraft::default());
        };
        let entity_quote = "The field report says 12 visits on May 17th, for about 10 minutes.";
        let Some(entity_start) = text.find(entity_quote) else {
            return Ok(KnowledgeDraft::default());
        };
        Ok(KnowledgeDraft {
            entities: vec![EntityDraft {
                kind: "document".into(),
                label: "field report".into(),
                evidence: EvidenceDraft {
                    quote: entity_quote.into(),
                    byte_start: entity_start,
                    byte_end: entity_start + entity_quote.len(),
                    origin: "transcribed speech".into(),
                    qualifier: None,
                    offset_basis: None,
                    source_location: None,
                },
            }],
            facts: vec![FactDraft {
                subject: "field report".into(),
                property: "possible visit count correction".into(),
                value: "could be 15".into(),
                evidence: EvidenceDraft {
                    quote: quote.into(),
                    byte_start: start,
                    byte_end: start + quote.len(),
                    origin: "transcribed speech".into(),
                    qualifier: Some(
                        "The speaker states an uncertain correction; compare the original audio."
                            .into(),
                    ),
                    offset_basis: None,
                    source_location: None,
                },
                record_key: None,
            }],
            ..Default::default()
        })
    }
}

fn whisper_app(root: &Path, assets: &Path) -> Application {
    Application::open_with_all_providers(
        root,
        Arc::new(AudioCorrectionProvider),
        Arc::new(LocalExtractor),
        Arc::new(WhisperAudioProcessor::new(assets)),
    )
    .unwrap()
}

#[test]
fn pinned_local_whisper_import_retrieval_evidence_restart_and_seek_fallback() {
    let _asr_lock = LOCAL_ASR_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let workspace = tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/frozen-two-speaker-uncertain-correction.wav");
    let original = fs::read(&source).unwrap();
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/audio");
    let collection = workspace.path().join("collection");
    let mut app = whisper_app(&collection, &assets);

    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(
        fs::read(app.original_path(&imported.info.source_id).unwrap()).unwrap(),
        original
    );
    let page_path = app.page_path(&imported.info.source_id).unwrap();
    let markdown = fs::read_to_string(&page_path).unwrap();
    assert!(markdown.contains("state: pending"));
    fs::write(
        &page_path,
        markdown.replacen("state: pending", "state: processing", 1),
    )
    .unwrap();
    drop(app);
    let mut app = whisper_app(&collection, &assets);
    let recovered = app.open_source(&imported.info.source_id).unwrap();
    assert_eq!(
        recovered.info.audio_processing.as_ref().unwrap().state,
        "pending"
    );
    assert_eq!(
        recovered
            .info
            .audio_processing
            .as_ref()
            .unwrap()
            .next_start_ms,
        0
    );
    app.resume_due_audio_jobs().unwrap();
    let transcribed = app.open_source(&imported.info.source_id).unwrap();
    let audio = transcribed.info.audio_processing.as_ref().unwrap();
    assert_eq!(audio.state, "complete");
    assert_eq!(audio.segments.len(), 2);
    let segments = audio.segments.clone();
    assert!(segments.iter().all(|segment| segment.speaker.is_none()));
    assert!(segments.iter().all(|segment| segment.confidence.is_none()));
    assert!(segments[0].text.contains("12 visits on May 17th"));
    assert!(segments[0].text.contains("10 minutes"));
    assert!(segments[1].text.contains("may have said 12"));
    assert!(segments[1].text.contains("count could be 15"));
    assert!(segments[1].start_ms.abs_diff(5_440) <= 250);
    assert!(segments[1].end_ms.abs_diff(11_393) <= 250);
    assert!(segments[0].end_ms < segments[1].start_ms);
    assert!(transcribed.body.contains(&format!(
        "No recognized speech · {}–{} ms",
        segments[0].end_ms, segments[1].start_ms
    )));
    let audio_locator = format!("AUDIO {}–{} ms", segments[1].start_ms, segments[1].end_ms);
    assert!(transcribed.body.contains("timestamps are estimates"));
    assert!(transcribed.body.contains("speakers are left unidentified"));

    let jobs = app.claim_due_semantic_jobs(1).unwrap();
    assert_eq!(jobs.len(), 1);
    let job = jobs.into_iter().next().unwrap();
    let result = AudioCorrectionProvider.form_knowledge(&job.source_text);
    app.finish_semantic_job(job, result).unwrap();
    let with_knowledge = app.open_source(&imported.info.source_id).unwrap();
    assert_eq!(
        with_knowledge.info.semantic_state, "complete",
        "semantic error: {:?}",
        with_knowledge.info.semantic_error
    );
    let fact_page = app
        .open_knowledge_page(&with_knowledge.info.knowledge_pages[0].page_id)
        .unwrap();
    assert!(fact_page.markdown.contains("could be 15"));
    assert!(fact_page.markdown.contains(&audio_locator));
    assert!(fact_page.markdown.contains("Open audio at this timestamp"));
    assert!(fact_page
        .markdown
        .contains("cannot guarantee precise seeking"));
    assert!(fact_page
        .markdown
        .contains("Machine transcription; wording and speaker identity are unverified."));
    let results = app
        .search_pages(PageSearchRequest {
            query: "count could be 15".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(results
        .pages
        .iter()
        .any(|page| page.page_id == fact_page.page_id));

    let stable_ids: Vec<_> = segments
        .iter()
        .map(|segment| segment.segment_id.clone())
        .collect();
    drop(app);
    let restarted = whisper_app(&collection, &assets);
    let after_restart = restarted.open_source(&imported.info.source_id).unwrap();
    assert_eq!(
        after_restart
            .info
            .audio_processing
            .as_ref()
            .unwrap()
            .segments
            .iter()
            .map(|segment| segment.segment_id.clone())
            .collect::<Vec<_>>(),
        stable_ids
    );
    assert_eq!(after_restart.info.knowledge_pages.len(), 1);
    assert_eq!(
        fs::read(restarted.original_path(&imported.info.source_id).unwrap()).unwrap(),
        original
    );
}

#[test]
fn pinned_local_whisper_accepts_aac_m4a_through_application_import() {
    let _asr_lock = LOCAL_ASR_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let workspace = tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/frozen-two-speaker-uncertain-correction.m4a");
    let original = fs::read(&source).unwrap();
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/audio");
    let collection = workspace.path().join("collection");
    let mut app = whisper_app(&collection, &assets);
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(
        fs::read(app.original_path(&imported.info.source_id).unwrap()).unwrap(),
        original
    );
    app.resume_due_audio_jobs().unwrap();
    let transcribed = app.open_source(&imported.info.source_id).unwrap();
    let audio = transcribed.info.audio_processing.as_ref().unwrap();
    assert_eq!(audio.state, "complete", "{}", audio.detail);
    assert_eq!(audio.duration_ms, 11_393);
    assert_eq!(audio.segments.len(), 2);
    assert!(audio.segments[0].text.contains("12 visits on May 17th"));
    assert!(audio.segments[1].text.contains("count could be 15"));
    let found = app
        .search_pages(PageSearchRequest {
            query: "May 17th visits".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(found
        .pages
        .iter()
        .any(|page| page.source_id == imported.info.source_id));
}

#[test]
fn missing_bundled_whisper_model_keeps_audio_pending_and_original_available() {
    let workspace = tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/frozen-two-speaker-uncertain-correction.wav");
    let original = fs::read(&source).unwrap();
    let collection = workspace.path().join("collection");
    let missing_assets = workspace.path().join("missing-assets");
    let mut app = whisper_app(&collection, &missing_assets);
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_audio_jobs().unwrap();
    let pending = app.open_source(&imported.info.source_id).unwrap();
    let audio = pending.info.audio_processing.as_ref().unwrap();
    assert_eq!(audio.state, "pending");
    assert!(audio
        .detail
        .contains("bundled large-v3-turbo model is unavailable"));
    assert!(pending
        .body
        .contains("No transcript passages are available yet"));
    assert_eq!(
        fs::read(app.original_path(&imported.info.source_id).unwrap()).unwrap(),
        original
    );
}
