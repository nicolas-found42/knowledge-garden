use knowledge_garden::application::{AcquisitionMethod, Application};
use knowledge_garden::providers::{JevSemanticProvider, SystemOneTransport};
use knowledge_garden::semantic::{
    EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft, ProviderError, SemanticProvider,
};
use knowledge_garden::timeline::TimelineRequest;
use serde_json::{json, Value};
use std::{fs, sync::Arc};

struct FrozenObservationDates;

struct FrozenMonthJudgments;

impl SystemOneTransport for FrozenMonthJudgments {
    fn complete(&self, _key: &str, request: &Value) -> Result<Value, ProviderError> {
        let mut answers = serde_json::Map::new();
        for (key, question) in request["questions"].as_object().unwrap() {
            if question["type"] == "noul" {
                answers.insert(key.clone(), json!({"type":"noul", "noul":0.99}));
                continue;
            }
            let criteria = question["criteria"].as_object().unwrap();
            let choice = match key.as_str() {
                "source_update_role" => "unknown",
                "source_order_date" | "source_order_revision" => "none",
                "source_update_evidence" => criteria
                    .keys()
                    .find(|option| option.starts_with("span_"))
                    .unwrap(),
                "event_date" => criteria
                    .iter()
                    .find(|(_, candidate)| candidate["value"] == "May 2024")
                    .map(|(option, _)| option.as_str())
                    .unwrap_or("none"),
                name if name.starts_with("event_identity_") => "event",
                _ => criteria.keys().next().unwrap(),
            };
            answers.insert(
                key.clone(),
                json!({"type":"choice", "choice":choice, "probabilities":{choice:0.99}}),
            );
        }
        Ok(json!({"model":"typesafe/jev-recorded-month-fixture", "answers":answers}))
    }
}

#[test]
fn jev_provider_keeps_a_source_month_as_month_precision_in_the_timeline() {
    let temporary = tempfile::tempdir().unwrap();
    let input = temporary.path().join("month.txt");
    let text =
        "Observation V20 occurred in May 2024. The source specifies the month only, not a day.\n";
    fs::write(&input, text).unwrap();
    let provider = JevSemanticProvider::with_transport(
        "recorded-fixture".into(),
        Arc::new(FrozenMonthJudgments),
    );
    let mut app = Application::open_with_semantic_provider(
        temporary.path().join("collection"),
        Arc::new(provider),
    )
    .unwrap();
    let source = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(
        source.info.semantic_state, "complete",
        "{:?}",
        source.info.semantic_error
    );
    let timeline = app.timeline(TimelineRequest::default()).unwrap();
    assert_eq!(timeline.entries.len(), 1);
    assert_eq!(timeline.entries[0].date_label, "May 2024");
    assert_eq!(timeline.entries[0].precision, "month");
    assert_eq!(
        timeline.entries[0].date_start.as_deref(),
        Some("2024-05-01")
    );
    assert_eq!(timeline.entries[0].date_end.as_deref(), Some("2024-05-31"));
    assert_eq!(
        fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        text.as_bytes()
    );
    assert!(app
        .open_knowledge_page(&timeline.entries[0].page_id)
        .unwrap()
        .markdown
        .contains("May 2024"));
}

impl SemanticProvider for FrozenObservationDates {
    fn form_knowledge(&self, text: &str) -> Result<KnowledgeDraft, ProviderError> {
        let (label, dates) = match text {
            "Observation V18 occurred on May 18, 2024." => ("V18", vec!["2024-05-18"]),
            "Observation V17 occurred on May 17, 2024." => ("V17", vec!["2024-05-17"]),
            "Observation M occurred in May 2024." => ("M", vec!["May 2024"]),
            "Observation Y occurred in 2024." => ("Y", vec!["2024"]),
            "Observation I occurred during 2024-05-03 to 2024-05-09." => {
                ("I", vec!["2024-05-03 to 2024-05-09"])
            }
            "Observation U has no supplied date." => ("U", vec![]),
            "Observation C has conflicting reports: May 16, 2024 or May 19, 2024." => {
                ("C", vec!["2024-05-16", "2024-05-19"])
            }
            _ => panic!("unrecorded source"),
        };
        let evidence = EvidenceDraft {
            quote: text.into(),
            byte_start: 0,
            byte_end: text.len(),
            origin: "source-described observation".into(),
            qualifier: None,
            offset_basis: None,
            source_location: None,
        };
        Ok(KnowledgeDraft {
            entities: vec![EntityDraft {
                kind: "event".into(),
                label: label.into(),
                evidence: evidence.clone(),
            }],
            facts: dates
                .into_iter()
                .enumerate()
                .map(|(index, date)| FactDraft {
                    subject: label.into(),
                    property: "occurred_on".into(),
                    value: date.into(),
                    evidence: evidence.clone(),
                    record_key: Some(format!("date-report-{index}")),
                })
                .collect(),
            tags: vec![knowledge_garden::semantic::TagDraft {
                subject: label.into(),
                label: "observations".into(),
                evidence: evidence.clone(),
            }],
            ..KnowledgeDraft::default()
        })
    }
}

