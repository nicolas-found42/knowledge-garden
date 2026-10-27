use knowledge_garden::{
    application::{
        trails::{TrailContext, TrailStop},
        views::{ViewDefinition, ViewTarget},
        AcquisitionMethod, Application,
    },
    extraction::LocalExtractor,
    media::{AudioInfo, AudioProcessor, TranscriptSegment, TranscriptionBatch},
    providers::{JevSemanticProvider, SystemOneTransport},
    semantic::{
        KnowledgeDraft, NavigationCandidate, NavigationJudgments, ProviderError, SemanticProvider,
    },
};
use serde_json::{json, Value};
use std::{fs, path::Path, sync::Arc};
use uuid::Uuid;
#[path = "fixtures/queries/provider.rs"]
mod labeled;
struct NavigationTransport;
impl SystemOneTransport for NavigationTransport {
    fn complete(&self, _key: &str, request: &Value) -> std::result::Result<Value, ProviderError> {
        assert_eq!(request["model"], "typesafe/jev-1.14");
        let state: Value =
            serde_json::from_str(request["state"]["source_text"].as_str().unwrap()).unwrap();
        assert_eq!(state["reading_goal"], "Riverside");
        let mut answers = serde_json::Map::new();
        for (key, question) in request["questions"].as_object().unwrap() {
            assert_eq!(question["type"], "noul");
            let index: usize = key.strip_prefix("stop_").unwrap().parse().unwrap();
            let useful = state["candidates"][index]["label"].as_str().unwrap() == "Riverside";
            answers.insert(
                key.clone(),
                json!({"type":"noul","noul":if useful{0.96}else{0.08}}),
            );
        }
        Ok(json!({"model":"typesafe/jev-1.14-recorded-trail-fixture","answers":answers}))
    }
}
struct FixtureProvider;
impl SemanticProvider for FixtureProvider {
    fn form_knowledge(&self, text: &str) -> std::result::Result<KnowledgeDraft, ProviderError> {
        if text.contains("V17 occurred at Riverside with location uncertainty") {
            labeled::LabeledGraph.form_knowledge(text)
        } else {
            Ok(KnowledgeDraft::default())
        }
    }
    fn judge_navigation(
        &self,
        goal: &str,
        candidates: &[NavigationCandidate],
    ) -> std::result::Result<NavigationJudgments, ProviderError> {
        JevSemanticProvider::with_transport("fixture".into(), Arc::new(NavigationTransport))
            .judge_navigation(goal, candidates)
    }
}
struct FixedAudio;
impl AudioProcessor for FixedAudio {
    fn install_assets(&self) -> std::result::Result<(), String> {
        Ok(())
    }
    fn inspect(&self, _: &Path) -> std::result::Result<AudioInfo, String> {
        Ok(AudioInfo {
            format: "wav".into(),
            duration_ms: 2000,
            sample_rate: 16000.,
            channels: 1,
            codec: "fixture PCM".into(),
        })
    }
    fn transcribe_segment(
        &self,
        _: &Path,
        start: u64,
        duration: u64,
    ) -> std::result::Result<TranscriptionBatch, String> {
        Ok(TranscriptionBatch {
            schema: 1,
            requested_start_ms: start,
            requested_duration_ms: duration,
            processed_duration_ms: 2000,
            media_duration_ms: 2000,
            state: "complete".into(),
            coverage: "Frozen integrity fixture; not ASR quality evidence".into(),
            detail: "Fixed transcript for navigation integrity.".into(),
            segments: vec![TranscriptSegment {
                segment_id: String::new(),
                start_ms: 500,
                end_ms: 1500,
                text: "Riverside field recording.".into(),
                confidence: None,
                alternatives: vec![],
                speaker: None,
                speaker_state: "unknown".into(),
                final_result: true,
            }],
        })
    }
}
fn page_stop(id: &str, title: &str) -> TrailStop {
    TrailStop {
        stop_id: Uuid::new_v4().to_string(),
        label: title.into(),
        target: ViewTarget {
            source_id: None,
            page_id: Some(id.into()),
            title: Some(title.into()),
        },
        context: None,
    }
}
fn definition(stops: Vec<TrailStop>) -> ViewDefinition {
    serde_json::from_value(json!({"active":"trail","trail":stops,"search":null,"query":null,"collection":null,"timeline":null,"paths":null,"comparison":[],"target":null})).unwrap()
}
#[test]
fn mixed_route_preserves_order_context_and_honest_resolution_after_restart_and_rebuild() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let mut app = Application::open_with_all_providers(
        &root,
        Arc::new(FixtureProvider),
        Arc::new(LocalExtractor),
        Arc::new(FixedAudio),
    )
    .unwrap();
    let input = temp.path().join("fieldwork.txt");
    fs::write(&input, labeled::TEXT).unwrap();
    let source = app.import_source(input, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&source.info.source_id).unwrap();
    let page = source
        .knowledge_pages
        .iter()
        .find(|p| p.title == "Riverside")
        .unwrap();
    let event = source
        .knowledge_pages
        .iter()
        .find(|p| p.title == "V17")
        .unwrap();
    let passage = app
        .trail_selected_passage(
            &source.info.source_id,
            "V17 occurred at Riverside with location uncertainty",
        )
        .unwrap();
    let conversation = temp.path().join("conversation.json");
    fs::write(&conversation,serde_json::to_vec(&json!({"format":"knowledge-garden-conversation-v1","conversation_id":"trail-test","messages":[{"id":"choice/1","role":"user","content":"Decision: inspect Riverside."},{"id":"reason","role":"reasoning","content":"The water level is uncertain."}]})).unwrap()).unwrap();
    let conversation = app
        .import_source(conversation, AcquisitionMethod::Picker)
        .unwrap();
    let messages = app
        .trail_source_choices(&conversation.info.source_id, 0)
        .unwrap();
    assert_eq!(messages.stops.len(), 2);
    assert!(
        matches!(&messages.stops[0].context,Some(TrailContext::Message{message_id,..}) if message_id=="choice/1")
    );
    let recording = temp.path().join("recording.wav");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/heldout-spoken-acts.wav"),
        &recording,
    )
    .unwrap();
    let recording = app
        .import_source(recording, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_audio_jobs().unwrap();
    let media = app
        .trail_source_choices(&recording.info.source_id, 0)
        .unwrap();
    assert_eq!(media.stops.len(), 1);
    assert!(matches!(
        media.stops[0].context,
        Some(TrailContext::Media { at_ms: 500, .. })
    ));
    let mut stops = vec![
        page_stop(&page.page_id, "Riverside"),
        page_stop(&event.page_id, "V17"),
        passage,
        messages.stops[0].clone(),
        media.stops[0].clone(),
    ];
    let resolved = app.resolve_trail(stops.clone()).unwrap();
    assert!(resolved.iter().all(|r| r.status == "available"));
    assert!(resolved[3]
        .quote
        .as_ref()
        .unwrap()
        .contains("Decision: inspect Riverside"));
    assert!(resolved[4].locator.as_ref().unwrap().contains("500 ms"));
    assert!(app.list_saved_views("", 0).unwrap().views.is_empty());
    stops.swap(1, 2);
    stops.remove(2);
    let expected = serde_json::to_value(&stops).unwrap();
    let saved = app
        .create_saved_view("River study route", definition(stops.clone()))
        .unwrap();
    let bytes =
        fs::read_to_string(root.join(&app.list_saved_views("", 0).unwrap().views[0].path)).unwrap();
    assert!(
        bytes.contains("## Ordered trail")
            && bytes.contains("Message choice\\/1")
            && bytes.contains("Media 500 ms"),
        "{bytes}"
    );
    // External page move/rename preserves the target identity; a removed conversation stays unavailable.
    let original = root.join(&page.path);
    let moved = root.join("pages/owner-river-name.md");
    let text = fs::read_to_string(&original).unwrap();
    fs::write(
        &moved,
        text.replacen("title: Riverside", "title: River renamed by owner", 1),
    )
    .unwrap();
    fs::remove_file(original).unwrap();
    fs::remove_dir_all(
        app.page_path(&conversation.info.source_id)
            .unwrap()
            .parent()
            .unwrap(),
    )
    .unwrap();
    drop(app);
    // A cold rebuild must not rely on any previous database or vector cache.
    fs::remove_dir_all(root.join(".derived")).unwrap();
    let mut app = Application::open(&root).unwrap();
    app.rebuild_index().unwrap();
    let reopened = app.open_saved_view(&saved.view_id).unwrap();
    assert_eq!(
        serde_json::to_value(&reopened.definition.trail).unwrap(),
        expected
    );
    let resolved = app.resolve_trail(reopened.definition.trail).unwrap();
    assert_eq!(
        resolved
            .iter()
            .map(|r| r.stop.stop_id.clone())
            .collect::<Vec<_>>(),
        stops.iter().map(|s| s.stop_id.clone()).collect::<Vec<_>>()
    );
    assert_eq!(resolved[0].title, "River renamed by owner");
    assert_eq!(resolved[0].status, "available");
    assert_eq!(resolved[2].status, "unavailable");
    assert_eq!(resolved[3].status, "available");
    assert!(reopened
        .warnings
        .iter()
        .any(|w| w.contains("River renamed by owner")));
}
#[test]
fn edited_passages_and_changed_versions_never_jump_to_similar_or_obsolete_material() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let input = temp.path().join("notes.txt");
    fs::write(&input, "First unique phrase. Repeated. Repeated.").unwrap();
    let mut app = Application::open(&root).unwrap();
    let source = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    assert!(app
        .trail_selected_passage(&source.info.source_id, "Repeated.")
        .is_err());
    let passage = app
        .trail_selected_passage(&source.info.source_id, "First unique phrase.")
        .unwrap();
    fs::write(
        app.original_path(&source.info.source_id).unwrap(),
        "Changed wording; First unique phrase. moved.",
    )
    .unwrap();
    let resolved = app.resolve_trail(vec![passage.clone()]).unwrap();
    assert_eq!(resolved[0].status, "unavailable");
    assert!(resolved[0].quote.is_none());
    let mut changed = passage.clone();
    if let Some(TrailContext::Span {
        source_version_id, ..
    }) = &mut changed.context
    {
        *source_version_id = "a".repeat(64);
    }
    let resolved = app.resolve_trail(vec![changed]).unwrap();
    assert_eq!(resolved[0].status, "changed");
    assert!(resolved[0].quote.is_none());
    let mut obsolete = app.open_source(&source.info.source_id).unwrap();
    obsolete.info.versions_seen[0].state = "superseded".into();
    fs::write(
        app.page_path(&source.info.source_id).unwrap(),
        format!(
            "---\n{}---\n{}",
            serde_yaml_ng::to_string(&obsolete.info).unwrap(),
            obsolete.body
        ),
    )
    .unwrap();
    assert_eq!(
        app.resolve_trail(vec![passage.clone()]).unwrap()[0].status,
        "unavailable"
    );
    assert!(app.trail_source_choices(&source.info.source_id, 0).is_err());
    assert!(app
        .trail_selected_passage(&source.info.source_id, "First unique phrase.")
        .is_err());
    let mut invalid = passage.clone();
    invalid.target.source_id = None;
    invalid.target.page_id = Some("page-river".into());
    assert!(app.resolve_trail(vec![invalid]).is_err());
    assert!(app.resolve_trail(vec![passage.clone(), passage]).is_err());
    assert!(app.list_saved_views("", 0).unwrap().views.is_empty());
}
#[test]
fn jev_selects_only_current_existing_candidates_and_unavailable_provider_does_not_invent_stops() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let input = temp.path().join("fieldwork.txt");
    fs::write(&input, labeled::TEXT).unwrap();
    let mut app =
        Application::open_with_semantic_provider(&root, Arc::new(FixtureProvider)).unwrap();
    let source = app.import_source(input, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let job = app.prepare_trail_suggestions("Riverside").unwrap();
    let evaluated = job.evaluate();
    let suggestions = app.finish_trail_suggestions(evaluated).unwrap();
    assert_eq!(suggestions.stops.len(), 1);
    assert_eq!(suggestions.stops[0].label, "Riverside");
    assert_eq!(
        suggestions.model.as_deref(),
        Some("typesafe/jev-1.14-recorded-trail-fixture")
    );
    assert_eq!(suggestions.judgments, vec![0.96]);
    assert_eq!(
        app.resolve_trail(suggestions.stops).unwrap()[0].status,
        "available"
    );
    let job = app.prepare_trail_suggestions("Riverside").unwrap();
    let edited = app
        .open_source(&source.info.source_id)
        .unwrap()
        .knowledge_pages
        .into_iter()
        .find(|p| p.title == "Riverside")
        .unwrap();
    let edited_path = root.join(&edited.path);
    let original = fs::read_to_string(&edited_path).unwrap();
    let evaluated = job.evaluate();
    fs::write(
        &edited_path,
        format!("{original}\nCurrent page changed during selection.\n"),
    )
    .unwrap();
    assert!(app
        .finish_trail_suggestions(evaluated)
        .unwrap()
        .stops
        .is_empty());
    fs::write(edited_path, original).unwrap();
    let job = app.prepare_trail_suggestions("Riverside").unwrap();
    let evaluated = job.evaluate();
    let page = app
        .open_source(&source.info.source_id)
        .unwrap()
        .knowledge_pages
        .into_iter()
        .find(|p| p.title == "Riverside")
        .unwrap();
    fs::remove_file(root.join(page.path)).unwrap();
    app.rebuild_index().unwrap();
    assert!(app
        .finish_trail_suggestions(evaluated)
        .unwrap()
        .stops
        .is_empty());
    drop(app);
    let mut app = Application::open(root).unwrap();
    let job = app.prepare_trail_suggestions("Riverside").unwrap();
    let suggestions = app.finish_trail_suggestions(job.evaluate()).unwrap();
    assert!(suggestions.stops.is_empty());
    assert!(suggestions.detail.contains("unavailable"));
    assert!(app.list_saved_views("", 0).unwrap().views.is_empty());
}

#[test]
fn decoded_video_frame_stops_preserve_the_visual_channel() {
    let temp = tempfile::tempdir().unwrap();
    let mut app = Application::open(temp.path().join("collection")).unwrap();
    let input = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/video/silent-no-audio-track.mp4");
    let source = app.import_source(input, AcquisitionMethod::Picker).unwrap();
    app.resume_due_audio_jobs().unwrap();
    let choices = app.trail_source_choices(&source.info.source_id, 0).unwrap();
    assert!(!choices.stops.is_empty());
    assert!(choices
        .stops
        .iter()
        .all(|s| matches!(&s.context,Some(TrailContext::Media{channel,..}) if channel=="video")));
    let mut wrong_channel = choices.stops[0].clone();
    if let Some(TrailContext::Media { channel, .. }) = &mut wrong_channel.context {
        *channel = "audio".into();
    }
    assert_eq!(
        app.resolve_trail(vec![wrong_channel]).unwrap()[0].status,
        "unavailable"
    );
    let resolved = app.resolve_trail(vec![choices.stops[0].clone()]).unwrap();
    assert_eq!(resolved[0].status, "available");
    assert!(resolved[0]
        .locator
        .as_ref()
        .unwrap()
        .contains("video media"));
}
