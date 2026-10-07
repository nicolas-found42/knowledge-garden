use knowledge_garden::application::{
    AcquisitionMethod, Application, ExtractionState, PageSearchRequest,
};
use knowledge_garden::providers::{JevSemanticProvider, SystemOneTransport};
use knowledge_garden::semantic::{
    EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft, ProviderError, RelationshipDraft,
    SemanticDecision, SemanticProvider, SourceUpdateDraft, SourceUpdateRole, TagDraft,
};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::{fs, path::Path};
use tempfile::tempdir;

struct RecordedProvider {
    results: Mutex<VecDeque<std::result::Result<KnowledgeDraft, ProviderError>>>,
}

impl RecordedProvider {
    fn once(result: std::result::Result<KnowledgeDraft, ProviderError>) -> Self {
        Self {
            results: Mutex::new(VecDeque::from([result])),
        }
    }

    fn sequence(results: Vec<std::result::Result<KnowledgeDraft, ProviderError>>) -> Self {
        Self {
            results: Mutex::new(results.into()),
        }
    }
}

impl SemanticProvider for RecordedProvider {
    fn form_knowledge(
        &self,
        _source_text: &str,
    ) -> std::result::Result<KnowledgeDraft, ProviderError> {
        self.results
            .lock()
            .unwrap()
            .pop_front()
            .expect("a recorded provider response")
    }
}

struct WholeInputProvider;

impl SemanticProvider for WholeInputProvider {
    fn form_knowledge(
        &self,
        source_text: &str,
    ) -> std::result::Result<KnowledgeDraft, ProviderError> {
        Ok(KnowledgeDraft {
            entities: vec![EntityDraft {
                kind: "document".into(),
                label: "Office source document".into(),
                evidence: EvidenceDraft {
                    quote: source_text.to_owned(),
                    byte_start: 0,
                    byte_end: source_text.len(),
                    origin: "observed".into(),
                    qualifier: None,
                    offset_basis: None,
                    source_location: None,
                },
            }],
            facts: vec![FactDraft {
                subject: "Office source document".into(),
                property: "document_statement".into(),
                value: source_text.to_owned(),
                evidence: EvidenceDraft {
                    quote: source_text.to_owned(),
                    byte_start: 0,
                    byte_end: source_text.len(),
                    origin: "document_body".into(),
                    qualifier: None,
                    offset_basis: None,
                    source_location: None,
                },
                record_key: Some("whole-input-projection".into()),
            }],
            ..KnowledgeDraft::default()
        })
    }
}

struct PriorCaptureProvider(Mutex<Vec<(String, Option<String>)>>);

impl SemanticProvider for PriorCaptureProvider {
    fn form_knowledge(
        &self,
        source_text: &str,
    ) -> std::result::Result<KnowledgeDraft, ProviderError> {
        WholeInputProvider.form_knowledge(source_text)
    }

    fn form_knowledge_with_prior(
        &self,
        source_text: &str,
        prior_source_text: Option<&str>,
    ) -> std::result::Result<KnowledgeDraft, ProviderError> {
        self.0
            .lock()
            .unwrap()
            .push((source_text.to_owned(), prior_source_text.map(str::to_owned)));
        self.form_knowledge(source_text)
    }
}

#[test]
fn office_revision_semantic_job_receives_previous_projection_separately() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/office/source");
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("working-measurement.docx");
    fs::copy(fixtures.join("multi-measurement.docx"), &source).unwrap();
    let provider = Arc::new(PriorCaptureProvider(Mutex::new(Vec::new())));
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        provider.clone(),
    )
    .unwrap();

    let first = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    fs::copy(fixtures.join("multi-measurement-update.docx"), &source).unwrap();
    app.import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();

    let calls = provider.0.lock().unwrap();
    let (incoming, prior) = calls.last().unwrap();
    let prior = prior
        .as_deref()
        .expect("revisions include prior projection");
    assert!(incoming.contains("15 °C"));
    assert!(prior.contains("12 °C"));
    assert!(!incoming.contains("12 °C"));
    assert!(!prior.contains("15 °C"));
    assert_eq!(
        app.open_source(&first.info.source_id)
            .unwrap()
            .info
            .source_id,
        first.info.source_id
    );
}

