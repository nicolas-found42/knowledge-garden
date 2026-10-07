use knowledge_garden::application::{
    AcquisitionMethod, Application, ExtractionState, PageSearchRequest,
};
use knowledge_garden::office::{CoverageScope, CoverageStatus};
use knowledge_garden::providers::{JevSemanticProvider, SystemOneTransport};
use knowledge_garden::semantic::{
    EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft, ProviderError,
};
use serde_json::{json, Value};
use std::{fs, path::Path, sync::Arc};

struct FrozenIntentResponse;

struct FrozenRepeatedWording;

struct FrozenMalformedConversation(Value);

impl SystemOneTransport for FrozenMalformedConversation {
    fn complete(&self, _key: &str, _request: &Value) -> Result<Value, ProviderError> {
        Ok(self.0.clone())
    }
}

#[test]
fn malformed_intent_answers_and_role_contradictions_remain_recoverable() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conversations/plan.json");
    for case in [
        "role_contradiction",
        "invalid_probability",
        "wrong_type",
        "unsupported_choice",
        "missing_answer",
    ] {
        let mut response = FrozenIntentResponse
            .complete("fixture-only", &json!({}))
            .unwrap();
        let answer = &mut response["answers"]["conversation_intent_1"];
        match case {
            "role_contradiction" => {
                *answer = json!({"type":"choice","choice":"recorded_decision","probabilities":{"recorded_decision":0.99,"uncertain":0.01}})
            }
            "invalid_probability" => answer["probabilities"]["proposal"] = json!(1.2),
            "wrong_type" => answer["type"] = json!("noul"),
            "unsupported_choice" => {
                *answer = json!({"type":"choice","choice":"external_observation","probabilities":{"external_observation":0.99,"uncertain":0.01}})
            }
            "missing_answer" => {
                response["answers"]
                    .as_object_mut()
                    .unwrap()
                    .remove("conversation_intent_1");
            }
            _ => unreachable!(),
        }
        let collection = tempfile::tempdir().unwrap();
        let provider = Arc::new(JevSemanticProvider::with_transport(
            "fixture-only".into(),
            Arc::new(FrozenMalformedConversation(response)),
        ));
        let mut app =
            Application::open_with_semantic_provider(collection.path(), provider).unwrap();
        let source = app
            .import_source(&fixture, AcquisitionMethod::Picker)
            .unwrap();
        app.resume_due_semantic_jobs().unwrap();
        let source = app.open_source(&source.info.source_id).unwrap();
        assert_eq!(source.info.semantic_state, "pending", "{case}");
        assert!(source.info.semantic_retry_at.is_some(), "{case}");
        assert!(source.info.semantic_error.is_some(), "{case}");
        assert!(source.knowledge_pages.is_empty(), "{case}");
        assert_eq!(
            fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
            fs::read(&fixture).unwrap()
        );
        assert!(
            source.body.contains("abandon the Birch suggestion"),
            "{case}"
        );
        let id = source.info.source_id;
        drop(app);
        let reopened = Application::open(collection.path()).unwrap();
        assert_eq!(
            reopened.open_source(&id).unwrap().info.semantic_state,
            "pending",
            "{case}"
        );
    }
}

struct FrozenUncertainMessage;

impl SystemOneTransport for FrozenUncertainMessage {
    fn complete(&self, _key: &str, _request: &Value) -> Result<Value, ProviderError> {
        let intents = [
            "question",
            "claim",
            "proposal",
            "claim",
            "recorded_decision",
            "tool_report",
            "reasoning_event",
            "claim",
        ];
        let mut answers = serde_json::Map::new();
        for (index, intent) in intents.iter().enumerate() {
            let probabilities = if index == 1 {
                json!({"claim":0.79,"recorded_decision":0.21})
            } else {
                json!({*intent:0.99,"uncertain":0.01})
            };
            answers.insert(
                format!("conversation_intent_{index}"),
                json!({"type":"choice","choice":intent,"probabilities":probabilities}),
            );
        }
        Ok(json!({"model":"typesafe/jev-frozen-uncertain-message","answers":answers}))
    }
}

