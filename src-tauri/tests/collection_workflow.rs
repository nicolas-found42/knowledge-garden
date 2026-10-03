use knowledge_garden::application::{
    AcquisitionMethod, Application, ExtractionState, PageSearchRequest,
};
use knowledge_garden::providers::{JevSemanticProvider, SystemOneTransport};
use knowledge_garden::semantic::{
    EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft, ProviderError, RelationshipDraft,
    SemanticDecision, SemanticProvider, TagDraft,
};
use serde_json::Value;
use std::fs;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

struct RecordedProvider {
    result: Mutex<Option<std::result::Result<KnowledgeDraft, ProviderError>>>,
}

impl RecordedProvider {
    fn once(result: std::result::Result<KnowledgeDraft, ProviderError>) -> Self {
        Self {
            result: Mutex::new(Some(result)),
        }
    }
}

impl SemanticProvider for RecordedProvider {
    fn form_knowledge(
        &self,
        _source_text: &str,
    ) -> std::result::Result<KnowledgeDraft, ProviderError> {
        self.result.lock().unwrap().take().expect("one recording")
    }
}

struct RecordedSystemOneTransport {
    response: Value,
}

impl SystemOneTransport for RecordedSystemOneTransport {
    fn complete(
        &self,
        _api_key: &str,
        _request: &Value,
    ) -> std::result::Result<Value, ProviderError> {
        Ok(self.response.clone())
    }
}

fn v17_recording() -> KnowledgeDraft {
    let text = "Revision 1 dated May 18, 2024; observation V17 on May 17, 2024; observer Maya; location Riverside; 12 visits; duration 10 minutes. It possibly occurred at Riverside. A different person named Maya filed another report.";
    let evidence = |quote: &str| EvidenceDraft {
        quote: quote.to_owned(),
        byte_start: text.find(quote).unwrap(),
        byte_end: text.find(quote).unwrap() + quote.len(),
        origin: "observed".to_owned(),
        qualifier: None,
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
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "observer".into(),
                value: "Maya (observer)".into(),
                evidence: evidence("observer Maya"),
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "location".into(),
                value: "Riverside".into(),
                evidence: evidence("location Riverside"),
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "visit_count".into(),
                value: "12 visits".into(),
                evidence: evidence("12 visits"),
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "duration".into(),
                value: "10 minutes".into(),
                evidence: evidence("duration 10 minutes"),
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "possible_location".into(),
                value: "Riverside".into(),
                evidence: EvidenceDraft {
                    qualifier: Some("possibly".into()),
                    ..evidence("possibly occurred at Riverside")
                },
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
    }
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
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "location".into(),
                value: "Riverside".into(),
                evidence: evidence("Riverside"),
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "observer".into(),
                value: "Maya".into(),
                evidence: evidence("Maya observed the event"),
            },
            FactDraft {
                subject: "Observation V17".into(),
                property: "visit_count".into(),
                value: "12 visits".into(),
                evidence: evidence("12 visits were reported"),
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
            },
            FactDraft {
                subject: event.into(),
                property: "observer".into(),
                value: "Maya".into(),
                evidence: evidence("different person named Maya observed"),
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
fn malformed_recorded_jev_answers_leave_the_public_application_in_recoverable_pending_state() {
    let source_text = "Maya observed Observation V50 at Riverside on May 1, 2024.";
    let valid = serde_json::json!({
        "model":"typesafe/jev-1.13-fixture",
        "answers":{
            "event_identity":{"type":"choice","choice":"event","probabilities":{"event":0.95,"not_event":0.03,"unclear":0.02}},
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