fn copy_collection(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_collection(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn export_recorded_search_collection(collection: &Path) {
    let Some(destination) = std::env::var_os("KG_TICKET18_COLLECTION_EXPORT") else {
        return;
    };
    let destination = Path::new(&destination);
    assert!(
        !destination.exists(),
        "fixture export destination must be fresh"
    );
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    copy_collection(collection, destination).unwrap();
}

struct RecordedSystemOneTransport {
    response: Value,
}

struct ConfiguredSystemOneTransport {
    roles: Mutex<VecDeque<String>>,
}

struct ContextCapturingTransport(Mutex<Option<Value>>);

impl SystemOneTransport for RecordedSystemOneTransport {
    fn complete(
        &self,
        _api_key: &str,
        _request: &Value,
    ) -> std::result::Result<Value, ProviderError> {
        Ok(self.response.clone())
    }
}

impl SystemOneTransport for ConfiguredSystemOneTransport {
    fn complete(
        &self,
        _api_key: &str,
        request: &Value,
    ) -> std::result::Result<Value, ProviderError> {
        let questions = request["questions"].as_object().unwrap();
        let role = if questions.contains_key("source_update_role") {
            self.roles.lock().unwrap().pop_front().unwrap()
        } else {
            "unknown".into()
        };
        let mut answers = serde_json::Map::new();
        for (key, question) in questions {
            match question["type"].as_str().unwrap_or_default() {
                "noul" => {
                    answers.insert(key.clone(), serde_json::json!({"type":"noul","noul":0.99}));
                }
                "choice" => {
                    let criteria = question["criteria"].as_object().unwrap();
                    let choice = match key.as_str() {
                        "source_update_role" => role.as_str(),
                        "source_update_evidence" => criteria
                            .keys()
                            .find(|option| option.starts_with("span_"))
                            .map(String::as_str)
                            .unwrap_or("span_0"),
                        "source_order_date" | "source_order_revision" | "event_date" => "none",
                        "event_identity" => "event",
                        name if name.starts_with("person_identity_") => "uncertain",
                        name if name.starts_with("relation_") => {
                            if criteria.contains_key("observer") {
                                "observer"
                            } else if criteria.contains_key("location") {
                                "location"
                            } else {
                                "none"
                            }
                        }
                        _ => criteria.keys().next().map(String::as_str).unwrap_or("none"),
                    };
                    if !criteria.contains_key(choice) {
                        return Err(ProviderError::recoverable(format!(
                            "fixture transport could not answer {key}"
                        )));
                    }
                    answers.insert(
                        key.clone(),
                        serde_json::json!({"type":"choice","choice":choice,"probabilities":{choice:0.99}}),
                    );
                }
                _ => {
                    return Err(ProviderError::recoverable(
                        "Unexpected typed question.".into(),
                    ))
                }
            }
        }
        Ok(serde_json::json!({"model":"typesafe/jev-1.13-recorded","answers":answers}))
    }
}

impl SystemOneTransport for ContextCapturingTransport {
    fn complete(
        &self,
        _api_key: &str,
        request: &Value,
    ) -> std::result::Result<Value, ProviderError> {
        *self.0.lock().unwrap() = Some(request.clone());
        let mut answers = serde_json::Map::new();
        for (key, question) in request["questions"].as_object().unwrap() {
            match question["type"].as_str().unwrap_or_default() {
                "noul" => {
                    answers.insert(key.clone(), serde_json::json!({"type":"noul","noul":0.99}));
                }
                "choice" => {
                    let criteria = question["criteria"].as_object().unwrap();
                    let choice = match key.as_str() {
                        "source_update_role" => "complete_replacement",
                        "source_update_evidence" => "span_0",
                        "source_order_date" | "source_order_revision" | "event_date" => "none",
                        "event_identity" => "unclear",
                        _ => criteria.keys().next().map(String::as_str).unwrap_or("none"),
                    };
                    answers.insert(
                        key.clone(),
                        serde_json::json!({"type":"choice","choice":choice,"probabilities":{choice:0.99}}),
                    );
                }
                _ => {
                    return Err(ProviderError::permanent(
                        "Unexpected typed question.".into(),
                    ))
                }
            }
        }
        Ok(serde_json::json!({"model":"typesafe/jev-1.13-capture","answers":answers}))
    }
}

#[test]
fn jev_update_question_receives_prior_snapshot_as_distinct_state() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/office/source");
    let previous = knowledge_garden::office::extract(
        &fixtures.join("multi-measurement.docx"),
        "docx",
        "working-measurement",
    )
    .unwrap();
    let incoming = knowledge_garden::office::extract(
        &fixtures.join("multi-measurement-update.docx"),
        "docx",
        "working-measurement",
    )
    .unwrap();
    let transport = Arc::new(ContextCapturingTransport(Mutex::new(None)));
    let provider =
        JevSemanticProvider::with_transport("recorded-test-credential".into(), transport.clone());

    provider
        .form_knowledge_with_prior(&incoming.semantic_text, Some(&previous.semantic_text))
        .unwrap();

    let request = transport.0.lock().unwrap().clone().unwrap();
    assert!(request["state"]["source_text"]
        .as_str()
        .unwrap()
        .contains("15 °C"));
    assert!(request["state"]["prior_source_projection"]
        .as_str()
        .unwrap()
        .contains("12 °C"));
    assert!(!request["state"]["source_text"]
        .as_str()
        .unwrap()
        .contains("12 °C"));
    assert!(!request["state"]["prior_source_projection"]
        .as_str()
        .unwrap()
        .contains("15 °C"));
    assert!(request["questions"]["source_update_role"]["instructions"]
        .as_str()
        .unwrap()
        .contains("A provisional, estimated, or uncertain qualifier attached to one table row"));
}

fn v17_recording() -> KnowledgeDraft {
    let text = "Revision 1 dated May 18, 2024; observation V17 on May 17, 2024; observer Maya; location Riverside; 12 visits; duration 10 minutes. It possibly occurred at Riverside. A different person named Maya filed another report.";
    let evidence = |quote: &str| EvidenceDraft {
        quote: quote.to_owned(),
        byte_start: text.find(quote).unwrap(),
        byte_end: text.find(quote).unwrap() + quote.len(),
        origin: "observed".to_owned(),
        qualifier: None,
        offset_basis: None,
        source_location: None,
    };
    KnowledgeDraft {
        entities: vec![
            EntityDraft {
                kind: "event".into(),
                label: "Observation V17".into(),
                evidence: evidence("observation V17 on May 17, 2024"),
            },
            EntityDraft {
                kind: "person".into(),
                label: "Maya (observer)".into(),
                evidence: evidence("observer Maya"),
            },
            EntityDraft {
                kind: "place".into(),
                label: "Riverside".into(),
                evidence: evidence("location Riverside"),
            },
            EntityDraft {
                kind: "person".into(),
                label: "Maya (separate report)".into(),
                evidence: evidence("different person named Maya"),
            },
        ],
        facts: vec![
            FactDraft {
                subject: "Observation V17".into(),
                property: "occurred_on".into(),
                value: "May 17, 2024".into(),
                evidence: evidence("observation V17 on May 17, 2024"),
                record_key: None,
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "observer".into(),
                value: "Maya (observer)".into(),
                evidence: evidence("observer Maya"),
                record_key: None,
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "location".into(),
                value: "Riverside".into(),
                evidence: evidence("location Riverside"),
                record_key: None,
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "visit_count".into(),
                value: "12 visits".into(),
                evidence: evidence("12 visits"),
                record_key: None,
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "duration".into(),
                value: "10 minutes".into(),
                evidence: evidence("duration 10 minutes"),
                record_key: None,
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "possible_location".into(),
                value: "Riverside".into(),
                evidence: EvidenceDraft {
                    qualifier: Some("possibly".into()),
                    ..evidence("possibly occurred at Riverside")
                },
                record_key: None,
            },
        ],
        relationships: vec![
            RelationshipDraft {
                from: "Observation V17".into(),
                to: "Maya (observer)".into(),
                kind: "observed_by".into(),
                qualifier: None,
                evidence: evidence("observer Maya"),
            },
            RelationshipDraft {
                from: "Observation V17".into(),
                to: "Riverside".into(),
                kind: "possibly_occurred_at".into(),
                qualifier: Some("possibly".into()),
                evidence: evidence("possibly occurred at Riverside"),
            },
        ],
        tags: vec![],
        decisions: [
            "span_selection:choice",
            "candidate_fact:noul",
            "entity_matching:score",
            "relationship_type:choice",
            "evidence_support:noul",
        ]
        .into_iter()
        .map(|question| SemanticDecision {
            question: question.into(),
            model: "jev-1.13".into(),
            outcome: "accepted".into(),
            probability: Some(0.99),
        })
        .collect(),
        source_update: None,
    }
}

fn replacement_recording(text: &str, visits: u32, include_duration: bool) -> KnowledgeDraft {
    let evidence = |quote: &str| EvidenceDraft {
        quote: quote.to_owned(),
        byte_start: text.find(quote).unwrap(),
        byte_end: text.find(quote).unwrap() + quote.len(),
        origin: "observed".to_owned(),
        qualifier: None,
        offset_basis: None,
        source_location: None,
    };
    let mut facts = vec![
        FactDraft {
            subject: "Observation V17".into(),
            property: "occurred_on".into(),
            value: "May 17, 2024".into(),
            evidence: evidence("Observation V17 took place on May 17, 2024."),
            record_key: None,
        },
        FactDraft {
            subject: "Observation V17".into(),
            property: "visit_count".into(),
            value: format!("{visits} visits"),
            evidence: evidence(&format!("Visits: {visits}.")),
            record_key: None,
        },
    ];
    if include_duration {
        facts.push(FactDraft {
            subject: "Observation V17".into(),
            property: "duration".into(),
            value: "10 minutes".into(),
            evidence: evidence("Duration: 10 minutes."),
            record_key: None,
        });
    }
    KnowledgeDraft {
        entities: vec![
            EntityDraft {
                kind: "event".into(),
                label: "Observation V17".into(),
                evidence: evidence("Observation V17 took place on May 17, 2024."),
            },
            EntityDraft {
                kind: "person".into(),
                label: "Maya".into(),
                evidence: evidence("Observer: Maya."),
            },
            EntityDraft {
                kind: "place".into(),
                label: "Riverside".into(),
                evidence: evidence("Location: Riverside."),
            },
        ],
        facts,
        relationships: vec![
            RelationshipDraft {
                from: "Observation V17".into(),
                to: "Maya".into(),
                kind: "observed_by".into(),
                qualifier: None,
                evidence: evidence("Observer: Maya."),
            },
            RelationshipDraft {
                from: "Observation V17".into(),
                to: "Riverside".into(),
                kind: "occurred_at".into(),
                qualifier: None,
                evidence: evidence("Location: Riverside."),
            },
        ],
        source_update: Some(recorded_update(text, SourceUpdateRole::CompleteReplacement)),
        ..KnowledgeDraft::default()
    }
}

fn recorded_update(text: &str, role: SourceUpdateRole) -> SourceUpdateDraft {
    let first_line = text.lines().next().unwrap_or(text);
    let (role_start, role_end) = text
        .find(first_line)
        .map(|start| (start, start + first_line.len()))
        .unwrap_or((0, text.len()));
    let lower = text.to_lowercase();
    let revision = lower.match_indices("revision ").find_map(|(start, _)| {
        let digits = lower[start + "revision ".len()..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>();
        digits.parse::<u64>().ok().filter(|revision| *revision > 0)
    });
    let revision_quote = revision.map(|number| format!("revision {number}"));
    let revision_evidence = revision_quote.as_ref().and_then(|quote| {
        let start = text.to_lowercase().find(quote)?;
        Some(EvidenceDraft {
            quote: text[start..start + quote.len()].to_owned(),
            byte_start: start,
            byte_end: start + quote.len(),
            origin: "observed".into(),
            qualifier: None,
            offset_basis: None,
            source_location: None,
        })
    });
    let date = if text.contains("May 18, 2024") {
        Some(("May 18, 2024", "2024-05-18"))
    } else if text.contains("May 19, 2024") {
        Some(("May 19, 2024", "2024-05-19"))
    } else if text.contains("May 20, 2024") {
        Some(("May 20, 2024", "2024-05-20"))
    } else if text.contains("May 21, 2024") {
        Some(("May 21, 2024", "2024-05-21"))
    } else {
        None
    };
    let date_evidence = date.and_then(|(date, _)| {
        let start = text.find(date)?;
        Some(EvidenceDraft {
            quote: date.to_owned(),
            byte_start: start,
            byte_end: start + date.len(),
            origin: "observed".into(),
            qualifier: None,
            offset_basis: None,
            source_location: None,
        })
    });
    SourceUpdateDraft {
        role,
        evidence: EvidenceDraft {
            quote: first_line.to_owned(),
            byte_start: role_start,
            byte_end: role_end,
            origin: "observed".into(),
            qualifier: None,
            offset_basis: None,
            source_location: None,
        },
        certainty: 0.99,
        source_date: date.map(|(_, iso)| iso.to_owned()),
        source_date_evidence: date_evidence,
        source_date_certainty: if date.is_some() { 0.99 } else { 0.0 },
        source_revision: revision,
        source_revision_evidence: revision_evidence,
        source_revision_certainty: if revision.is_some() { 0.99 } else { 0.0 },
    }
}

const REVISION_1: &str = "Complete report, revision 1 dated May 18, 2024.\nObservation V17 took place on May 17, 2024.\nObserver: Maya. Location: Riverside. Visits: 12. Duration: 10 minutes.\n";
const REVISION_2: &str = "Complete report, revision 2 dated May 20, 2024. This report completely replaces revision 1 of this same observation record.\nObservation V17 took place on May 17, 2024.\nObserver: Maya. Location: Riverside. Visits: 15.\n";

#[test]
fn independently_acquired_duration_support_survives_replacement() {
    let workspace = tempdir().unwrap();
    let revision_one = workspace.path().join("revision-1.txt");
    let independent = workspace.path().join("independent-duration.txt");
    let revision_two = workspace.path().join("revision-2.txt");
    let independent_text = "Independent report dated May 19, 2024.\nObservation V17 took place on May 17, 2024.\nObserver: Maya. Location: Riverside. Visits: 12. Duration: 10 minutes.\n";
    fs::write(&revision_one, REVISION_1).unwrap();
    fs::write(&independent, independent_text).unwrap();
    fs::write(&revision_two, REVISION_2).unwrap();
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(RecordedProvider::sequence(vec![
            Ok(replacement_recording(REVISION_1, 12, true)),
            Ok(replacement_recording(independent_text, 12, true)),
            Ok(replacement_recording(REVISION_2, 15, false)),
        ])),
    )
    .unwrap();

    let first = app
        .import_source(&revision_one, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let second = app
        .import_source(&independent, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    app.import_source(&revision_two, AcquisitionMethod::Picker)
        .unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    app.set_publication_failpoint_for_test(1);
    let error = app
        .finish_semantic_job(job, Ok(replacement_recording(REVISION_2, 15, false)))
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("Injected publication interruption"));
    drop(app);
    let app = Application::open(workspace.path().join("collection")).unwrap();

    let event = app
        .open_source(&first.info.source_id)
        .unwrap()
        .knowledge_pages
        .into_iter()
        .find(|page| page.kind == "event")
        .unwrap();
    let markdown = app.open_knowledge_page(&event.page_id).unwrap().markdown;
    assert!(
        markdown.contains("- **visit count:** 15 visits"),
        "{markdown}"
    );
    assert!(
        !markdown.contains("- **visit count:** 12 visits"),
        "{markdown}"
    );
    assert!(
        markdown.contains("- **duration:** 10 minutes"),
        "{markdown}"
    );
    assert!(
        markdown.contains(&format!("source_id: {}", second.info.source_id)),
        "{markdown}"
    );
    let source_short_id = second.info.source_id.strip_prefix("source-").unwrap();
    let retained_link = format!(
        "[Retained original](../sources/{source_short_id}/{})",
        second.info.asset
    );
    assert!(markdown.contains(&retained_link), "{markdown}");
    let replaced_revision = app.open_source(&first.info.source_id).unwrap();
    assert_eq!(
        replaced_revision.info.versions_seen[0].state, "superseded",
        "a uniquely replaced source version must be durably marked for search currentness"
    );
    let relationship_section = markdown.split("## Relationships\n").nth(1).unwrap();
    assert!(
        relationship_section.contains(&format!(
            "[Source page](../sources/{source_short_id}/index.md)"
        )),
        "{relationship_section}"
    );
    let linked_original = app
        .page_path(&first.info.source_id)
        .unwrap()
        .parent()
        .unwrap()
        .join("../")
        .join("../")
        .join("sources")
        .join(source_short_id)
        .join(&second.info.asset);
    assert!(String::from_utf8(fs::read(linked_original).unwrap())
        .unwrap()
        .contains("Duration: 10 minutes."));
}

#[test]
fn reimporting_old_and_duplicate_revisions_keeps_latest_current_value() {
    let workspace = tempdir().unwrap();
    let revision_one_path = workspace.path().join("revision-1.txt");
    let revision_two_path = workspace.path().join("revision-2.txt");
    fs::write(&revision_one_path, REVISION_1).unwrap();
    fs::write(&revision_two_path, REVISION_2).unwrap();
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(RecordedProvider::sequence(vec![
            Ok(replacement_recording(REVISION_1, 12, true)),
            Ok(replacement_recording(REVISION_2, 15, false)),
        ])),
    )
    .unwrap();
    let first = app
        .import_source(&revision_one_path, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    app.import_source(&revision_two_path, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let source_count = app.list_sources(0).unwrap().sources.len();

    app.import_source(&revision_one_path, AcquisitionMethod::Picker)
        .unwrap();
    app.import_source(&revision_two_path, AcquisitionMethod::Picker)
        .unwrap();

    assert_eq!(app.list_sources(0).unwrap().sources.len(), source_count);
    let event = app
        .open_source(&first.info.source_id)
        .unwrap()
        .knowledge_pages
        .into_iter()
        .find(|page| page.kind == "event")
        .unwrap();
    let markdown = app.open_knowledge_page(&event.page_id).unwrap().markdown;
    assert!(
        markdown.contains("- **visit count:** 15 visits"),
        "{markdown}"
    );
    assert!(
        !markdown.contains("- **visit count:** 12 visits"),
        "{markdown}"
    );
    assert!(
        !markdown.contains("- **duration:** 10 minutes"),
        "{markdown}"
    );
}

#[test]
fn undated_explicit_replacement_wins_by_arrival_and_records_uncertainty() {
    let workspace = tempdir().unwrap();
    let first_text = "Complete same-record report for observation V17. No source date or numbered revision is provided.\nObservation V17 took place on May 17, 2024.\nObserver: Maya. Location: Riverside. Visits: 12.\n";
    let second_text = "Complete same-record report for observation V17. This replaces the previously supplied undated report in its entirety. No source date or numbered revision is provided.\nObservation V17 took place on May 17, 2024.\nObserver: Maya. Location: Riverside. Visits: 15.\n";
    let first_path = workspace.path().join("undated-1.txt");
    let second_path = workspace.path().join("undated-2.txt");
    fs::write(&first_path, first_text).unwrap();
    fs::write(&second_path, second_text).unwrap();
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(RecordedProvider::sequence(vec![
            Ok(replacement_recording(first_text, 12, false)),
            Ok(replacement_recording(second_text, 15, false)),
        ])),
    )
    .unwrap();
    let first = app
        .import_source(&first_path, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let second = app
        .import_source(&second_path, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();

    let second_current = app.open_source(&second.info.source_id).unwrap();
    assert!(second_current.info.ordering_uncertain);
    let current_version_id = second_current.info.current_version_id.as_ref().unwrap();
    let current = second_current
        .info
        .versions_seen
        .iter()
        .find(|version| &version.source_version_id == current_version_id)
        .unwrap();
    assert_eq!(current.source_date, None);
    assert_eq!(current.order_basis, "arrival_fallback");
    let event = app
        .open_source(&first.info.source_id)
        .unwrap()
        .knowledge_pages
        .into_iter()
        .find(|page| page.kind == "event")
        .unwrap();
    let markdown = app.open_knowledge_page(&event.page_id).unwrap().markdown;
    assert!(
        markdown.contains("- **visit count:** 15 visits"),
        "{markdown}"
    );
    assert!(
        !markdown.contains("- **visit count:** 12 visits"),
        "{markdown}"
    );
}

#[test]
fn reopening_replays_a_durable_interrupted_publication_as_one_complete_generation() {
    let workspace = tempdir().unwrap();
    let source_path = workspace.path().join("V17.txt");
    fs::write(&source_path, REVISION_1).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open_with_semantic_provider(
        &collection,
        Arc::new(RecordedProvider::once(Ok(replacement_recording(
            REVISION_1, 12, true,
        )))),
    )
    .unwrap();
    let source = app
        .import_source(&source_path, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let event = app
        .open_source(&source.info.source_id)
        .unwrap()
        .knowledge_pages
        .into_iter()
        .find(|page| page.kind == "event")
        .unwrap();
    let current_page_path = collection.join(&event.path);
    let current_page = fs::read_to_string(&current_page_path).unwrap();
    let next_page = current_page.replace("12 visits", "15 visits");
    let mut next_info = app.open_source(&source.info.source_id).unwrap().info;
    let next_version = "a".repeat(64);
    next_info.current_version_id = Some(next_version.clone());
    next_info.versions_seen[0].state = "superseded".into();
    let mut next_version_record = next_info.versions_seen[0].clone();
    next_version_record.source_version_id = next_version.clone();
    next_version_record.sha256 = next_version.clone();
    next_version_record.state = "complete".into();
    next_version_record.source_revision = Some(2);
    next_info.versions_seen.push(next_version_record);
    let source_body = app.open_source(&source.info.source_id).unwrap().body;
    let next_source = format!(
        "---\n{}---\n\n{}",
        serde_yaml_ng::to_string(&next_info).unwrap(),
        source_body
    );

    let transaction = collection.join(".staging/transactions/interrupted-generation");
    let staged = transaction.join("files");
    fs::create_dir_all(&staged).unwrap();
    fs::write(staged.join("0000"), next_page).unwrap();
    fs::write(staged.join("0001"), next_source).unwrap();
    fs::write(
        transaction.join("manifest.json"),
        serde_json::json!({
            "entries": [
                { "destination": event.path, "staged": "files/0000" },
                { "destination": format!("sources/{}/index.md", &source.info.source_id["source-".len()..]), "staged": "files/0001" }
            ]
        })
        .to_string(),
    )
    .unwrap();
    drop(app);

    let recovered = Application::open(&collection).unwrap();
    let recovered_source = recovered.open_source(&source.info.source_id).unwrap();
    assert_eq!(
        recovered_source.info.current_version_id.as_deref(),
        Some(next_version.as_str())
    );
    let recovered_page = recovered.open_knowledge_page(&event.page_id).unwrap();
    assert!(recovered_page.markdown.contains("15 visits"));
    assert!(!collection
        .join(".staging/transactions/interrupted-generation")
        .exists());
}

#[test]
fn application_recovery_replays_an_actual_interrupted_replacement_transaction() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("working-report.txt");
    fs::write(&source, REVISION_1).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open_with_semantic_provider(
        &collection,
        Arc::new(RecordedProvider::sequence(vec![
            Ok(replacement_recording(REVISION_1, 12, true)),
            Ok(replacement_recording(REVISION_2, 15, false)),
        ])),
    )
    .unwrap();
    let first = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    fs::write(&source, REVISION_2).unwrap();
    app.import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    app.set_publication_failpoint_for_test(1);
    let error = app
        .finish_semantic_job(job, Ok(replacement_recording(REVISION_2, 15, false)))
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("Injected publication interruption"));
    drop(app);

    let app = Application::open(&collection).unwrap();
    let current = app.open_source(&first.info.source_id).unwrap();
    assert_eq!(current.info.semantic_state, "complete");
    assert_eq!(
        current.info.current_version_id.as_deref(),
        Some(current.info.sha256.as_str())
    );
    assert_eq!(
        fs::read(app.original_path(&first.info.source_id).unwrap()).unwrap(),
        REVISION_2.as_bytes()
    );
    let event = current
        .knowledge_pages
        .iter()
        .find(|page| page.kind == "event")
        .unwrap();
    let markdown = app.open_knowledge_page(&event.page_id).unwrap().markdown;
    assert_eq!(markdown.matches("value: 15 visits").count(), 2);
    assert_eq!(markdown.matches("- **visit count:** 15 visits").count(), 1);
    assert!(!markdown.contains("value: 12 visits"));
    let results = app
        .search_pages(PageSearchRequest {
            query: "15 visits".into(),
            ..PageSearchRequest::default()
        })
        .unwrap();
    assert!(results
        .pages
        .iter()
        .any(|page| page.page_id == event.page_id));
    assert!(fs::read_dir(collection.join(".staging/transactions"))
        .unwrap()
        .next()
        .is_none());
}

#[test]
fn complete_newer_revision_updates_current_knowledge_in_place() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("V17.txt");
    fs::write(&source, REVISION_1).unwrap();
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(RecordedProvider::sequence(vec![
            Ok(replacement_recording(REVISION_1, 12, true)),
            Ok(replacement_recording(REVISION_2, 15, false)),
        ])),
    )
    .unwrap();

    let revision_one = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    fs::write(&source, REVISION_2).unwrap();
    let revision_two = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();

    assert_eq!(revision_two.info.source_id, revision_one.info.source_id);
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
    let current = app.open_source(&revision_one.info.source_id).unwrap();
    let event = current
        .knowledge_pages
        .into_iter()
        .find(|page| page.kind == "event")
        .unwrap();
    let markdown = app.open_knowledge_page(&event.page_id).unwrap().markdown;
    assert!(markdown.contains("value: 15 visits"), "{}", markdown);
    assert!(!markdown.contains("value: 12 visits"));
    assert!(!markdown.contains("value: 10 minutes"));
    assert!(markdown.contains("value: May 17, 2024"));
    assert!(markdown.contains("[Maya](../pages/"));
    assert!(markdown.contains("[Riverside](../pages/"));

    let current_hits = app
        .search_pages(PageSearchRequest {
            query: "15 visits".into(),
            ..PageSearchRequest::default()
        })
        .unwrap()
        .pages
        .into_iter()
        .filter(|page| page.page_type == "knowledge")
        .collect::<Vec<_>>();
    assert_eq!(current_hits.len(), 1);
    assert_eq!(current_hits[0].page_id, event.page_id);

    let stale_hits = app
        .search_pages(PageSearchRequest {
            query: "12 visits".into(),
            ..PageSearchRequest::default()
        })
        .unwrap()
        .pages
        .into_iter()
        .filter(|page| page.page_type == "knowledge")
        .collect::<Vec<_>>();
    assert!(stale_hits.is_empty(), "{stale_hits:?}\n{markdown}");
}

#[test]
fn search_returns_frozen_page_set_from_titles_tags_keyword_and_current_metadata() {
    // Independent expected labels are frozen in ticket-18-expected-results.md.
    let workspace = tempdir().unwrap();
    let source_text = "Observation V17 took place at Riverside on May 17, 2024. Maya observed the event. 12 visits were reported. This is fieldwork.";
    let source = workspace.path().join("V17 research.txt");
    fs::write(&source, source_text).unwrap();
    let evidence = |quote: &str| EvidenceDraft {
        quote: quote.to_owned(),
        byte_start: source_text.find(quote).unwrap(),
        byte_end: source_text.find(quote).unwrap() + quote.len(),
        origin: "observed".into(),
        qualifier: None,
        offset_basis: None,
        source_location: None,
    };
    let draft = KnowledgeDraft {
        entities: vec![
            EntityDraft {
                kind: "event".into(),
                label: "Observation V17".into(),
                evidence: evidence("Observation V17 took place at Riverside on May 17, 2024"),
            },
            EntityDraft {
                kind: "place".into(),
                label: "Riverside".into(),
                evidence: evidence("Riverside"),
            },
            EntityDraft {
                kind: "person".into(),
                label: "Maya".into(),
                evidence: evidence("Maya observed the event"),
            },
        ],
        facts: vec![
            FactDraft {
                subject: "Observation V17".into(),
                property: "occurred_on".into(),
                value: "May 17, 2024".into(),
                evidence: evidence("May 17, 2024"),
                record_key: None,
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "location".into(),
                value: "Riverside".into(),
                evidence: evidence("Riverside"),
                record_key: None,
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "observer".into(),
                value: "Maya".into(),
                evidence: evidence("Maya observed the event"),
                record_key: None,
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "visit_count".into(),
                value: "12 visits".into(),
                evidence: evidence("12 visits were reported"),
                record_key: None,
            },
        ],
        tags: vec![
            TagDraft {
                subject: "Observation V17".into(),
                label: "Riverside".into(),
                evidence: evidence("Riverside"),
            },
            TagDraft {
                subject: "Observation V17".into(),
                label: "fieldwork".into(),
                evidence: evidence("This is fieldwork"),
            },
        ],
        ..KnowledgeDraft::default()
    };
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(RecordedProvider::once(Ok(draft))),
    )
    .unwrap();
    let source_page = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();

    let combined = app
        .search_pages(PageSearchRequest {
            query: "Riverside".into(),
            tags: vec!["#fieldwork".into()],
            date_from: Some("2024-05-01".into()),
            date_to: Some("2024-05-31".into()),
            format: Some("txt".into()),
            processing_status: Some("complete".into()),
            offset: 0,
        })
        .unwrap();
    assert_eq!(
        combined
            .pages
            .iter()
            .map(|page| page.title.as_str())
            .collect::<Vec<_>>(),
        ["Observation V17"]
    );
    assert!(combined.pages[0].excerpt.contains("Riverside"));
    assert_eq!(combined.pages[0].format, "txt");
    assert_eq!(combined.pages[0].processing_status, "complete");
    assert!(combined.pages[0].tags.iter().any(|tag| tag == "fieldwork"));
    assert!(combined.available_tags.contains(&"fieldwork".to_owned()));
    assert!(combined.available_formats.contains(&"txt".to_owned()));
    let markdown = app
        .open_knowledge_page(&combined.pages[0].page_id)
        .unwrap()
        .markdown;
    assert!(markdown.contains("normalized: fieldwork"));
    assert!(markdown.contains("source_version_id:"));

    let by_keyword = app
        .search_pages(PageSearchRequest {
            query: "12 visits".into(),
            ..PageSearchRequest::default()
        })
        .unwrap();
    assert!(by_keyword
        .pages
        .iter()
        .any(|page| page.title == "Observation V17"));
    assert!(by_keyword
        .pages
        .iter()
        .any(|page| page.page_id == source_page.info.page_id));
    let by_title = app
        .search_pages(PageSearchRequest {
            query: "Observation V17".into(),
            ..PageSearchRequest::default()
        })
        .unwrap();
    assert!(by_title
        .pages
        .iter()
        .any(|page| page.title == "Observation V17"));
    let by_tag = app
        .search_pages(PageSearchRequest {
            tags: vec!["#fieldwork".into()],
            ..PageSearchRequest::default()
        })
        .unwrap();
    assert_eq!(
        by_tag
            .pages
            .iter()
            .map(|page| page.title.as_str())
            .collect::<Vec<_>>(),
        ["Observation V17"]
    );
    let no_match = app
        .search_pages(PageSearchRequest {
            query: "Atlantis no such observation".into(),
            ..PageSearchRequest::default()
        })
        .unwrap();
    assert!(no_match.pages.is_empty());

    drop(app);
    export_recorded_search_collection(&workspace.path().join("collection"));
    fs::remove_dir_all(workspace.path().join("collection/.derived")).unwrap();
    let rebuilt = Application::open(workspace.path().join("collection")).unwrap();
    let after_restart = rebuilt
        .search_pages(PageSearchRequest {
            query: "Riverside".into(),
            tags: vec!["fieldwork".into()],
            ..PageSearchRequest::default()
        })
        .unwrap();
    assert_eq!(
        after_restart
            .pages
            .iter()
            .map(|page| page.title.as_str())
            .collect::<Vec<_>>(),
        ["Observation V17"]
    );
}