#[test]
fn timeline_preserves_month_precision_unknown_dates_and_conflicting_reports() {
    let temporary = tempfile::tempdir().unwrap();
    let collection = temporary.path().join("collection");
    let mut app =
        Application::open_with_semantic_provider(&collection, Arc::new(FrozenObservationDates))
            .unwrap();
    for (name, text) in [
        ("m.txt", "Observation M occurred in May 2024."),
        ("u.txt", "Observation U has no supplied date."),
        (
            "c.txt",
            "Observation C has conflicting reports: May 16, 2024 or May 19, 2024.",
        ),
    ] {
        let input = temporary.path().join(name);
        fs::write(&input, text).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        app.resume_due_semantic_jobs().unwrap();
        assert_eq!(
            app.open_source(&source.info.source_id)
                .unwrap()
                .info
                .semantic_state,
            "complete"
        );
    }
    let timeline = app.timeline(TimelineRequest::default()).unwrap();
    assert_eq!(timeline.entries.len(), 3);
    assert_eq!(timeline.entries[0].title, "M");
    let month = &timeline.entries[0];
    assert_eq!(month.date_label, "May 2024");
    assert_eq!(month.precision, "month");
    assert_eq!(month.date_status, "approximate");
    assert_eq!(month.date_start.as_deref(), Some("2024-05-01"));
    assert_eq!(month.date_end.as_deref(), Some("2024-05-31"));
    let unknown = timeline
        .entries
        .iter()
        .find(|entry| entry.title == "U")
        .unwrap();
    assert_eq!(unknown.date_status, "unknown");
    assert_eq!(unknown.date_start, None);
    assert_eq!(unknown.date_end, None);
    let conflict = timeline
        .entries
        .iter()
        .find(|entry| entry.title == "C")
        .unwrap();
    assert_eq!(conflict.date_status, "conflicting");
    assert_eq!(conflict.date_start, None);
    assert!(conflict.date_label.contains("2024-05-16"));
    assert!(conflict.date_label.contains("2024-05-19"));
}

#[test]
fn source_event_timeline_sorts_real_dates_independently_of_acquisition_order() {
    let temporary = tempfile::tempdir().unwrap();
    let mut app = Application::open_with_semantic_provider(
        temporary.path().join("collection"),
        Arc::new(FrozenObservationDates),
    )
    .unwrap();
    for (name, text) in [
        ("v18.txt", "Observation V18 occurred on May 18, 2024."),
        ("v17.txt", "Observation V17 occurred on May 17, 2024."),
    ] {
        let input = temporary.path().join(name);
        fs::write(&input, text).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        app.resume_due_semantic_jobs().unwrap();
        assert_eq!(
            app.open_source(&source.info.source_id)
                .unwrap()
                .info
                .semantic_state,
            "complete"
        );
        assert_eq!(
            fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
            text.as_bytes()
        );
    }
    let timeline = app.timeline(TimelineRequest::default()).unwrap();
    assert_eq!(
        timeline
            .entries
            .iter()
            .map(|entry| entry.title.as_str())
            .collect::<Vec<_>>(),
        ["V17", "V18"]
    );
    assert_eq!(
        timeline.entries[0].date_start.as_deref(),
        Some("2024-05-17")
    );
    assert_eq!(
        timeline.entries[1].date_start.as_deref(),
        Some("2024-05-18")
    );
    assert_ne!(timeline.entries[0].page_id, timeline.entries[1].page_id);
}

#[test]
fn a_malformed_event_page_does_not_hide_the_collection_on_reopen() {
    let temporary = tempfile::tempdir().unwrap();
    let collection = temporary.path().join("collection");
    let input = temporary.path().join("v17.txt");
    let text = "Observation V17 occurred on May 17, 2024.";
    fs::write(&input, text).unwrap();
    let mut app =
        Application::open_with_semantic_provider(&collection, Arc::new(FrozenObservationDates))
            .unwrap();
    let source = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let ready = app.open_source(&source.info.source_id).unwrap();
    let event_path = collection.join(&ready.knowledge_pages[0].path);
    let owner_bytes = b"---\nschema: malformed-owner-edit\n---\n\n# Owner keeps this note\n\nRetain this intentional text.\n";
    fs::write(&event_path, owner_bytes).unwrap();
    drop(app);
    let reopened = Application::open(&collection).unwrap_or_else(|error| {
        panic!("damaged event metadata must not make every source inaccessible: {error}")
    });
    assert_eq!(
        reopened
            .open_source(&source.info.source_id)
            .unwrap()
            .info
            .source_id,
        source.info.source_id
    );
    assert_eq!(fs::read(&event_path).unwrap(), owner_bytes);
    assert_eq!(
        fs::read(reopened.original_path(&source.info.source_id).unwrap()).unwrap(),
        text.as_bytes()
    );
}