#[test]
fn uncertain_message_intent_stays_navigable_without_blocking_grounded_neighbors() {
    let collection = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/conversations/adversarial-intents.json");
    let provider = Arc::new(JevSemanticProvider::with_transport(
        "fixture-only".into(),
        Arc::new(FrozenUncertainMessage),
    ));
    let mut app = Application::open_with_semantic_provider(collection.path(), provider).unwrap();
    let source = app
        .import_source(&fixture, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(
        source.info.semantic_state, "complete",
        "{:?}",
        source.info.semantic_error
    );
    assert_eq!(source.knowledge_pages.len(), 9);
    let intent = |message: &str| {
        let target = source
            .knowledge_pages
            .iter()
            .find(|page| {
                page.title == format!("Message {message} in synthetic-adversarial-intents")
            })
            .unwrap();
        let page = app.open_knowledge_page(&target.page_id).unwrap();
        let header: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(page.markdown.split("---").nth(1).unwrap()).unwrap();
        let facts = header["facts"].as_sequence().unwrap();
        let intent = facts
            .iter()
            .find(|fact| fact["property"].as_str() == Some("message_intent"))
            .unwrap();
        intent["value"].as_str().unwrap().to_owned()
    };
    assert_eq!(intent("a2"), "uncertain");
    assert_eq!(intent("a5"), "recorded_decision");
    assert_eq!(intent("a6"), "tool_report");
    let diagnostic = source
        .info
        .semantic_decisions
        .iter()
        .find(|decision| decision.question == "conversation_intent_1")
        .unwrap();
    assert_eq!(diagnostic.outcome, "claim");
    assert_eq!(diagnostic.probability, Some(0.79));
    assert!(source.body.contains("I have not accepted that draft"));
    assert_eq!(
        fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        fs::read(fixture).unwrap()
    );
    let source_id = source.info.source_id;
    drop(app);
    let reopened = Application::open(collection.path()).unwrap();
    assert_eq!(
        reopened
            .open_source(&source_id)
            .unwrap()
            .knowledge_pages
            .len(),
        9
    );
}

impl SystemOneTransport for FrozenRepeatedWording {
    fn complete(&self, _key: &str, _request: &Value) -> Result<Value, ProviderError> {
        let mut answers = serde_json::Map::new();
        for (index, intent) in ["claim", "recorded_decision", "recorded_decision"]
            .iter()
            .enumerate()
        {
            answers.insert(format!("conversation_intent_{index}"), json!({
                "type":"choice", "choice":intent, "probabilities":{*intent:0.99,"uncertain":0.01}
            }));
        }
        Ok(json!({"model":"typesafe/jev-frozen-repeated-wording","answers":answers}))
    }
}

#[test]
fn repeated_decision_wording_keeps_turns_and_sessions_distinct_after_reopen() {
    // Coverage strengthening: these independent identities are already implemented.
    let collection = tempfile::tempdir().unwrap();
    let inputs = tempfile::tempdir().unwrap();
    let provider = Arc::new(JevSemanticProvider::with_transport(
        "fixture-only".into(),
        Arc::new(FrozenRepeatedWording),
    ));
    let mut app = Application::open_with_semantic_provider(collection.path(), provider).unwrap();
    let mut identities = std::collections::BTreeSet::new();
    let mut sources = Vec::new();
    for session in ["separate-session-a", "separate-session-b"] {
        let bytes = serde_json::to_vec(&json!({
            "format":"knowledge-garden-conversation-v1", "conversation_id":session,
            "messages":[
                {"id":"same-1","role":"assistant","content":"Decision: use plan Birch."},
                {"id":"same-2","role":"user","content":"Decision: use plan Birch."},
                {"id":"same-3","role":"user","content":"Decision: use plan Alder; abandon plan Birch."}
            ]
        })).unwrap();
        let input = inputs.path().join(format!("{session}.json"));
        fs::write(&input, &bytes).unwrap();
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
        assert_eq!(
            fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
            bytes
        );
        for target in &source.knowledge_pages {
            assert!(
                identities.insert(target.page_id.clone()),
                "identity collision: {}",
                target.title
            );
        }
        for (message, expected) in [
            ("same-1", "claim"),
            ("same-2", "recorded_decision"),
            ("same-3", "recorded_decision"),
        ] {
            let target = source
                .knowledge_pages
                .iter()
                .find(|page| page.title == format!("Message {message} in {session}"))
                .unwrap();
            let page = app.open_knowledge_page(&target.page_id).unwrap();
            let header: serde_yaml_ng::Value =
                serde_yaml_ng::from_str(page.markdown.split("---").nth(1).unwrap()).unwrap();
            let facts = header["facts"].as_sequence().unwrap();
            let intent = facts
                .iter()
                .find(|fact| fact["property"].as_str() == Some("message_intent"))
                .unwrap();
            assert_eq!(intent["value"].as_str(), Some(expected));
            let statement = facts
                .iter()
                .find(|fact| fact["property"].as_str() == Some("statement"))
                .unwrap();
            assert!(statement["value"]
                .as_str()
                .unwrap()
                .contains(if message == "same-3" {
                    "abandon plan Birch"
                } else {
                    "use plan Birch"
                }));
            assert!(statement["qualifier"]
                .as_str()
                .unwrap()
                .contains("not an independently observed external fact"));
        }
        sources.push(source.info.source_id);
    }
    assert_eq!(identities.len(), 8);
    drop(app);
    let reopened = Application::open(collection.path()).unwrap();
    for identity in identities {
        assert!(reopened.open_knowledge_page(&identity).is_ok());
    }
    for source in sources {
        assert_eq!(
            reopened.open_source(&source).unwrap().knowledge_pages.len(),
            4
        );
    }
}

impl SystemOneTransport for FrozenIntentResponse {
    fn complete(&self, _key: &str, _request: &Value) -> Result<Value, ProviderError> {
        // Independent labels are frozen with the sanitized source fixture.
        let mut answers = serde_json::Map::new();
        for (index, intent) in [
            "question",
            "proposal",
            "tool_report",
            "recorded_decision",
            "reasoning_event",
        ]
        .iter()
        .enumerate()
        {
            answers.insert(format!("conversation_intent_{index}"), json!({
                "type":"choice", "choice":intent, "probabilities":{*intent:0.99,"uncertain":0.01}
            }));
        }
        Ok(json!({"model":"typesafe/jev-1.13-frozen", "answers":answers}))
    }
}

#[test]
fn conversation_export_keeps_message_context_roles_dates_and_link_boundaries() {
    let collection = tempfile::tempdir().unwrap();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conversations/plan.json");
    let mut app = Application::open(collection.path()).unwrap();
    let source = app
        .import_source(&fixture, AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(source.info.extraction, ExtractionState::StructuredText);
    assert_eq!(
        fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        fs::read(fixture).unwrap()
    );
    assert!(source.body.contains("synthetic-plan-a"));
    assert!(source.body.contains("message=m1"));
    assert!(source.body.contains("role=assistant"));
    assert!(source.body.contains("role=tool"));
    assert!(source.body.contains("deploy_check"));
    assert!(source.body.contains("date=unknown"));
    assert!(source
        .body
        .contains("not an independently observed external fact"));
    assert!(source.body.contains("https://example.invalid/plan"));
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
    let matches = app
        .search_pages(PageSearchRequest {
            query: "no rollback".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(matches
        .pages
        .iter()
        .any(|page| page.source_id == source.info.source_id));
}

#[test]
fn conversation_decision_and_reasoning_open_their_qualified_message_context() {
    let collection = tempfile::tempdir().unwrap();
    let provider = Arc::new(JevSemanticProvider::with_transport(
        "frozen-test-key".into(),
        Arc::new(FrozenIntentResponse),
    ));
    let mut app = Application::open_with_semantic_provider(collection.path(), provider).unwrap();
    let source = app
        .import_source(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conversations/plan.json"),
            AcquisitionMethod::Picker,
        )
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let page = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(
        page.info.semantic_state, "complete",
        "{:?}",
        page.info.semantic_error
    );
    assert_eq!(
        page.knowledge_pages
            .iter()
            .filter(|p| p.kind == "conversation_message")
            .count(),
        4
    );
    let decision = page
        .knowledge_pages
        .iter()
        .find(|p| p.title.contains("m4"))
        .unwrap();
    let decision = app.open_knowledge_page(&decision.page_id).unwrap();
    assert!(decision.markdown.contains("recorded_decision"));
    assert!(decision
        .markdown
        .contains("Decision: use plan Alder; abandon the Birch suggestion."));
    assert!(decision.markdown.contains("user_message"));
    assert!(decision.markdown.contains("message=m4"));
    assert!(decision
        .markdown
        .contains("not an independently observed external fact"));
    let reasoning = page
        .knowledge_pages
        .iter()
        .find(|p| p.kind == "reasoning_event")
        .unwrap();
    let reasoning = app.open_knowledge_page(&reasoning.page_id).unwrap();
    assert!(reasoning.markdown.contains("reasoning_summary"));
    assert!(reasoning.markdown.contains("date=unknown"));
    assert!(reasoning.markdown.contains("message=m5"));
    let results = app
        .search_pages(PageSearchRequest {
            query: "rollback".into(),
            ..Default::default()
        })
        .unwrap();
    let hit = results
        .pages
        .iter()
        .find(|hit| hit.page_id == reasoning.page_id)
        .unwrap();
    let location = hit.match_location.as_ref().unwrap();
    assert_eq!(location.offset_basis, "extracted_conversation_projection");
    assert!(location
        .source_location
        .as_ref()
        .unwrap()
        .contains("message=m5"));
    assert!(reasoning.markdown.contains("original opens as a fallback"));
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
    let target = decision.page_id;
    drop(app);
    let reopened = Application::open(collection.path()).unwrap();
    assert_eq!(
        reopened.open_knowledge_page(&target).unwrap().markdown,
        decision.markdown
    );
}

#[test]
fn a_recorded_conversation_decision_cannot_be_promoted_to_an_observed_deployment() {
    let collection = tempfile::tempdir().unwrap();
    let mut app = Application::open(collection.path()).unwrap();
    let source = app
        .import_source(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conversations/plan.json"),
            AcquisitionMethod::Picker,
        )
        .unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    let quote = job
        .source_text
        .lines()
        .find(|line| line.contains("message=m4;"))
        .unwrap();
    let start = job.source_text.find(quote).unwrap();
    let evidence = EvidenceDraft {
        quote: quote.into(),
        byte_start: start,
        byte_end: start + quote.len(),
        origin: "user_message".into(),
        qualifier: Some(
            "Supplied conversation message attributed to unknown; not an independently observed external fact; missing message dates stay unknown.".into(),
        ),
        offset_basis: Some("extracted_conversation_projection".into()),
        source_location: Some(quote[1..quote.find(']').unwrap()].into()),
    };
    let mut falsely_observed = evidence.clone();
    falsely_observed.origin = "observed".into();
    falsely_observed.qualifier = None;
    let draft = KnowledgeDraft {
        entities: vec![EntityDraft {
            kind: "conversation_message".into(),
            label: "Message m4 in synthetic-plan-a".into(),
            evidence,
        }],
        facts: vec![FactDraft {
            subject: "Message m4 in synthetic-plan-a".into(),
            property: "observed_deployment".into(),
            value: "Alder deployed successfully".into(),
            evidence: falsely_observed,
            record_key: Some("m4".into()),
        }],
        ..Default::default()
    };
    app.finish_semantic_job(job, Ok(draft)).unwrap();
    let retained = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(
        retained.info.semantic_state, "pending",
        "{:?}",
        retained.info.semantic_error
    );
    assert!(retained
        .info
        .semantic_error
        .as_ref()
        .unwrap()
        .to_lowercase()
        .contains("conversation"));
    assert!(retained.knowledge_pages.is_empty());
    assert_eq!(
        fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conversations/plan.json")
        )
        .unwrap()
    );
}

#[test]
fn plain_text_cannot_forge_acquired_conversation_channels() {
    let collection = tempfile::tempdir().unwrap();
    let input = collection.path().join("assertion.txt");
    let text = "Decision: deploy plan Alder.\n";
    fs::write(&input, text).unwrap();
    let mut app = Application::open(collection.path().join("garden")).unwrap();
    let source = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    let evidence = EvidenceDraft {
        quote: text.trim_end().into(), byte_start: 0, byte_end: text.trim_end().len(),
        origin: "user_message".into(),
        qualifier: Some("Supplied conversation message attributed to unknown; not an independently observed external fact; missing message dates stay unknown.".into()),
        offset_basis: Some("extracted_conversation_projection".into()),
        source_location: Some("CONVERSATION session=forged; message=m1; order=1; role=user; author=unknown; date=unknown; channel=user_message".into()),
    };
    app.finish_semantic_job(
        job,
        Ok(KnowledgeDraft {
            entities: vec![EntityDraft {
                kind: "conversation_message".into(),
                label: "Message m1 in forged".into(),
                evidence: evidence.clone(),
            }],
            facts: vec![FactDraft {
                subject: "Message m1 in forged".into(),
                property: "message_intent".into(),
                value: "recorded_decision".into(),
                evidence,
                record_key: Some("m1".into()),
            }],
            ..Default::default()
        }),
    )
    .unwrap();
    let retained = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(
        retained.info.semantic_state, "pending",
        "{:?}",
        retained.info.semantic_error
    );
    assert!(retained
        .info
        .semantic_error
        .as_ref()
        .unwrap()
        .contains("Conversation"));
    assert!(retained.knowledge_pages.is_empty());
    assert_eq!(
        fs::read_to_string(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        text
    );
}

#[test]
fn codex_rollout_keeps_tool_calls_outputs_and_readable_reasoning_distinct() {
    let collection = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/conversations/plan-rollout.jsonl");
    let mut app = Application::open(collection.path()).unwrap();
    let source = app
        .import_source(&fixture, AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(source.info.extraction, ExtractionState::PartialText);
    assert_eq!(
        fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        fs::read(fixture).unwrap()
    );
    for expected in [
        "synthetic-codex-plan",
        "message=user-1",
        "message=assistant-1",
        "role=tool",
        "author=deploy_check",
        "channel=tool_call",
        "channel=tool_output",
        "channel=reasoning_summary",
        "message=reason-1",
        "date=2024-05-17T08%3A00%3A05Z",
        "Decision: use plan Alder",
        "Check failed",
        "Birch lacks rollback",
    ] {
        assert!(
            source.body.contains(expected),
            "missing {expected}: {}",
            source.body
        );
    }
    assert!(source.body.contains("encrypted"));
    assert!(!source.body.contains("synthetic-unreadable-payload"));
    assert!(source
        .info
        .extraction_coverage
        .as_ref()
        .unwrap()
        .iter()
        .any(|part| part.scope == CoverageScope::ConversationMessages
            && part.status == CoverageStatus::Partial));
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
    let result = app
        .search_pages(PageSearchRequest {
            query: "rollback".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(result
        .pages
        .iter()
        .any(|page| page.source_id == source.info.source_id));
    drop(app);
    let reopened = Application::open(collection.path()).unwrap();
    assert_eq!(
        reopened.open_source(&source.info.source_id).unwrap().body,
        source.body
    );
}

#[test]
fn malformed_conversation_metadata_stays_uncertain_without_losing_originals() {
    let collection = tempfile::tempdir().unwrap();
    let fixture = collection.path().join("partial.json");
    let bytes = serde_json::to_vec(&json!({
        "format":"knowledge-garden-conversation-v1", "conversation_id":"partial-session",
        "messages":[
            {"id":"entry-1","role":"user","date":"2024-05-17T08:00:00Z","content":"Could Alder retain a rollback?"},
            {"id":"bad-date","role":"assistant","date":"2024-02-30T25:00:00Z","content":"I suggest Birch, but this timestamp is malformed."},
            {"content":"Tool-like wording alone cannot establish a missing role or date."}
        ]
    })).unwrap();
    fs::write(&fixture, &bytes).unwrap();
    let mut app = Application::open(collection.path().join("garden")).unwrap();
    let source = app
        .import_source(&fixture, AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(source.info.extraction, ExtractionState::PartialText);
    let malformed = source
        .body
        .lines()
        .find(|line| line.contains("message=bad-date;"))
        .unwrap();
    assert!(malformed.contains("date=unknown"), "{malformed}");
    assert!(source.body.contains("malformed timestamp"));
    assert!(source.body.contains("role=unknown"));
    assert!(source.body.contains("missing or unsupported role"));
    let missing_date = source
        .body
        .lines()
        .find(|line| line.contains("message=entry-3;"))
        .unwrap();
    assert!(missing_date.contains("date=unknown"), "{missing_date}");
    assert!(source.body.contains("absent dates remain unknown"));
    assert!(
        !source
            .body
            .contains("Message 1 lacks its original identity"),
        "a supplied entry-1 is a real ID, not a generated fallback"
    );
    assert_eq!(
        fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        bytes
    );
    drop(app);
    let reopened = Application::open(collection.path().join("garden")).unwrap();
    assert_eq!(
        reopened.open_source(&source.info.source_id).unwrap().body,
        source.body
    );
}

struct FrozenCodexIntents;

struct FrozenSingleClaim;

impl SystemOneTransport for FrozenSingleClaim {
    fn complete(&self, _key: &str, _request: &Value) -> Result<Value, ProviderError> {
        Ok(json!({"model":"typesafe/jev-frozen-claim","answers":{
            "conversation_intent_0":{"type":"choice","choice":"claim","probabilities":{"claim":0.99,"uncertain":0.01}}
        }}))
    }
}

#[test]
fn quoted_numeric_correction_becomes_an_attributed_message_without_changing_the_event() {
    let workspace = tempfile::tempdir().unwrap();
    let collection = workspace.path().join("garden");
    let provider = Arc::new(JevSemanticProvider::with_transport(
        "frozen-test-key".into(),
        Arc::new(FrozenSingleClaim),
    ));
    let mut app = Application::open_with_semantic_provider(&collection, provider).unwrap();
    let base_path = workspace.path().join("base.txt");
    let base_text = "Observation V17 recorded 12 visits.";
    fs::write(&base_path, base_text).unwrap();
    let base = app
        .import_source(&base_path, AcquisitionMethod::Picker)
        .unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    let evidence = EvidenceDraft {
        quote: base_text.into(),
        byte_start: 0,
        byte_end: base_text.len(),
        origin: "observed".into(),
        qualifier: None,
        offset_basis: None,
        source_location: None,
    };
    app.finish_semantic_job(
        job,
        Ok(KnowledgeDraft {
            entities: vec![EntityDraft {
                kind: "event".into(),
                label: "Observation V17".into(),
                evidence: evidence.clone(),
            }],
            facts: vec![FactDraft {
                subject: "Observation V17".into(),
                property: "visit_count".into(),
                value: "12 visits".into(),
                evidence,
                record_key: None,
            }],
            ..Default::default()
        }),
    )
    .unwrap();
    let event_id = app
        .open_source(&base.info.source_id)
        .unwrap()
        .knowledge_pages[0]
        .page_id
        .clone();
    let before = app.open_knowledge_page(&event_id).unwrap().markdown;
    let export = workspace.path().join("quoted-correction.json");
    let content = "The supplied memo quotes this correction: For Observation V17: The visit total should read 15, not 12.";
    let bytes = serde_json::to_vec(&json!({
        "format":"knowledge-garden-conversation-v1", "conversation_id":"quoted-correction-session",
        "messages":[{"id":"quoted-1","role":"assistant","date":"unknown","content":content}]
    }))
    .unwrap();
    fs::write(&export, &bytes).unwrap();
    let imported = app
        .import_source(&export, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&imported.info.source_id).unwrap();
    assert_eq!(
        source.info.semantic_state, "complete",
        "{:?}",
        source.info.semantic_error
    );
    let target = source
        .knowledge_pages
        .iter()
        .find(|page| page.kind == "conversation_message")
        .unwrap();
    let message = app.open_knowledge_page(&target.page_id).unwrap();
    assert!(message.markdown.contains(content));
    assert!(message.markdown.contains("assistant_message"));
    assert!(message
        .markdown
        .contains("not an independently observed external fact"));
    assert!(!source
        .knowledge_pages
        .iter()
        .any(|page| page.kind == "event"));
    assert_eq!(app.open_knowledge_page(&event_id).unwrap().markdown, before);
    assert_eq!(
        fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        bytes
    );
}

#[test]
fn malformed_codex_text_parts_expose_partial_coverage_and_keep_readable_neighbors() {
    let workspace = tempfile::tempdir().unwrap();
    let fixture = workspace.path().join("partial-rollout.jsonl");
    let rows = [
        json!({"timestamp":"2024-05-17T08:00:00Z","type":"session_meta","payload":{
            "id":"11111111-1111-4111-8111-111111111111", "session_id":"11111111-1111-4111-8111-111111111111",
            "timestamp":"2024-05-17T08:00:00Z","cwd":"/synthetic/workspace","originator":"codex_cli_rs","cli_version":"synthetic-pinned-fixture"
        }}),
        json!({"timestamp":"2024-05-17T08:00:01Z","type":"response_item","payload":{
            "type":"message","id":"mixed-1","role":"user","content":[
                {"type":"input_text","text":"Keep the readable Alder question in context."},
                {"type":"input_text","text":42}
            ]
        }}),
    ];
    let bytes = rows
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(&fixture, &bytes).unwrap();
    let collection = workspace.path().join("garden");
    let mut app = Application::open(&collection).unwrap();
    let source = app
        .import_source(&fixture, AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(
        source.info.extraction,
        ExtractionState::PartialText,
        "{}",
        source.body
    );
    assert!(source
        .body
        .contains("Keep the readable Alder question in context."));
    assert!(source.body.contains("Rollout row 2"));
    assert!(source.body.contains("unreadable"));
    assert!(!source.body.contains("text: 42"));
    assert_eq!(
        fs::read_to_string(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        bytes
    );
    assert!(source
        .info
        .extraction_coverage
        .as_ref()
        .unwrap()
        .iter()
        .any(|part| part.scope == CoverageScope::ConversationMessages
            && part.status == CoverageStatus::Partial));
    let source_id = source.info.source_id.clone();
    drop(app);
    let reopened = Application::open(collection).unwrap();
    assert_eq!(reopened.open_source(&source_id).unwrap().body, source.body);
}

impl SystemOneTransport for FrozenCodexIntents {
    fn complete(&self, _key: &str, _request: &Value) -> Result<Value, ProviderError> {
        let mut answers = serde_json::Map::new();
        // A function_call is acquired metadata, not a Jev semantic guess.
        for (index, intent) in [
            (0, "question"),
            (1, "proposal"),
            (3, "tool_report"),
            (4, "reasoning_event"),
            (5, "recorded_decision"),
        ] {
            answers.insert(
                format!("conversation_intent_{index}"),
                json!({
                    "type":"choice","choice":intent,"probabilities":{intent:0.99,"uncertain":0.01}
                }),
            );
        }
        Ok(json!({"model":"typesafe/jev-frozen-codex","answers":answers}))
    }
}

#[test]
fn acquired_tool_call_is_navigable_without_invented_model_certainty_or_success() {
    let collection = tempfile::tempdir().unwrap();
    let provider = Arc::new(JevSemanticProvider::with_transport(
        "frozen-test-key".into(),
        Arc::new(FrozenCodexIntents),
    ));
    let mut app = Application::open_with_semantic_provider(collection.path(), provider).unwrap();
    let source = app
        .import_source(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/conversations/plan-rollout-shape-v2.jsonl"),
            AcquisitionMethod::Picker,
        )
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let page = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(
        page.info.semantic_state, "complete",
        "{:?}",
        page.info.semantic_error
    );
    let call_ref = page
        .knowledge_pages
        .iter()
        .find(|page| page.title.contains("call-check-1"))
        .unwrap();
    let output_ref = page
        .knowledge_pages
        .iter()
        .find(|page| page.title.contains("output-check-1"))
        .unwrap();
    assert_ne!(call_ref.page_id, output_ref.page_id);
    let call = app.open_knowledge_page(&call_ref.page_id).unwrap();
    let output = app.open_knowledge_page(&output_ref.page_id).unwrap();
    let intent_of = |markdown: &str| {
        let header: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(markdown.split("---").nth(1).unwrap()).unwrap();
        header["facts"]
            .as_sequence()
            .unwrap()
            .iter()
            .find(|fact| fact["property"].as_str() == Some("message_intent"))
            .unwrap()["value"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(intent_of(&call.markdown), "tool_call");
    assert!(call
        .markdown
        .contains("Tool call deploy_check arguments: {\"plan\":\"Birch\"}"));
    assert!(call.markdown.contains("assistant_tool_call"));
    assert!(call
        .markdown
        .contains("not an independently observed external fact"));
    assert_eq!(intent_of(&output.markdown), "tool_report");
    assert!(output
        .markdown
        .contains("Check failed: plan Birch has no rollback."));
    assert!(page
        .info
        .semantic_decisions
        .iter()
        .all(|decision| decision.question != "conversation_intent_2"));
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
    let target = call.page_id.clone();
    drop(app);
    let reopened = Application::open(collection.path()).unwrap();
    assert_eq!(
        reopened.open_knowledge_page(&target).unwrap().markdown,
        call.markdown
    );
}