#[test]
fn search_paginates_a_bounded_set_of_page_results() {
    let workspace = tempdir().unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    for index in 0..51 {
        let source = workspace.path().join(format!("Page {index:02}.txt"));
        fs::write(&source, format!("Searchable fixture source {index:02}.")).unwrap();
        app.import_source(&source, AcquisitionMethod::Picker)
            .unwrap();
    }
    let first = app.search_pages(PageSearchRequest::default()).unwrap();
    assert_eq!(first.pages.len(), 50);
    assert_eq!(first.next_offset, Some(50));
    let second = app
        .search_pages(PageSearchRequest {
            offset: first.next_offset.unwrap(),
            ..PageSearchRequest::default()
        })
        .unwrap();
    assert_eq!(second.pages.len(), 1);
    assert_eq!(second.next_offset, None);
    assert_ne!(first.pages[0].page_id, second.pages[0].page_id);
}

#[test]
fn import_forms_linked_knowledge_pages_with_typed_facts_qualifiers_and_exact_evidence() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("V17.txt");
    let text = "Revision 1 dated May 18, 2024; observation V17 on May 17, 2024; observer Maya; location Riverside; 12 visits; duration 10 minutes. It possibly occurred at Riverside. A different person named Maya filed another report.";
    fs::write(&source, text).unwrap();
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(RecordedProvider::once(Ok(v17_recording()))),
    )
    .unwrap();

    let queued = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(queued.info.semantic_state, "pending");
    app.resume_due_semantic_jobs().unwrap();
    let page = app.open_source(&queued.info.source_id).unwrap();

    assert_eq!(page.info.semantic_state, "complete");
    assert!(page.body.contains("Observation V17"));
    assert!(page.body.contains("Maya (observer)"));
    assert!(page.body.contains("Maya (separate report)"));
    let event = page
        .knowledge_pages
        .iter()
        .find(|candidate| candidate.title == "Observation V17")
        .unwrap();
    let event_page = app.open_knowledge_page(&event.page_id).unwrap();
    assert!(event_page.markdown.contains("May 17, 2024"));
    assert!(event_page.markdown.contains("12 visits"));
    assert!(event_page.markdown.contains("10 minutes"));
    assert!(event_page.markdown.contains("possibly"));
    assert!(event_page.markdown.contains("observer Maya"));
    assert!(event_page.markdown.contains("byte_start:"));
    assert!(event_page.markdown.contains("line_start: 1"));
    assert!(event_page.markdown.contains("[Source page]"));
    for page in page.knowledge_pages {
        let path = workspace
            .path()
            .join("collection")
            .join("pages")
            .join(format!("{}.md", page.page_id));
        assert!(path.is_file());
    }
}

