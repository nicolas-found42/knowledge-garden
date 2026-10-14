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
