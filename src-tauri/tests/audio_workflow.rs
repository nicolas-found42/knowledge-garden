use knowledge_garden::{
    application::{AcquisitionMethod, Application, PageSearchRequest},
    extraction::LocalExtractor,
    media::{AudioInfo, AudioProcessor, TranscriptSegment, TranscriptionBatch},
    semantic::{
        EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft, ProviderError, SemanticProvider,
    },
};
use std::{fs, path::Path, sync::Arc};
use tempfile::tempdir;

struct EmptyProvider;
impl SemanticProvider for EmptyProvider {
    fn form_knowledge(&self, _text: &str) -> Result<KnowledgeDraft, ProviderError> {
        Ok(KnowledgeDraft::default())
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
    assert_eq!(audio.state, "complete");
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