#[test]
fn application_keeps_two_same_name_observers_as_distinct_evidence_backed_entities() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("two-mayas.txt");
    let text =
        "Maya observed Observation V42. A different person named Maya observed Observation V42.";
    fs::write(&source, text).unwrap();
    let evidence = |quote: &str| EvidenceDraft {
        quote: quote.to_owned(),
        byte_start: text.find(quote).unwrap(),
        byte_end: text.find(quote).unwrap() + quote.len(),
        origin: "observed".to_owned(),
        qualifier: None,
        offset_basis: None,
        source_location: None,
    };
    let event = "Observation V42";
    let maya_one = "Maya (observer; mention 1)";
    let maya_two = "Maya (observer; mention 2)";
    let draft = KnowledgeDraft {
        entities: vec![
            EntityDraft {
                kind: "event".into(),
                label: event.into(),
                evidence: evidence("Observation V42"),
            },
            EntityDraft {
                kind: "person".into(),
                label: maya_one.into(),
                evidence: evidence("Maya observed"),
            },
            EntityDraft {
                kind: "person".into(),
                label: maya_two.into(),
                evidence: evidence("different person named Maya"),
            },
        ],
        facts: vec![
            FactDraft {
                subject: event.into(),
                property: "observer".into(),
                value: "Maya".into(),
                evidence: evidence("Maya observed"),
                record_key: None,
            },
            FactDraft {
                subject: event.into(),
                property: "observer".into(),
                value: "Maya".into(),
                evidence: evidence("different person named Maya observed"),
                record_key: None,
            },
        ],
        relationships: vec![
            RelationshipDraft {
                from: event.into(),
                to: maya_one.into(),
                kind: "observed_by".into(),
                qualifier: None,
                evidence: evidence("Maya observed"),
            },
            RelationshipDraft {
                from: event.into(),
                to: maya_two.into(),
                kind: "observed_by".into(),
                qualifier: None,
                evidence: evidence("different person named Maya observed"),
            },
        ],
        tags: vec![],
        decisions: vec![SemanticDecision {
            question: "person_identity".into(),
            model: "jev-1.13".into(),
            outcome: "distinct_identity".into(),
            probability: Some(0.98),
        }],
        source_update: None,
    };
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(RecordedProvider::once(Ok(draft))),
    )
    .unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let source_page = app.open_source(&imported.info.source_id).unwrap();
    let people = source_page
        .knowledge_pages
        .iter()
        .filter(|page| page.kind == "person")
        .collect::<Vec<_>>();
    assert_eq!(people.len(), 2);
    assert_ne!(people[0].page_id, people[1].page_id);
    for person in people {
        let markdown = app.open_knowledge_page(&person.page_id).unwrap().markdown;
        assert!(markdown.contains("observed by"));
        assert!(markdown.contains("Observation V42"));
    }
    let event_page = source_page
        .knowledge_pages
        .iter()
        .find(|page| page.title == event)
        .unwrap();
    let markdown = app
        .open_knowledge_page(&event_page.page_id)
        .unwrap()
        .markdown;
    assert!(markdown.contains("mention 1"));
    assert!(markdown.contains("mention 2"));
}

