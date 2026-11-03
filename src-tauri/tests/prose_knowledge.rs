use knowledge_garden::{
    application::{AcquisitionMethod, Application},
    extraction::LocalExtractor,
    providers::{JevSemanticProvider, SystemOneTransport},
    semantic::{ProviderError, SemanticProvider},
};
use serde_json::{json, Value};
use std::{fs, sync::Arc};

// Authored judgments, independent of model variance. The real candidate builder,
// typed response validator, evidence checks and publication path all run.
struct RecordedProseJudgments;
impl SystemOneTransport for RecordedProseJudgments {
    fn complete(&self, _: &str, request: &Value) -> Result<Value, ProviderError> {
        let mut answers = serde_json::Map::new();
        for (key, question) in request["questions"].as_object().unwrap() {
            let instructions = question["instructions"].as_str().unwrap();
            let answer = if question["type"] == "noul" {
                json!({"type":"noul", "noul":if instructions.contains("Heading only") {0.01} else {0.99}})
            } else {
                let choice = if key.starts_with("prose_role_") {
                    if instructions.contains("was not held") {
                        "statement"
                    } else if instructions.contains("opens on")
                        || instructions.contains("inspection is scheduled")
                    {
                        "event"
                    } else {
                        "statement"
                    }
                } else if key == "source_update_role" {
                    "supplement"
                } else if key == "source_update_evidence" {
                    "span_0"
                } else {
                    "none"
                };
                json!({"type":"choice", "choice":choice, "probabilities":{choice:0.99}})
            };
            answers.insert(key.clone(), answer);
        }
        Ok(json!({"model":"typesafe/jev-1.14", "answers":answers}))
    }
}
fn provider() -> JevSemanticProvider {
    JevSemanticProvider::with_transport(
        "synthetic-credential".into(),
        Arc::new(RecordedProseJudgments),
    )
}

fn exhausted_legacy_prose(root: &std::path::Path, input: &std::path::Path) -> String {
    let mut app = Application::open(root).unwrap();
    let source = app.import_source(input, AcquisitionMethod::Picker).unwrap();
    for _ in 0..3 {
        let job = app
            .claim_due_semantic_jobs_at(1, u128::MAX / 2)
            .unwrap()
            .pop()
            .unwrap();
        app.finish_semantic_job(job, Err(ProviderError::recoverable(
            "No supported event identity or acquired reference was found; semantic coverage remains incomplete and will retry automatically.".into(),
        ))).unwrap();
    }
    assert_eq!(
        app.open_source(&source.info.source_id)
            .unwrap()
            .info
            .semantic_state,
        "failed"
    );
    source.info.source_id
}

#[test]
fn exhausted_legacy_prose_is_processed_after_the_policy_upgrade() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let input = temp.path().join("garden.txt");
    fs::write(&input, GARDEN).unwrap();
    let id = exhausted_legacy_prose(&root, &input);
    let mut app =
        Application::open_with_providers(&root, Arc::new(provider()), Arc::new(LocalExtractor))
            .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    assert_eq!(
        app.open_source(&id).unwrap().info.semantic_state,
        "complete"
    );
    assert!(app
        .claim_due_semantic_jobs_at(1, u128::MAX / 2)
        .unwrap()
        .is_empty());
}