#[test]
fn all_date_meanings_use_actual_dates_and_current_filters() {
    use knowledge_garden::timeline::{DateMeaning, DateOrder};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let mut app =
        Application::open_with_semantic_provider(&root, Arc::new(FrozenObservationDates)).unwrap();
    for (name, text) in [
        ("v18.txt", "Observation V18 occurred on May 18, 2024."),
        ("v17.txt", "Observation V17 occurred on May 17, 2024."),
        ("year.txt", "Observation Y occurred in 2024."),
        (
            "interval.txt",
            "Observation I occurred during 2024-05-03 to 2024-05-09.",
        ),
        ("unknown.txt", "Observation U has no supplied date."),
    ] {
        let path = temp.path().join(name);
        fs::write(&path, text).unwrap();
        app.import_source(path, AcquisitionMethod::Picker).unwrap();
        app.resume_due_semantic_jobs().unwrap();
    }
    let events = app.timeline(TimelineRequest::default()).unwrap();
    assert_eq!(
        events
            .entries
            .iter()
            .map(|e| e.title.as_str())
            .collect::<Vec<_>>(),
        ["Y", "I", "V17", "V18", "U"]
    );
    assert_eq!(events.entries[0].precision, "year");
    assert_eq!(events.entries[1].date_end.as_deref(), Some("2024-05-09"));
    assert!(events.entries[4].date_start.is_none());
    let descending = app
        .timeline(TimelineRequest {
            order: DateOrder::Descending,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        descending
            .entries
            .iter()
            .map(|e| e.title.as_str())
            .collect::<Vec<_>>(),
        ["V18", "V17", "I", "Y", "U"]
    );
    let filtered = app
        .timeline(TimelineRequest {
            query: "V17".into(),
            tags: vec!["observations".into()],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(filtered.entries.len(), 1);
    assert_eq!(filtered.entries[0].title, "V17");
    assert!(app
        .timeline(TimelineRequest {
            tags: vec!["absent".into()],
            ..Default::default()
        })
        .unwrap()
        .entries
        .is_empty());
    let activity = app
        .timeline(TimelineRequest {
            date_meaning: DateMeaning::CollectionActivity,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(activity.entries.len(), 5);
    assert_eq!(
        app.timeline(TimelineRequest {
            date_meaning: DateMeaning::CollectionActivity,
            tags: vec!["observations".into()],
            ..Default::default()
        })
        .unwrap()
        .entries
        .len(),
        5
    );
    assert!(activity
        .entries
        .iter()
        .all(|e| e.kind == "acquired" && e.date_start.as_deref().is_some_and(|d| d.contains('T'))));
    assert!(activity.entries.iter().all(|e| e
        .qualifier
        .as_deref()
        .unwrap()
        .contains("not the source-described")));
    app.rebuild_index().unwrap();
    assert_eq!(
        app.timeline(TimelineRequest::default())
            .unwrap()
            .entries
            .len(),
        5
    );
}

struct FrozenDecision;
impl SystemOneTransport for FrozenDecision {
    fn complete(&self, _key: &str, request: &Value) -> Result<Value, ProviderError> {
        let mut answers = serde_json::Map::new();
        for key in request["questions"].as_object().unwrap().keys() {
            let choice = if key.ends_with("_1") {
                "reasoning_event"
            } else {
                "recorded_decision"
            };
            answers.insert(key.clone(),json!({"type":"choice","choice":choice,"probabilities":{choice:0.99,"uncertain":0.01}}));
        }
        Ok(json!({"model":"typesafe/jev-frozen-timeline","answers":answers}))
    }
}
#[test]
fn reasoning_dates_keep_message_provenance_without_using_acquisition_time() {
    use knowledge_garden::timeline::DateMeaning;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let path = temp.path().join("decision.json");
    fs::write(&path,serde_json::to_vec(&json!({"format":"knowledge-garden-conversation-v1","conversation_id":"timeline-decision","messages":[{"id":"decision","role":"user","date":"2024-05-17T08:03:00-04:00","content":"Decision: use plan Alder."},{"id":"reason","role":"reasoning","content":"The chosen plan retains a rollback."}]})).unwrap()).unwrap();
    let mut app = Application::open_with_semantic_provider(
        &root,
        Arc::new(JevSemanticProvider::with_transport(
            "fixture".into(),
            Arc::new(FrozenDecision),
        )),
    )
    .unwrap();
    let source = app.import_source(&path, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let complete = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(
        complete.info.semantic_state, "complete",
        "{:?}",
        complete.info.semantic_error
    );
    let reasoning = app
        .timeline(TimelineRequest {
            date_meaning: DateMeaning::ReasoningEvent,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(reasoning.entries.len(), 2);
    assert_eq!(
        reasoning.entries[0].date_start.as_deref(),
        Some("2024-05-17T12:03:00.000Z")
    );
    assert_eq!(reasoning.entries[1].date_status, "unknown");
    assert!(reasoning.entries[1].date_start.is_none());
    assert!(reasoning.entries[0]
        .support
        .as_ref()
        .unwrap()
        .source_location
        .as_ref()
        .unwrap()
        .contains("message=decision"));
    assert!(app
        .timeline(TimelineRequest::default())
        .unwrap()
        .entries
        .is_empty());
    drop(app);
    let app = Application::open(root).unwrap();
    assert_eq!(
        app.timeline(TimelineRequest {
            date_meaning: DateMeaning::ReasoningEvent,
            ..Default::default()
        })
        .unwrap()
        .entries
        .len(),
        2
    );
}

#[test]
fn acquisition_activity_is_bounded_and_continues_past_fifty_without_fabricating_events() {
    use knowledge_garden::timeline::DateMeaning;
    let temp = tempfile::tempdir().unwrap();
    let mut app = Application::open(temp.path().join("collection")).unwrap();
    for i in 0..51 {
        let path = temp.path().join(format!("copy-{i}.txt"));
        fs::write(&path, "An undated retained note.").unwrap();
        app.import_source(path, AcquisitionMethod::Picker).unwrap();
    }
    let first = app
        .timeline(TimelineRequest {
            date_meaning: DateMeaning::CollectionActivity,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(first.entries.len(), 50);
    assert_eq!(first.next_offset, Some(50));
    let second = app
        .timeline(TimelineRequest {
            date_meaning: DateMeaning::CollectionActivity,
            offset: 50,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(second.entries.len(), 1);
    assert!(second.next_offset.is_none());
    assert!(!first
        .entries
        .iter()
        .any(|e| e.entry_id == second.entries[0].entry_id));
    assert!(app
        .timeline(TimelineRequest::default())
        .unwrap()
        .entries
        .is_empty());
}

#[test]
fn removing_a_representative_source_keeps_a_retained_pages_selected_alternate_support() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let mut app =
        Application::open_with_semantic_provider(&root, Arc::new(FrozenObservationDates)).unwrap();
    let a = temp.path().join("a.txt");
    fs::write(&a, "Observation V17 occurred on May 17, 2024.").unwrap();
    let a = app.import_source(a, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let a = app.open_source(&a.info.source_id).unwrap();
    let b = temp.path().join("b.txt");
    fs::write(&b, "Observation V18 occurred on May 18, 2024.").unwrap();
    let b = app.import_source(b, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let b = app.open_source(&b.info.source_id).unwrap();
    // A persisted page can select date support supplied by another retained source.
    // Exercise that durable-record boundary independently of identity judgment.
    let a_path = root.join(&a.knowledge_pages[0].path);
    let b_path = root.join(&b.knowledge_pages[0].path);
    let a_bytes = fs::read_to_string(&a_path).unwrap();
    let b_bytes = fs::read_to_string(b_path).unwrap();
    let mut chunks = a_bytes.splitn(3, "---");
    chunks.next();
    let yaml = chunks.next().unwrap();
    let body = chunks.next().unwrap();
    let mut header: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml).unwrap();
    let b_header: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(b_bytes.split("---").nth(1).unwrap()).unwrap();
    header["facts"][0] = b_header["facts"][0].clone();
    fs::write(
        &a_path,
        format!(
            "---\n{}---{}",
            serde_yaml_ng::to_string(&header).unwrap(),
            body
        ),
    )
    .unwrap();
    fs::remove_dir_all(
        root.join("sources")
            .join(a.info.source_id.trim_start_matches("source-")),
    )
    .unwrap();
    app.rebuild_index().unwrap();
    let entries = app.timeline(TimelineRequest::default()).unwrap().entries;
    let page = entries
        .iter()
        .find(|entry| entry.page_id == a.knowledge_pages[0].page_id)
        .unwrap();
    assert_eq!(
        page.support.as_ref().map(|s| s.source_id.as_str()),
        Some(b.info.source_id.as_str())
    );
}