#[test]
fn document_pages_are_source_scoped_and_fact_record_keys_preserve_granular_ids() {
    fn draft(text: &str, first_value: &str, second_value: &str) -> KnowledgeDraft {
        let evidence = |quote: &str| EvidenceDraft {
            quote: quote.to_owned(),
            byte_start: text.find(quote).unwrap(),
            byte_end: text.find(quote).unwrap() + quote.len(),
            origin: "observed".into(),
            qualifier: None,
            offset_basis: None,
            source_location: None,
        };
        KnowledgeDraft {
            entities: vec![EntityDraft {
                kind: "document".into(),
                label: "Imported document".into(),
                evidence: evidence(text),
            }],
            facts: vec![
                FactDraft {
                    subject: "Imported document".into(),
                    property: "statement".into(),
                    value: first_value.into(),
                    evidence: evidence(first_value),
                    record_key: Some("section:summary".into()),
                },
                FactDraft {
                    subject: "Imported document".into(),
                    property: "statement".into(),
                    value: second_value.into(),
                    evidence: evidence(second_value),
                    record_key: Some("section:method".into()),
                },
            ],
            source_update: Some(recorded_update(text, SourceUpdateRole::CompleteReplacement)),
            ..KnowledgeDraft::default()
        }
    }

    let workspace = tempdir().unwrap();
    let first_path = workspace.path().join("first.txt");
    let second_path = workspace.path().join("second.txt");
    let first_text = "Summary: Alpha finding. Method: Surveyed 8 sites.";
    let second_text = "Summary: Beta finding. Method: Surveyed 3 sites.";
    let revised_text = "Summary: Revised alpha finding. Method: Surveyed 8 sites.";
    fs::write(&first_path, first_text).unwrap();
    fs::write(&second_path, second_text).unwrap();
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(RecordedProvider::sequence(vec![
            Ok(draft(first_text, "Alpha finding", "Surveyed 8 sites")),
            Ok(draft(second_text, "Beta finding", "Surveyed 3 sites")),
            Ok(draft(
                revised_text,
                "Revised alpha finding",
                "Surveyed 8 sites",
            )),
        ])),
    )
    .unwrap();
    let first = app
        .import_source(&first_path, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let second = app
        .import_source(&second_path, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let first_page = app
        .open_source(&first.info.source_id)
        .unwrap()
        .knowledge_pages
        .first()
        .unwrap()
        .page_id
        .clone();
    let second_page = app
        .open_source(&second.info.source_id)
        .unwrap()
        .knowledge_pages
        .first()
        .unwrap()
        .page_id
        .clone();
    assert_ne!(first_page, second_page);
    let first_markdown = app.open_knowledge_page(&first_page).unwrap().markdown;
    let ids_before = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(
        first_markdown
            .strip_prefix("---\n")
            .unwrap()
            .split_once("\n---\n")
            .unwrap()
            .0,
    )
    .unwrap();
    let facts = ids_before["facts"].as_sequence().unwrap();
    assert_eq!(facts.len(), 2);
    let summary_id = facts
        .iter()
        .find(|fact| fact["record_key"] == "section:summary")
        .unwrap()["fact_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let method_id = facts
        .iter()
        .find(|fact| fact["record_key"] == "section:method")
        .unwrap()["fact_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_ne!(summary_id, method_id);

    fs::write(&first_path, revised_text).unwrap();
    app.import_source(&first_path, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let revised = app.open_knowledge_page(&first_page).unwrap().markdown;
    assert!(revised.contains(&summary_id));
    assert!(revised.contains(&method_id));
    assert!(revised.contains("value: Revised alpha finding"));
}

#[test]
fn semantic_evidence_offsets_are_measured_in_original_utf8_bytes_after_a_bom() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("bom-report.txt");
    let bytes = b"\xef\xbb\xbfObservation V50 had 2 visits.";
    fs::write(&source, bytes).unwrap();
    let text = "\u{feff}Observation V50 had 2 visits.";
    let evidence = |quote: &str| EvidenceDraft {
        quote: quote.to_owned(),
        byte_start: text.find(quote).unwrap(),
        byte_end: text.find(quote).unwrap() + quote.len(),
        origin: "observed".to_owned(),
        qualifier: None,
        offset_basis: None,
        source_location: None,
    };
    let draft = KnowledgeDraft {
        entities: vec![EntityDraft {
            kind: "event".into(),
            label: "Observation V50".into(),
            evidence: evidence("Observation V50"),
        }],
        facts: vec![FactDraft {
            subject: "Observation V50".into(),
            property: "visit_count".into(),
            value: "2 visits".into(),
            evidence: evidence("2 visits"),
            record_key: None,
        }],
        ..KnowledgeDraft::default()
    };
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(RecordedProvider::once(Ok(draft))),
    )
    .unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let event = app
        .open_source(&imported.info.source_id)
        .unwrap()
        .knowledge_pages
        .remove(0);
    let markdown = app.open_knowledge_page(&event.page_id).unwrap().markdown;
    assert!(markdown.contains("byte_start: 3"));
    assert!(markdown.contains(&format!("byte_end: {}", 3 + "Observation V50".len())));
    assert_eq!(
        fs::read(app.original_path(&imported.info.source_id).unwrap()).unwrap(),
        bytes
    );
}

#[test]
fn jev_role_judgments_keep_same_path_supplements_and_uncertain_updates_non_destructive() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("V17.txt");
    let original = "Revision 1 dated May 18, 2024. Observation V17 took place on May 17, 2024. Observer: Maya. Location: Riverside. Visits: 12. Duration: 10 minutes.";
    fs::write(&source, original).unwrap();
    let transport = ConfiguredSystemOneTransport {
        roles: Mutex::new(
            [
                "unknown",
                "supplement",
                "conditional",
                "targeted_correction",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        ),
    };
    let provider =
        JevSemanticProvider::with_transport("recorded-test-key".into(), Arc::new(transport));
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(provider),
    )
    .unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let baseline = app.open_source(&imported.info.source_id).unwrap();
    assert_eq!(baseline.info.semantic_state, "complete");
    let baseline_event = baseline
        .knowledge_pages
        .iter()
        .find(|page| page.kind == "event")
        .unwrap();
    assert!(app
        .open_knowledge_page(&baseline_event.page_id)
        .unwrap()
        .markdown
        .contains("value: 12 visits"));

    for (number, (role, text)) in [
        (
            "supplement",
            "Supplement: an independent note mentions 14 visits for Observation V17.",
        ),
        (
            "conditional",
            "Conditional proposal: if verified, Observation V17 may have 14 visits.",
        ),
        (
            "targeted_correction",
            "Targeted correction only: the count for Observation V17 is 14 visits.",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            role,
            ["supplement", "conditional", "targeted_correction"][number]
        );
        fs::write(&source, text).unwrap();
        app.import_source(&source, AcquisitionMethod::Picker)
            .unwrap();
        app.resume_due_semantic_jobs().unwrap();
        let current = app.open_source(&imported.info.source_id).unwrap();
        let event = current
            .knowledge_pages
            .iter()
            .find(|page| page.kind == "event")
            .unwrap();
        let markdown = app.open_knowledge_page(&event.page_id).unwrap().markdown;
        assert!(markdown.contains("value: 12 visits"), "{role}: {markdown}");
        assert!(!markdown.contains("value: 14 visits"), "{role}: {markdown}");
        assert_eq!(
            current.info.update_status.as_deref(),
            Some("uncertain"),
            "{role}"
        );
        assert_eq!(current.info.semantic_state, "complete");
    }
}

#[test]
fn complete_revision_does_not_withdraw_an_independent_same_numbered_supplement() {
    let workspace = tempdir().unwrap();
    let base = workspace.path().join("revision-1.txt");
    let timing = workspace.path().join("independent-revision-one.txt");
    let revised = workspace.path().join("revision-2.txt");
    fs::write(
        &base,
        "Complete report revision 1 dated May 18, 2024. Observation R9 took place on May 17, 2024. Visits: 12.",
    )
    .unwrap();
    fs::write(
        &timing,
        "Independent timing supplement revision 1: Observation R9 lasted 10 minutes.",
    )
    .unwrap();
    fs::write(
        &revised,
        "Complete report revision 2 dated May 19, 2024. Observation R9 took place on May 17, 2024. Visits: 15.",
    )
    .unwrap();
    let transport = ConfiguredSystemOneTransport {
        roles: Mutex::new(
            ["unknown", "supplement", "complete_replacement"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        ),
    };
    let provider =
        JevSemanticProvider::with_transport("recorded-test-key".into(), Arc::new(transport));
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(provider),
    )
    .unwrap();
    let base_page = app.import_source(&base, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let timing_page = app
        .import_source(&timing, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    app.import_source(&revised, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();

    let current = app.open_source(&base_page.info.source_id).unwrap();
    let event = current
        .knowledge_pages
        .iter()
        .find(|page| page.kind == "event")
        .unwrap();
    let markdown = app.open_knowledge_page(&event.page_id).unwrap().markdown;
    assert!(markdown.contains("value: 15 visits"), "{markdown}");
    assert!(!markdown.contains("value: 12 visits"), "{markdown}");
    assert!(markdown.contains("value: 10 minutes"), "{markdown}");
    assert!(markdown.contains(&timing_page.info.source_id), "{markdown}");
    let timing_source = app.open_source(&timing_page.info.source_id).unwrap();
    assert_ne!(timing_source.info.versions_seen[0].state, "superseded");
}

#[test]
fn malformed_recorded_jev_answers_leave_the_public_application_in_recoverable_pending_state() {
    let source_text = "Maya observed Observation V50 at Riverside on May 1, 2024.";
    let valid = serde_json::json!({
        "model":"typesafe/jev-1.13-fixture",
        "answers":{
            "event_identity":{"type":"choice","choice":"event","probabilities":{"event":0.95,"not_event":0.03,"unclear":0.02}},
            "source_update_role":{"type":"choice","choice":"supplement","probabilities":{"supplement":0.99}},
            "source_update_evidence":{"type":"choice","choice":"span_0","probabilities":{"span_0":0.99}},
            "source_order_date":{"type":"choice","choice":"none","probabilities":{"none":0.99}},
            "source_order_revision":{"type":"choice","choice":"none","probabilities":{"none":0.99}},
            "relation_0":{"type":"choice","choice":"observer","probabilities":{"observer":0.94,"attendee":0.01,"location":0.01,"other":0.03,"none":0.01}},
            "relation_3":{"type":"choice","choice":"location","probabilities":{"location":0.96,"observer":0.01,"attendee":0.01,"other":0.01,"none":0.01}},
            "support_2":{"type":"noul","noul":0.97},
            "support_3":{"type":"noul","noul":0.97},
            "event_date":{"type":"choice","choice":"date_2","probabilities":{"date_2":0.96,"none":0.04}}
        }
    });
    let mut malformed = Vec::new();
    for (name, mutate) in [
        ("missing date choice", 0),
        ("wrong date answer type", 1),
        ("unknown date option", 2),
        ("missing date probability", 3),
        ("out of range date probability", 4),
        ("missing identity probability", 5),
        ("invalid identity choice", 6),
        ("missing observer probability", 7),
        ("invalid relation choice", 8),
        ("missing place support", 9),
        ("wrong place support type", 10),
        ("out of range place support", 11),
        ("missing source-update role", 12),
        ("wrong source-update role type", 13),
        ("invalid source-update role", 14),
        ("missing source-update role probability", 15),
        ("missing source-update evidence", 16),
        ("invalid source-update evidence", 17),
        ("missing source date choice", 18),
        ("missing source revision probability", 19),
    ] {
        let mut response = valid.clone();
        let answers = response["answers"].as_object_mut().unwrap();
        match mutate {
            0 => {
                answers.remove("event_date");
            }
            1 => {
                answers["event_date"]["type"] = "noul".into();
            }
            2 => {
                answers["event_date"]["choice"] = "date_99".into();
            }
            3 => {
                answers["event_date"]["probabilities"]
                    .as_object_mut()
                    .unwrap()
                    .remove("date_2");
            }
            4 => {
                answers["event_date"]["probabilities"]["date_2"] = 1.1.into();
            }
            5 => {
                answers["event_identity"]["probabilities"]
                    .as_object_mut()
                    .unwrap()
                    .remove("event");
            }
            6 => {
                answers["event_identity"]["choice"] = "maybe".into();
            }
            7 => {
                answers["relation_0"]["probabilities"]
                    .as_object_mut()
                    .unwrap()
                    .remove("observer");
            }
            8 => {
                answers["relation_0"]["choice"] = "maybe".into();
            }
            9 => {
                answers.remove("support_3");
            }
            10 => {
                answers["support_3"]["type"] = "choice".into();
            }
            11 => {
                answers["support_3"]["noul"] = 1.1.into();
            }
            12 => {
                answers.remove("source_update_role");
            }
            13 => {
                answers["source_update_role"]["type"] = "noul".into();
            }
            14 => {
                answers["source_update_role"]["choice"] = "suggested_replacement".into();
            }
            15 => {
                answers["source_update_role"]["probabilities"]
                    .as_object_mut()
                    .unwrap()
                    .remove("supplement");
            }
            16 => {
                answers.remove("source_update_evidence");
            }
            17 => {
                answers["source_update_evidence"]["choice"] = "span_99".into();
            }
            18 => {
                answers.remove("source_order_date");
            }
            19 => {
                answers["source_order_revision"]["probabilities"]
                    .as_object_mut()
                    .unwrap()
                    .remove("none");
            }
            _ => unreachable!(),
        }
        malformed.push((name, response));
    }

    for (case, response) in malformed {
        let workspace = tempdir().unwrap();
        let source = workspace.path().join("typed-response.txt");
        fs::write(&source, source_text).unwrap();
        let provider = JevSemanticProvider::with_transport(
            "recorded-test-credential".into(),
            Arc::new(RecordedSystemOneTransport { response }),
        );
        let mut app = Application::open_with_semantic_provider(
            workspace.path().join("collection"),
            Arc::new(provider),
        )
        .unwrap();
        let imported = app
            .import_source(&source, AcquisitionMethod::Picker)
            .unwrap();
        app.resume_due_semantic_jobs().unwrap();
        let current = app.open_source(&imported.info.source_id).unwrap();
        assert_eq!(current.info.semantic_state, "pending", "{case}");
        assert!(current.info.semantic_error.is_some(), "{case}");
        assert!(current.knowledge_pages.is_empty(), "{case}");
        assert!(current.body.contains(source_text), "{case}");
    }

    let workspace = tempdir().unwrap();
    let source = workspace.path().join("typed-response-valid.txt");
    fs::write(&source, source_text).unwrap();
    let provider = JevSemanticProvider::with_transport(
        "recorded-test-credential".into(),
        Arc::new(RecordedSystemOneTransport { response: valid }),
    );
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("valid-collection"),
        Arc::new(provider),
    )
    .unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let current = app.open_source(&imported.info.source_id).unwrap();
    assert_eq!(
        current.info.semantic_state, "complete",
        "{:?}",
        current.info.semantic_error
    );
    assert!(current
        .knowledge_pages
        .iter()
        .any(|page| page.kind == "event"));
    let event = current
        .knowledge_pages
        .iter()
        .find(|page| page.kind == "event")
        .unwrap();
    let markdown = app.open_knowledge_page(&event.page_id).unwrap().markdown;
    assert!(markdown.contains("value: May 1, 2024"));
    assert!(markdown.contains("value: Maya"));
    assert!(markdown.contains("value: Riverside"));
    assert_eq!(markdown.matches("  property: location\n").count(), 1);
    assert_eq!(markdown.matches("  kind: occurred_at\n").count(), 1);
}

#[test]
fn provider_failure_keeps_a_readable_source_and_resumes_without_reimport() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("V17.txt");
    fs::write(&source, "Observation V17 happened at Riverside.").unwrap();
    let collection = workspace.path().join("collection");
    let mut failed = Application::open_with_semantic_provider(
        &collection,
        Arc::new(RecordedProvider::once(Err(ProviderError::recoverable(
            "Provider quota exceeded".into(),
        )))),
    )
    .unwrap();
    let queued = failed
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(queued.info.semantic_state, "pending");
    failed.resume_due_semantic_jobs().unwrap();
    let imported = failed.open_source(&queued.info.source_id).unwrap();
    assert_eq!(imported.info.semantic_state, "pending");
    assert!(imported
        .body
        .contains("Observation V17 happened at Riverside."));
    assert_eq!(
        failed
            .open_source(&imported.info.source_id)
            .unwrap()
            .info
            .semantic_state,
        "pending"
    );
    drop(failed);
    std::thread::sleep(std::time::Duration::from_secs(2));

    let mut resumed = Application::open_with_semantic_provider(
        &collection,
        Arc::new(RecordedProvider::once(Ok(KnowledgeDraft::default()))),
    )
    .unwrap();
    resumed.resume_due_semantic_jobs().unwrap();
    let current = resumed.open_source(&imported.info.source_id).unwrap();
    assert_eq!(current.info.semantic_state, "complete");
    assert!(current
        .body
        .contains("Observation V17 happened at Riverside."));
}

#[test]
fn imports_a_source_into_a_readable_page_and_retains_exact_original_bytes() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("Riverside notes.md");
    let bytes = b"# Riverside\r\n\r\nObservation V17 took place at Riverside on May 17, 2024.\r\n";
    fs::write(&source, bytes).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();

    let page = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();

    assert_eq!(page.info.title, "Riverside notes");
    assert_eq!(page.info.extraction, ExtractionState::TextPreserved);
    assert!(page
        .body
        .contains("Observation V17 took place at Riverside on May 17, 2024."));
    assert!(page.markdown.contains("source_id:"));
    assert!(page.markdown.contains("page_id:"));
    assert!(page.markdown.contains("Source text · lines 1–3"));
    assert!(page.markdown.contains("[Open original](original.md)"));
    assert_eq!(page.info.acquisitions[0].method, AcquisitionMethod::Picker);
    assert_eq!(page.info.acquisitions[0].path, source.to_string_lossy());
    assert_eq!(
        fs::read(app.original_path(&page.info.source_id).unwrap()).unwrap(),
        bytes
    );
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert_eq!(
        fs::read_to_string(app.page_path(&page.info.source_id).unwrap()).unwrap(),
        page.markdown
    );
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
    assert_eq!(
        app.open_source(&page.info.source_id).unwrap().markdown,
        page.markdown
    );
}

#[test]
fn a_retained_version_resolves_only_its_exact_known_original_asset() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("report.txt");
    let first_bytes = REVISION_1.as_bytes();
    let second_bytes = REVISION_2.as_bytes();
    fs::write(&source, first_bytes).unwrap();
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(RecordedProvider::sequence(vec![
            Ok(replacement_recording(REVISION_1, 12, true)),
            Ok(replacement_recording(REVISION_2, 15, false)),
        ])),
    )
    .unwrap();
    let first = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let old_version = first.info.versions_seen[0].clone();

    fs::write(&source, second_bytes).unwrap();
    app.import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let current = app.open_source(&first.info.source_id).unwrap();

    assert_eq!(
        fs::read(
            app.original_version_path(
                &first.info.source_id,
                &old_version.source_version_id,
                &old_version.asset,
            )
            .unwrap()
        )
        .unwrap(),
        first_bytes
    );
    assert_eq!(
        fs::read(app.original_path(&first.info.source_id).unwrap()).unwrap(),
        second_bytes
    );
    assert_eq!(
        fs::read(
            app.original_asset_path(&first.info.source_id, &current.info.asset)
                .unwrap()
        )
        .unwrap(),
        second_bytes
    );
    assert!(app
        .original_asset_path(&first.info.source_id, "../../outside.txt")
        .is_err());
    assert!(app
        .original_version_path(
            &first.info.source_id,
            &old_version.source_version_id,
            "../../outside.txt",
        )
        .is_err());
    assert!(app
        .original_version_path(&first.info.source_id, &"f".repeat(64), &old_version.asset)
        .is_err());
    assert_eq!(current.info.source_id, first.info.source_id);
}