#[test]
fn the_same_policy_never_replenishes_an_exhausted_failure_budget() {
    struct StillUnavailable;
    impl SystemOneTransport for StillUnavailable {
        fn complete(&self, _: &str, _: &Value) -> Result<Value, ProviderError> {
            Err(ProviderError::recoverable("No supported event identity or acquired reference was found; semantic coverage remains incomplete.".into()))
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let input = temp.path().join("garden.txt");
    fs::write(&input, GARDEN).unwrap();
    let id = exhausted_legacy_prose(&root, &input);
    let provider = JevSemanticProvider::with_transport(
        "synthetic-credential".into(),
        Arc::new(StillUnavailable),
    );
    let mut app =
        Application::open_with_providers(&root, Arc::new(provider), Arc::new(LocalExtractor))
            .unwrap();
    for _ in 0..3 {
        app.resume_due_semantic_jobs_at(u128::MAX / 2).unwrap();
    }
    assert_eq!(app.open_source(&id).unwrap().info.semantic_state, "failed");
    assert!(app
        .claim_due_semantic_jobs_at(1, u128::MAX / 2)
        .unwrap()
        .is_empty());
}

const GARDEN: &str = "The community garden opens on November 15, 2026. The water inspection is scheduled for November 10, 2026.";

#[test]
fn newline_terminated_prose_schedules_keep_their_own_dates() {
    for ending in ["\n", "\r\n"] {
        for (first, second) in [
            ("2026-11-20", "2026-11-18"),
            ("November 20, 2026", "November 18, 2026"),
        ] {
            let source = format!("The garden opens on {first}{ending}The inspection is scheduled for {second}{ending}");
            let draft = provider()
                .form_knowledge_for_source(&source, None, "md")
                .unwrap();
            let dates = draft
                .facts
                .iter()
                .filter(|fact| fact.property == "event_date")
                .collect::<Vec<_>>();
            assert_eq!(dates.len(), 2);
            for (subject, value) in [("garden opens", first), ("inspection is scheduled", second)] {
                let date = dates
                    .iter()
                    .find(|fact| fact.subject.contains(subject))
                    .unwrap();
                assert_eq!(date.value, value);
                assert_eq!(
                    &source[date.evidence.byte_start..date.evidence.byte_end],
                    date.evidence.quote
                );
            }
        }
    }
}

#[test]
fn iso_dates_remain_attached_to_their_own_prose_events() {
    let source = "The café garden opens on 2026-11-20. The water inspection is scheduled for November 18, 2026.";
    let draft = provider()
        .form_knowledge_for_source(source, None, "md")
        .unwrap();
    for (subject, value) in [
        ("garden opens", "2026-11-20"),
        ("inspection is scheduled", "November 18, 2026"),
    ] {
        let dates = draft
            .facts
            .iter()
            .filter(|fact| fact.subject.contains(subject) && fact.property == "event_date")
            .collect::<Vec<_>>();
        assert_eq!(dates.len(), 1);
        assert_eq!(dates[0].value, value);
        assert_eq!(
            &source[dates[0].evidence.byte_start..dates[0].evidence.byte_end],
            dates[0].evidence.quote
        );
        assert!(dates[0].evidence.quote.contains(subject));
    }
}

#[test]
fn invalid_iso_calendar_dates_remain_literal_source_statements() {
    let source = "The garden opens on 2026-02-30.";
    let draft = provider().form_knowledge(source).unwrap();
    assert!(draft.facts.iter().all(|fact| fact.property != "event_date"));
    assert!(draft.facts.iter().any(|fact| fact.value == source));
}

#[test]
fn repeated_iso_date_in_one_passage_does_not_duplicate_the_date_fact() {
    let source = "The garden opens on 2026-11-20; the date 2026-11-20 is confirmed in this note.";
    let draft = provider().form_knowledge(source).unwrap();
    assert_eq!(
        draft
            .facts
            .iter()
            .filter(|fact| fact.property == "event_date")
            .count(),
        1
    );
}

#[test]
fn a_reference_link_does_not_hide_ordinary_event_descriptions() {
    let source = "The community garden opens on November 15, 2026. See https://example.org/schedule for further context.";
    let draft = provider().form_knowledge(source).unwrap();
    assert!(draft.entities.iter().any(|entity| entity.kind == "event"));
    assert!(draft
        .entities
        .iter()
        .any(|entity| entity.kind == "reference"));
    for entity in &draft.entities {
        assert_eq!(
            &source[entity.evidence.byte_start..entity.evidence.byte_end],
            entity.evidence.quote
        );
    }
}

#[test]
fn ordinary_dated_prose_forms_distinct_grounded_events_without_special_identifiers() {
    let draft = provider().form_knowledge(GARDEN).unwrap();
    let events = draft
        .entities
        .iter()
        .filter(|entity| entity.kind == "event")
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 2);
    for (phrase, date) in [
        ("garden opens", "November 15, 2026"),
        ("inspection is scheduled", "November 10, 2026"),
    ] {
        let event = events
            .iter()
            .find(|event| event.label.contains(phrase))
            .unwrap();
        let dates = draft
            .facts
            .iter()
            .filter(|fact| fact.subject == event.label && fact.property == "event_date")
            .collect::<Vec<_>>();
        assert_eq!(dates.len(), 1);
        assert_eq!(dates[0].value, date);
        assert_eq!(
            &GARDEN[event.evidence.byte_start..event.evidence.byte_end],
            event.evidence.quote
        );
        assert!(dates[0].evidence.quote.contains(phrase));
    }
}

#[test]
fn ordinary_prose_publishes_through_the_application_boundary() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("garden.txt");
    fs::write(&input, GARDEN).unwrap();
    let mut app = Application::open_with_providers(
        temp.path().join("collection"),
        Arc::new(provider()),
        Arc::new(LocalExtractor),
    )
    .unwrap();
    let source = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let current = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(current.info.semantic_state, "complete");
    assert!(current.info.update_status.is_none());
    assert_eq!(
        current
            .knowledge_pages
            .iter()
            .filter(|page| page.kind == "event")
            .count(),
        2
    );
    for page in &current.knowledge_pages {
        assert!(!app
            .open_knowledge_page(&page.page_id)
            .unwrap()
            .markdown
            .is_empty());
    }
}

#[test]
fn a_negative_statement_does_not_become_an_occurred_event() {
    let draft = provider()
        .form_knowledge("The garden opening was not held on November 15, 2026.")
        .unwrap();
    assert!(draft.entities.iter().all(|entity| entity.kind != "event"));
    assert!(draft.facts.iter().all(|fact| fact.property != "event_date"));
    assert!(draft
        .facts
        .iter()
        .any(|fact| fact.value.contains("was not held")));
}