#[test]
fn restart_and_reimport_preserve_identities_and_external_page_edits_without_duplicates() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("Riverside notes.txt");
    fs::write(
        &source,
        "Observation V17 took place at Riverside on May 17, 2024.",
    )
    .unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    let first = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    let edited = first
        .markdown
        .replace("Observation V17", "Externally annotated observation V17");
    fs::write(app.page_path(&first.info.source_id).unwrap(), &edited).unwrap();
    drop(app);
    let mut restarted = Application::open(&collection).unwrap();
    let duplicate = workspace.path().join("Same bytes.txt");
    fs::copy(&source, &duplicate).unwrap();
    let reopened = restarted
        .import_source(&duplicate, AcquisitionMethod::Drop)
        .unwrap();
    assert_eq!(reopened.info.source_id, first.info.source_id);
    assert_eq!(reopened.info.page_id, first.info.page_id);
    assert!(reopened
        .body
        .contains("Externally annotated observation V17"));
    assert_eq!(restarted.list_sources(0).unwrap().sources.len(), 1);
    assert_eq!(reopened.info.acquisitions.len(), 2);
    assert_eq!(
        reopened.info.acquisitions[1].path,
        duplicate.to_string_lossy()
    );
    assert_eq!(
        reopened.info.acquisitions[1].method,
        AcquisitionMethod::Drop
    );
    let again = restarted
        .import_source(&duplicate, AcquisitionMethod::Drop)
        .unwrap();
    assert_eq!(again.markdown, reopened.markdown);
}

#[test]
fn unsupported_and_damaged_sources_keep_inspectable_originals_with_explicit_coverage() {
    let workspace = tempdir().unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    for (name, bytes, state) in [
        (
            "unsupported.bin",
            b"plain-looking binary payload".as_slice(),
            ExtractionState::Unsupported,
        ),
        (
            "damaged.txt",
            b"\xff\xfe\x00".as_slice(),
            ExtractionState::InvalidUtf8,
        ),
        (
            "binary.txt",
            b"hello\x00world".as_slice(),
            ExtractionState::Unsupported,
        ),
    ] {
        let source = workspace.path().join(name);
        fs::write(&source, bytes).unwrap();
        let page = app.import_source(&source, AcquisitionMethod::Drop).unwrap();
        assert_eq!(page.info.extraction, state);
        assert_eq!(page.info.line_count, 0);
        assert!(page.body.contains("Text is unavailable"));
        assert!(!page.info.extraction_detail.contains("text preserved"));
        assert_eq!(
            fs::read(app.original_path(&page.info.source_id).unwrap()).unwrap(),
            bytes
        );
    }
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 3);
}

#[test]
fn identical_bytes_with_different_formats_keep_their_own_coverage_and_original_names() {
    let workspace = tempdir().unwrap();
    let text_source = workspace.path().join("same.txt");
    let unsupported_source = workspace.path().join("same.bin");
    let bytes = b"The same bytes can arrive under different format labels.";
    fs::write(&text_source, bytes).unwrap();
    fs::write(&unsupported_source, bytes).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();

    let text_page = app
        .import_source(&text_source, AcquisitionMethod::Picker)
        .unwrap();
    let unsupported_page = app
        .import_source(&unsupported_source, AcquisitionMethod::Drop)
        .unwrap();

    assert_ne!(text_page.info.source_id, unsupported_page.info.source_id);
    assert_eq!(text_page.info.sha256, unsupported_page.info.sha256);
    assert_eq!(text_page.info.extraction, ExtractionState::TextPreserved);
    assert_eq!(
        unsupported_page.info.extraction,
        ExtractionState::Unsupported
    );
    assert_eq!(text_page.info.asset, "original.txt");
    assert_eq!(unsupported_page.info.asset, "original.bin");
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 2);
    assert_eq!(
        fs::read(app.original_path(&unsupported_page.info.source_id).unwrap()).unwrap(),
        bytes
    );
}

#[test]
fn partial_office_coverage_stays_inspectable_after_semantic_processing() {
    use knowledge_garden::office::{CoverageScope, CoverageStatus};

    let workspace = tempdir().unwrap();
    let source = workspace.path().join("field-visit.docx");
    fs::write(
        &source,
        include_bytes!("fixtures/office/source/field-visit.docx"),
    )
    .unwrap();
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(WholeInputProvider),
    )
    .unwrap();

    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    assert!(imported
        .info
        .extraction_coverage
        .as_ref()
        .unwrap()
        .iter()
        .any(|part| part.scope == CoverageScope::EmbeddedObject
            && part.status == CoverageStatus::Partial));

    app.resume_due_semantic_jobs().unwrap();
    let processed = app.open_source(&imported.info.source_id).unwrap();
    assert_eq!(processed.info.semantic_state, "complete");
    assert!(processed
        .info
        .extraction_detail
        .to_lowercase()
        .contains("embedded"));
    assert!(processed
        .info
        .extraction_coverage
        .as_ref()
        .unwrap()
        .iter()
        .any(|part| part.scope == CoverageScope::EmbeddedObject
            && part.status == CoverageStatus::Partial));
    assert_eq!(
        processed.info.versions_seen[0].coverage, "partial",
        "semantic success over a projection must not imply full package coverage"
    );
}

#[test]
fn office_record_keys_preserve_multiple_fields_and_source_scope_across_revision() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/office");
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("multi-measurement.docx");
    fs::copy(fixtures.join("source/multi-measurement.docx"), &source).unwrap();
    let second_source = workspace.path().join("pressure-trial.docx");
    fs::copy(fixtures.join("heldout/pressure-trial.docx"), &second_source).unwrap();
    let slide_source = fixtures.join("source/multi-field.pptx");
    let transport = Arc::new(ConfiguredSystemOneTransport {
        roles: Mutex::new(VecDeque::from([
            "complete_replacement".into(),
            "complete_replacement".into(),
            "complete_replacement".into(),
            "unknown".into(),
        ])),
    });
    let provider =
        JevSemanticProvider::with_transport("recorded-test-credential".into(), transport);
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(provider),
    )
    .unwrap();

    let before_bytes = fs::read(&source).unwrap();
    let first = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let first = app.open_source(&first.info.source_id).unwrap();
    assert_eq!(first.info.semantic_state, "complete");
    let first_page = app
        .open_knowledge_page(&first.info.knowledge_pages[0].page_id)
        .unwrap();
    let first_frontmatter = first_page
        .markdown
        .strip_prefix("---\n")
        .unwrap()
        .split_once("\n---\n")
        .unwrap()
        .0;
    let first_record: Value = serde_yaml_ng::from_str(first_frontmatter).unwrap();
    let initial_rows = first_record["facts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|fact| fact["property"] == "table_row")
        .collect::<Vec<_>>();
    assert_eq!(initial_rows.len(), 2, "both measurements should survive");
    let water = initial_rows
        .iter()
        .find(|fact| {
            fact["value"]
                .as_str()
                .unwrap()
                .contains("Water temperature")
        })
        .unwrap();
    let oxygen = initial_rows
        .iter()
        .find(|fact| fact["value"].as_str().unwrap().contains("Dissolved oxygen"))
        .unwrap();
    assert_ne!(water["fact_id"], oxygen["fact_id"]);
    assert!(water["record_key"]
        .as_str()
        .unwrap()
        .contains("water-temperature"));
    assert!(oxygen["record_key"]
        .as_str()
        .unwrap()
        .contains("dissolved-oxygen"));
    let water_id = water["fact_id"].as_str().unwrap().to_owned();
    let water_key = water["record_key"].as_str().unwrap().to_owned();

    let second = app
        .import_source(&second_source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let second = app.open_source(&second.info.source_id).unwrap();
    assert_eq!(second.info.semantic_state, "complete");
    assert_ne!(first.info.source_id, second.info.source_id);
    let first_page_ids = first
        .info
        .knowledge_pages
        .iter()
        .map(|page| page.page_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert!(second
        .info
        .knowledge_pages
        .iter()
        .all(|page| !first_page_ids.contains(page.page_id.as_str())));

    let slide = app
        .import_source(&slide_source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let slide = app.open_source(&slide.info.source_id).unwrap();
    assert_eq!(slide.info.semantic_state, "complete");
    assert_ne!(first.info.source_id, slide.info.source_id);
    assert!(slide
        .info
        .knowledge_pages
        .iter()
        .all(|page| !first_page_ids.contains(page.page_id.as_str())));
    let slide_page = app
        .open_knowledge_page(&slide.info.knowledge_pages[0].page_id)
        .unwrap();
    let slide_frontmatter = slide_page
        .markdown
        .strip_prefix("---\n")
        .unwrap()
        .split_once("\n---\n")
        .unwrap()
        .0;
    let slide_record: Value = serde_yaml_ng::from_str(slide_frontmatter).unwrap();
    let visible_facts = slide_record["facts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|fact| fact["property"] == "visible_slide_text")
        .collect::<Vec<_>>();
    assert_eq!(visible_facts.len(), 2, "both visible shapes should survive");
    assert!(visible_facts
        .iter()
        .all(|fact| fact["origin"] == "slide_visible"));
    assert_ne!(visible_facts[0]["fact_id"], visible_facts[1]["fact_id"]);
    assert_ne!(
        visible_facts[0]["record_key"],
        visible_facts[1]["record_key"]
    );
    assert!(visible_facts
        .iter()
        .all(|fact| fact["record_key"].as_str().unwrap().contains("shape-id")));

    fs::copy(
        fixtures.join("source/multi-measurement-update.docx"),
        &source,
    )
    .unwrap();
    let updated = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    assert_eq!(updated.info.source_id, first.info.source_id);
    app.resume_due_semantic_jobs().unwrap();
    let updated = app.open_source(&first.info.source_id).unwrap();
    assert_eq!(updated.info.semantic_state, "complete");
    assert_eq!(updated.info.update_status, None);
    assert_eq!(
        updated.info.current_version_id.as_deref(),
        updated
            .info
            .versions_seen
            .last()
            .map(|version| version.source_version_id.as_str())
    );
    assert_eq!(
        updated.info.versions_seen.last().unwrap().update_role,
        "unknown"
    );
    let updated_page = app
        .open_knowledge_page(&updated.info.knowledge_pages[0].page_id)
        .unwrap();
    let updated_frontmatter = updated_page
        .markdown
        .strip_prefix("---\n")
        .unwrap()
        .split_once("\n---\n")
        .unwrap()
        .0;
    let updated_record: Value = serde_yaml_ng::from_str(updated_frontmatter).unwrap();
    let updated_rows = updated_record["facts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|fact| fact["property"] == "table_row")
        .collect::<Vec<_>>();
    assert_eq!(updated_rows.len(), 2);
    let updated_water = updated_rows
        .iter()
        .find(|fact| {
            fact["value"]
                .as_str()
                .unwrap()
                .contains("Water temperature")
        })
        .unwrap();
    assert_eq!(updated_water["fact_id"], water_id);
    assert_eq!(updated_water["record_key"], water_key);
    assert!(updated_water["qualifier"]
        .as_str()
        .unwrap()
        .contains("Provisional pending instrument calibration"));
    assert!(
        updated_water["value"].as_str().unwrap().contains("15 °C"),
        "updated fact must retain its stable field identity and new value: {updated_water}"
    );
    let updated_oxygen = updated_rows
        .iter()
        .find(|fact| fact["value"].as_str().unwrap().contains("Dissolved oxygen"))
        .unwrap();
    assert_eq!(updated_oxygen["fact_id"], oxygen["fact_id"]);
    assert!(updated_oxygen["value"]
        .as_str()
        .unwrap()
        .contains("7.4 mg/L"));
    assert_eq!(
        fs::read(app.original_path(&first.info.source_id).unwrap()).unwrap(),
        fs::read(&source).unwrap()
    );
    let prior_version = &updated.info.versions_seen[0];
    let prior_original = workspace
        .path()
        .join("collection")
        .join("sources")
        .join(first.info.source_id.trim_start_matches("source-"))
        .join("versions")
        .join(&prior_version.source_version_id)
        .join(&prior_version.asset);
    assert_eq!(fs::read(prior_original).unwrap(), before_bytes);
    assert_ne!(before_bytes, fs::read(&source).unwrap());
}

#[test]
fn ordinary_report_without_event_identity_stays_recoverable_instead_of_panicking() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("field-report.txt");
    fs::write(&source, "There were 12 visits on May 17, 2024.").unwrap();
    let transport = Arc::new(ConfiguredSystemOneTransport {
        roles: Mutex::new(VecDeque::from(["complete_replacement".into()])),
    });
    let provider =
        JevSemanticProvider::with_transport("recorded-test-credential".into(), transport);
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(provider),
    )
    .unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let current = app.open_source(&imported.info.source_id).unwrap();
    assert_eq!(current.info.semantic_state, "pending");
    assert!(current
        .info
        .semantic_error
        .as_deref()
        .unwrap_or_default()
        .contains("No supported event identity"));
    assert!(current
        .body
        .contains("There were 12 visits on May 17, 2024."));
}

#[test]
fn office_repeated_facts_without_distinct_record_keys_stay_pending() {
    let workspace = tempdir().unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/office/source/multi-measurement.docx");
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(RecordedProvider::once(Ok(KnowledgeDraft::default()))),
    )
    .unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    let rows = job
        .source_text
        .lines()
        .filter(|line| line.contains("; channel=table]"))
        .map(|line| line.split_once("] ").unwrap().1.trim().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    let evidence = |quote: &str| EvidenceDraft {
        quote: quote.to_owned(),
        byte_start: job.source_text.find(quote).unwrap(),
        byte_end: job.source_text.find(quote).unwrap() + quote.len(),
        origin: "table_cell".into(),
        qualifier: None,
        offset_basis: None,
        source_location: None,
    };
    let first_quote = rows[0].as_str();
    let second_quote = rows[1].as_str();
    let draft = KnowledgeDraft {
        entities: vec![EntityDraft {
            kind: "document".into(),
            label: "Imported document".into(),
            evidence: evidence(first_quote),
        }],
        facts: vec![
            FactDraft {
                subject: "Imported document".into(),
                property: "table_row".into(),
                value: first_quote.into(),
                evidence: evidence(first_quote),
                record_key: None,
            },
            FactDraft {
                subject: "Imported document".into(),
                property: "table_row".into(),
                value: second_quote.into(),
                evidence: evidence(second_quote),
                record_key: None,
            },
        ],
        ..KnowledgeDraft::default()
    };
    app.finish_semantic_job(job, Ok(draft)).unwrap();
    let current = app.open_source(&imported.info.source_id).unwrap();
    assert_eq!(current.info.semantic_state, "pending");
    assert!(current
        .info
        .semantic_error
        .as_deref()
        .unwrap_or_default()
        .contains("distinct stable record identities"));
    assert!(current.knowledge_pages.is_empty());
    assert!(current.body.contains("Water temperature"));
    assert!(current.body.contains("Dissolved oxygen"));
}

#[test]
fn deleting_the_lookup_index_reconstructs_the_same_page_and_asset() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("Riverside.txt");
    fs::write(
        &source,
        "Observation V17 took place at Riverside on May 17, 2024.",
    )
    .unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    let first = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    let original = fs::read(app.original_path(&first.info.source_id).unwrap()).unwrap();
    drop(app);
    fs::remove_dir_all(collection.join(".derived")).unwrap();
    let rebuilt = Application::open(&collection).unwrap();
    let visible = rebuilt.list_sources(0).unwrap();
    assert_eq!(visible.sources.len(), 1);
    assert_eq!(visible.sources[0].page_id, first.info.page_id);
    let page = rebuilt.open_source(&visible.sources[0].source_id).unwrap();
    assert_eq!(page.markdown, first.markdown);
    assert_eq!(
        fs::read(rebuilt.original_path(&page.info.source_id).unwrap()).unwrap(),
        original
    );
}

#[test]
fn oversized_text_retains_the_entire_original_without_claiming_complete_extraction() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("large.txt");
    let bytes = vec![b'x'; knowledge_garden::application::MAX_TEXT_BYTES + 1];
    fs::write(&source, &bytes).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    let page = app.import_source(&source, AcquisitionMethod::Drop).unwrap();
    assert_eq!(page.info.extraction, ExtractionState::TooLarge);
    assert_eq!(page.info.bytes, bytes.len() as u64);
    assert!(page.body.contains("Text is unavailable"));
    assert_eq!(
        fs::read(app.original_path(&page.info.source_id).unwrap()).unwrap(),
        bytes
    );
}

#[test]
fn empty_text_source_has_an_accurate_line_location() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("empty.txt");
    fs::write(&source, []).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();

    let page = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();

    assert_eq!(page.info.extraction, ExtractionState::TextPreserved);
    assert_eq!(page.info.line_count, 0);
    assert!(page.body.contains("Source text is empty."));
    assert!(!page.body.contains("lines 1–0"));
    assert!(fs::read(app.original_path(&page.info.source_id).unwrap())
        .unwrap()
        .is_empty());
}

#[test]
fn source_fences_and_metadata_cannot_be_injected_by_supplied_text_or_filename() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("Riverside [draft].txt");
    let text = "```\n---\nsource_id: attacker\n---\n# A heading in the original\n```\n";
    fs::write(&source, text).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    let first = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    assert!(first.body.contains("````text\n```\n---"));
    drop(app);
    let restarted = Application::open(workspace.path().join("collection")).unwrap();
    let reopened = restarted.open_source(&first.info.source_id).unwrap();
    assert_eq!(reopened.info.source_id, first.info.source_id);
    assert!(reopened.body.contains(text));
    assert_eq!(restarted.list_sources(0).unwrap().sources.len(), 1);
}

#[test]
fn a_second_process_cannot_write_the_same_collection_and_directories_are_rejected() {
    let workspace = tempdir().unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    assert!(Application::open(&collection).is_err());
    assert!(app
        .import_source(workspace.path(), AcquisitionMethod::Drop)
        .is_err());
    assert!(app.list_sources(0).unwrap().sources.is_empty());
    assert!(app.open_source("../../outside").is_err());
    drop(app);
    assert!(Application::open(&collection).is_ok());
}

#[test]
fn search_omits_superseded_source_text_and_locates_independent_support_after_rebuild() {
    // Expected obsolete-value and active-support behavior is independent of any live model output.
    let workspace = tempdir().unwrap();
    let revision_one = workspace.path().join("reports/revision-1.txt");
    let independent = workspace.path().join("notes/independent-duration.txt");
    let revision_two = workspace.path().join("updates/revision-2.txt");
    let pending_path = workspace.path().join("pending/unprocessed.txt");
    let independent_text = "Independent report dated May 19, 2024.\nObservation V17 took place on May 17, 2024.\nObserver: Maya. Location: Riverside. Visits: 12. Duration: 10 minutes.\n";
    fs::create_dir_all(revision_one.parent().unwrap()).unwrap();
    fs::create_dir_all(independent.parent().unwrap()).unwrap();
    fs::create_dir_all(revision_two.parent().unwrap()).unwrap();
    fs::create_dir_all(pending_path.parent().unwrap()).unwrap();
    fs::write(&revision_one, REVISION_1).unwrap();
    fs::write(&independent, independent_text).unwrap();
    fs::write(&revision_two, REVISION_2).unwrap();
    fs::write(&pending_path, "PENDING-CONTENT-TRACER 88142").unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open_with_semantic_provider(
        &collection,
        Arc::new(RecordedProvider::sequence(vec![
            Ok(replacement_recording(REVISION_1, 12, true)),
            Ok(replacement_recording(independent_text, 12, true)),
            Ok(replacement_recording(REVISION_2, 15, false)),
        ])),
    )
    .unwrap();

    let first = app
        .import_source(&revision_one, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let independent_source = app
        .import_source(&independent, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    app.import_source(&revision_two, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();

    let current = app.open_source(&first.info.source_id).unwrap();
    let event = current
        .knowledge_pages
        .iter()
        .find(|page| page.kind == "event")
        .unwrap();
    let obsolete = app
        .search_pages(PageSearchRequest {
            query: "12 visits".into(),
            ..PageSearchRequest::default()
        })
        .unwrap();
    assert!(
        !obsolete.pages.iter().any(|result| result.page_id == first.info.page_id),
        "superseded revision-1 source text must not be presented as an ordinary result: {obsolete:?}"
    );
    let active_support = app
        .search_pages(PageSearchRequest {
            query: "10 minutes".into(),
            ..PageSearchRequest::default()
        })
        .unwrap()
        .pages
        .into_iter()
        .find(|result| result.page_id == event.page_id)
        .expect("active independent duration result");
    let location = active_support
        .match_location
        .expect("fact evidence location");
    let event_owner = app.open_knowledge_page(&event.page_id).unwrap();
    assert_eq!(active_support.page_id, event.page_id);
    assert_eq!(active_support.source_id, event_owner.source_id);
    assert_eq!(location.source_id, independent_source.info.source_id);
    let independent_current = app.open_source(&independent_source.info.source_id).unwrap();
    assert_eq!(
        location.source_version_id.as_deref(),
        independent_current.info.current_version_id.as_deref()
    );
    assert_eq!(location.offset_basis, "preserved_text");
    assert_eq!(location.quote, "Duration: 10 minutes.");
    assert_eq!(
        &independent_text[location.byte_start..location.byte_end],
        location.quote
    );

    app.import_source(&pending_path, AcquisitionMethod::Picker)
        .unwrap();
    drop(app);
    let mut reopened = Application::open(&collection).unwrap();
    reopened.rebuild_index().unwrap();
    let after_rebuild = reopened
        .search_pages(PageSearchRequest {
            query: "10 minutes".into(),
            ..PageSearchRequest::default()
        })
        .unwrap()
        .pages
        .into_iter()
        .find(|result| result.page_id == event.page_id)
        .expect("active support survives index rebuild");
    assert_eq!(
        after_rebuild.source_id,
        reopened
            .open_knowledge_page(&event.page_id)
            .unwrap()
            .source_id
    );
    let rebuilt_location = after_rebuild.match_location.unwrap();
    assert_eq!(
        rebuilt_location.source_id,
        independent_source.info.source_id
    );
    assert_eq!(rebuilt_location.quote, "Duration: 10 minutes.");
    let pending = reopened
        .search_pages(PageSearchRequest {
            query: "PENDING-CONTENT-TRACER".into(),
            ..PageSearchRequest::default()
        })
        .unwrap();
    assert!(pending
        .pages
        .iter()
        .any(|result| { result.title == "unprocessed" && result.processing_status == "pending" }));
    let stale_after_rebuild = reopened
        .search_pages(PageSearchRequest {
            query: "12 visits".into(),
            ..PageSearchRequest::default()
        })
        .unwrap();
    assert!(!stale_after_rebuild
        .pages
        .iter()
        .any(|result| result.page_id == first.info.page_id));
}
