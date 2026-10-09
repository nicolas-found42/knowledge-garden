use knowledge_garden::application::{
    AcquisitionMethod, Application, ExploredPathStep, PathDetailsRequest, PathExploreRequest,
};
use knowledge_garden::extraction::SourceExtractor;
use knowledge_garden::office::{CoveragePart, CoverageScope, CoverageStatus, OfficeProjection};
use knowledge_garden::semantic::{
    EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft, RelationshipDraft, SemanticProvider,
    SourceUpdateDraft, SourceUpdateRole,
};
use rusqlite::{Connection, params};
use std::sync::Arc;
use tempfile::tempdir;

struct FixedGraph(KnowledgeDraft);

impl SemanticProvider for FixedGraph {
    fn form_knowledge(
        &self,
        _source_text: &str,
    ) -> std::result::Result<KnowledgeDraft, knowledge_garden::semantic::ProviderError> {
        Ok(self.0.clone())
    }
}

struct RelationshipWithdrawalGraph;

impl SemanticProvider for RelationshipWithdrawalGraph {
    fn form_knowledge(
        &self,
        source_text: &str,
    ) -> std::result::Result<KnowledgeDraft, knowledge_garden::semantic::ProviderError> {
        let entities = vec![
            entity(source_text, "person", "Maya"),
            entity(source_text, "event", "V17"),
        ];
        if source_text.contains("complete replacement") {
            return Ok(KnowledgeDraft {
                entities,
                source_update: Some(SourceUpdateDraft {
                    role: SourceUpdateRole::CompleteReplacement,
                    evidence: evidence(source_text, "complete replacement"),
                    certainty: 0.99,
                    source_date: None,
                    source_date_evidence: None,
                    source_date_certainty: 0.0,
                    source_revision: None,
                    source_revision_evidence: None,
                    source_revision_certainty: 0.0,
                }),
                ..KnowledgeDraft::default()
            });
        }
        let relationships = if source_text.contains("Maya observed V17") {
            vec![RelationshipDraft {
                from: "V17".into(),
                to: "Maya".into(),
                kind: "observed_by".into(),
                qualifier: None,
                evidence: evidence(source_text, "Maya observed V17"),
            }]
        } else {
            Vec::new()
        };
        Ok(KnowledgeDraft {
            entities,
            relationships,
            ..KnowledgeDraft::default()
        })
    }
}

fn evidence(text: &str, quote: &str) -> EvidenceDraft {
    let start = text.find(quote).expect("fixture quote is present");
    EvidenceDraft {
        quote: quote.to_owned(),
        byte_start: start,
        byte_end: start + quote.len(),
        origin: "observed".into(),
        qualifier: None,
        offset_basis: None,
        source_location: None,
    }
}

fn entity(text: &str, kind: &str, label: &str) -> EntityDraft {
    EntityDraft {
        kind: kind.into(),
        label: label.into(),
        evidence: evidence(text, label),
    }
}

fn fixture_graph(text: &str) -> KnowledgeDraft {
    KnowledgeDraft {
        entities: vec![
            entity(text, "person", "Maya"),
            entity(text, "event", "V17"),
            entity(text, "event", "V18"),
            entity(text, "place", "Riverside"),
            entity(text, "place", "Riverside Park"),
            entity(text, "place", "Paris"),
        ],
        relationships: vec![
            RelationshipDraft {
                from: "V17".into(),
                to: "Maya".into(),
                kind: "observed_by".into(),
                qualifier: Some("field note".into()),
                evidence: evidence(text, "Maya observed V17"),
            },
            RelationshipDraft {
                from: "V17".into(),
                to: "Riverside".into(),
                kind: "occurred_at".into(),
                qualifier: Some("location reported with uncertainty".into()),
                evidence: evidence(text, "V17 occurred at Riverside"),
            },
            RelationshipDraft {
                from: "V18".into(),
                to: "Maya".into(),
                kind: "observed_by".into(),
                qualifier: None,
                evidence: evidence(text, "Maya also observed V18"),
            },
            RelationshipDraft {
                from: "V18".into(),
                to: "Riverside".into(),
                kind: "occurred_at".into(),
                qualifier: Some("location reported with uncertainty".into()),
                evidence: evidence(text, "V18 occurred at Riverside"),
            },
            RelationshipDraft {
                from: "Riverside".into(),
                to: "Riverside".into(),
                kind: "related_to".into(),
                qualifier: Some("self-link fixture".into()),
                evidence: evidence(text, "Riverside is self-linked"),
            },
        ],
        ..KnowledgeDraft::default()
    }
}

struct VersionedOfficeGraph;

impl SemanticProvider for VersionedOfficeGraph {
    fn form_knowledge(
        &self,
        source_text: &str,
    ) -> std::result::Result<KnowledgeDraft, knowledge_garden::semantic::ProviderError> {
        let profile = FactDraft {
            subject: "Maya".into(),
            property: "description".into(),
            value: "field observer".into(),
            evidence: evidence(source_text, "Maya is a field observer"),
            record_key: Some("maya-profile".into()),
        };
        if source_text.contains("Revision 2 supplement") {
            return Ok(KnowledgeDraft {
                entities: vec![entity(source_text, "person", "Maya")],
                facts: vec![profile],
                source_update: Some(SourceUpdateDraft {
                    role: SourceUpdateRole::Unknown,
                    evidence: evidence(source_text, "Revision 2 supplement"),
                    certainty: 0.99,
                    source_date: None,
                    source_date_evidence: None,
                    source_date_certainty: 0.0,
                    source_revision: None,
                    source_revision_evidence: None,
                    source_revision_certainty: 0.0,
                }),
                ..KnowledgeDraft::default()
            });
        }
        Ok(KnowledgeDraft {
            entities: vec![
                entity(source_text, "person", "Maya"),
                entity(source_text, "event", "V17"),
            ],
            facts: vec![profile],
            relationships: vec![RelationshipDraft {
                from: "V17".into(),
                to: "Maya".into(),
                kind: "observed_by".into(),
                qualifier: None,
                evidence: evidence(source_text, "Maya observed V17"),
            }],
            ..KnowledgeDraft::default()
        })
    }
}

struct TextOfficeProjection;

impl SourceExtractor for TextOfficeProjection {
    fn extract(
        &self,
        path: &std::path::Path,
        _format: &str,
        _title: &str,
    ) -> Result<OfficeProjection, String> {
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        Ok(OfficeProjection {
            markdown: text.clone(),
            semantic_text: text.clone(),
            line_count: text.lines().count(),
            coverage: vec![CoveragePart {
                scope: CoverageScope::MainDocument,
                status: CoverageStatus::Complete,
                source_location: Some("document".into()),
                detail: "The complete test document was projected.".into(),
            }],
            partial: false,
            detail: "Complete test projection.".into(),
        })
    }
}

fn load_path_steps(app: &mut Application, detail_token: &str) -> Vec<ExploredPathStep> {
    let mut continuation = None;
    let mut steps = Vec::new();
    loop {
        let page = app
            .explore_path_details(PathDetailsRequest {
                detail_token: detail_token.to_owned(),
                continuation,
                work_budget: 50,
            })
            .unwrap();
        steps.extend(page.steps);
        let Some(next) = page.next_cursor else {
            assert!(page.complete);
            break;
        };
        continuation = Some(next);
    }
    steps.reverse();
    steps
}

#[test]
fn source_version_currentness_change_invalidates_stored_path_details() {
    let workspace = tempdir().unwrap();
    let text = "Maya is a field observer. Maya observed V17.";
    let source = workspace.path().join("versioned-path.docx");
    std::fs::write(&source, text).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open_with_providers(
        &collection,
        Arc::new(VersionedOfficeGraph),
        Arc::new(TextOfficeProjection),
    )
    .unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let first = app.open_source(&imported.info.source_id).unwrap();
    let first_version = first.info.current_version_id.clone().unwrap();
    let maya = first
        .knowledge_pages
        .into_iter()
        .find(|page| page.title == "Maya")
        .unwrap();
    let result = app
        .explore_paths(PathExploreRequest {
            start_page_id: maya.page_id.clone(),
            max_hops: 1,
            page_size: 10,
            relationship_work_budget: 10,
            continuation: None,
        })
        .unwrap();
    let detail_token = result.paths[0].detail_token.clone();

    // A field-aligned Office refresh promotes revision 2 while preserving the
    // unrelated V17 relationship as prior-version support.
    let revised_text = "Maya is a field observer. Maya observed V17.\nRevision 2 supplement: V18 was also recorded.";
    std::fs::write(&source, revised_text).unwrap();
    app.import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let after = app.open_source(&imported.info.source_id).unwrap();
    let promoted = after.info.current_version_id.clone().unwrap();
    assert_ne!(promoted, first_version);
    assert!(
        after
            .info
            .versions_seen
            .iter()
            .find(|version| version.source_version_id == promoted)
            .unwrap()
            .state
            == "complete"
    );

    let refreshed = app
        .explore_paths(PathExploreRequest {
            start_page_id: maya.page_id,
            max_hops: 1,
            page_size: 10,
            relationship_work_budget: 10,
            continuation: None,
        })
        .unwrap();
    let prior_edge = refreshed
        .paths
        .iter()
        .find(|path| path.target_title == "V17")
        .expect("the supplement leaves the prior V17 relationship available");
    let prior_step = load_path_steps(&mut app, &prior_edge.detail_token);
    assert_eq!(prior_step[0].source_version_id, first_version);

    let error = app
        .explore_path_details(PathDetailsRequest {
            detail_token,
            continuation: None,
            work_budget: 10,
        })
        .expect_err("version-dependent support details must be stale after promotion");
    assert!(error.to_string().contains("stale"));
}

#[test]
fn accepted_complete_replacement_withdraws_obsolete_path_support() {
    let workspace = tempdir().unwrap();
    let source_a = workspace.path().join("field-report.txt");
    let source_b = workspace.path().join("independent-index.txt");
    let initial_text = "Maya observed V17.";
    std::fs::write(&source_a, initial_text).unwrap();
    std::fs::write(&source_b, "Maya and V17 are independently indexed names.").unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open_with_semantic_provider(
        &collection,
        Arc::new(RelationshipWithdrawalGraph),
    )
    .unwrap();
    let first = app
        .import_source(&source_a, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    app.import_source(&source_b, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();

    let maya = app
        .open_source(&first.info.source_id)
        .unwrap()
        .knowledge_pages
        .into_iter()
        .find(|page| page.title == "Maya")
        .expect("both independent records keep the Maya page available");
    let before = app
        .explore_paths(PathExploreRequest {
            start_page_id: maya.page_id.clone(),
            max_hops: 1,
            page_size: 10,
            relationship_work_budget: 10,
            continuation: None,
        })
        .unwrap();
    let prior_path = before
        .paths
        .iter()
        .find(|path| path.target_title == "V17")
        .expect("the original observation relationship is navigable");
    let obsolete_detail = prior_path.detail_token.clone();
    let previous_version = app
        .open_source(&first.info.source_id)
        .unwrap()
        .info
        .current_version_id
        .unwrap();

    let replacement = "Revision 2 complete replacement. Maya and V17 remain as disconnected references.";
    std::fs::write(&source_a, replacement).unwrap();
    app.import_source(&source_a, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let updated = app.open_source(&first.info.source_id).unwrap();
    assert_ne!(
        updated.info.current_version_id.as_deref(),
        Some(previous_version.as_str())
    );
    assert_eq!(updated.info.update_status, None);

    let stale = app
        .explore_path_details(PathDetailsRequest {
            detail_token: obsolete_detail,
            continuation: None,
            work_budget: 10,
        })
        .expect_err("withdrawn source support invalidates previously saved path details");
    assert!(stale.to_string().contains("stale"));
    let after = app
        .explore_paths(PathExploreRequest {
            start_page_id: maya.page_id,
            max_hops: 1,
            page_size: 10,
            relationship_work_budget: 10,
            continuation: None,
        })
        .unwrap();
    assert!(
        after.paths.iter().all(|path| path.target_title != "V17"),
        "the old relationship is not offered for ordinary navigation after its only support was withdrawn"
    );
}

#[test]
fn continuation_cursor_is_an_opaque_token_not_a_serialized_graph_frontier() {
    let workspace = tempdir().unwrap();
    let text = "Maya observed V17. Maya also observed V18.";
    let source = workspace.path().join("cursor-fixture.txt");
    std::fs::write(&source, text).unwrap();
    let collection = workspace.path().join("collection");
    let provider: Arc<dyn SemanticProvider> = Arc::new(FixedGraph(KnowledgeDraft {
        entities: vec![
            entity(text, "person", "Maya"),
            entity(text, "event", "V17"),
            entity(text, "event", "V18"),
        ],
        relationships: vec![
            RelationshipDraft {
                from: "V17".into(),
                to: "Maya".into(),
                kind: "observed_by".into(),
                qualifier: None,
                evidence: evidence(text, "Maya observed V17"),
            },
            RelationshipDraft {
                from: "V18".into(),
                to: "Maya".into(),
                kind: "observed_by".into(),
                qualifier: None,
                evidence: evidence(text, "Maya also observed V18"),
            },
        ],
        ..KnowledgeDraft::default()
    }));
    let mut app = Application::open_with_semantic_provider(&collection, provider.clone()).unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let start = app
        .open_source(&imported.info.source_id)
        .unwrap()
        .knowledge_pages
        .into_iter()
        .find(|page| page.title == "Maya")
        .expect("Maya is a published page");

    let first = app
        .explore_paths(PathExploreRequest {
            start_page_id: start.page_id.clone(),
            max_hops: 1,
            page_size: 1,
            relationship_work_budget: 1,
            continuation: None,
        })
        .unwrap();
    let token = first.next_cursor.expect("one relationship remains");
    assert!(
        token.len() <= 128,
        "public continuations stay within the documented 128-byte token budget"
    );
    let tampered = format!("{token}x");
    let invalid = app
        .explore_paths(PathExploreRequest {
            start_page_id: start.page_id.clone(),
            max_hops: 1,
            page_size: 1,
            relationship_work_budget: 1,
            continuation: Some(tampered),
        })
        .expect_err("a tampered continuation is rejected");
    assert!(invalid.to_string().contains("cursor"));
    drop(app);
    let mut reopened = Application::open_with_semantic_provider(&collection, provider).unwrap();
    let resumed = reopened
        .explore_paths(PathExploreRequest {
            start_page_id: start.page_id,
            max_hops: 1,
            page_size: 1,
            relationship_work_budget: 1,
            continuation: Some(token),
        })
        .unwrap();
    assert_eq!(resumed.paths.len(), 1);
    assert!(resumed.paths[0].target_title == "V17" || resumed.paths[0].target_title == "V18");
}

#[test]
fn selected_path_details_are_loaded_only_when_requested_and_are_paged() {
    let workspace = tempdir().unwrap();
    let text = "Maya observed V17.";
    let source = workspace.path().join("selected-path.txt");
    std::fs::write(&source, text).unwrap();
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(FixedGraph(KnowledgeDraft {
            entities: vec![entity(text, "person", "Maya"), entity(text, "event", "V17")],
            relationships: vec![RelationshipDraft {
                from: "V17".into(),
                to: "Maya".into(),
                kind: "observed_by".into(),
                qualifier: Some("field note".into()),
                evidence: evidence(text, text),
            }],
            ..KnowledgeDraft::default()
        })),
    )
    .unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let start = app
        .open_source(&imported.info.source_id)
        .unwrap()
        .knowledge_pages
        .into_iter()
        .find(|page| page.title == "Maya")
        .expect("Maya is a published page");
    let result = app
        .explore_paths(PathExploreRequest {
            start_page_id: start.page_id,
            max_hops: 1,
            page_size: 10,
            relationship_work_budget: 10,
            continuation: None,
        })
        .unwrap();
    let path = result
        .paths
        .iter()
        .find(|path| path.target_title == "V17")
        .expect("V17 is reachable");
    assert_eq!(path.step_count, 1);
    assert!(
        serde_json::to_value(path).unwrap().get("steps").is_none(),
        "unselected stops do not return path steps"
    );
    let details = app
        .explore_path_details(PathDetailsRequest {
            detail_token: path.detail_token.clone(),
            continuation: None,
            work_budget: 1,
        })
        .unwrap();
    assert_eq!(details.target_title, "V17");
    assert!(details.complete);
    assert_eq!(details.work_units, 1);
    assert_eq!(details.steps.len(), 1);
    assert_eq!(details.steps[0].from_title, "V17");
    assert_eq!(details.steps[0].to_title, "Maya");
    assert_eq!(details.steps[0].evidence_quote, text);
    assert_eq!(details.steps[0].qualifier.as_deref(), Some("field note"));
}

#[test]
fn public_path_exploration_returns_reachable_pages_from_an_imported_relationship() {
    let workspace = tempdir().unwrap();
    let text = "Maya observed V17.";
    let source = workspace.path().join("field-note.txt");
    std::fs::write(&source, text).unwrap();
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(FixedGraph(KnowledgeDraft {
            entities: vec![entity(text, "person", "Maya"), entity(text, "event", "V17")],
            relationships: vec![RelationshipDraft {
                from: "V17".into(),
                to: "Maya".into(),
                kind: "observed_by".into(),
                qualifier: None,
                evidence: evidence(text, text),
            }],
            ..KnowledgeDraft::default()
        })),
    )
    .unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let processed = app.open_source(&imported.info.source_id).unwrap();
    let maya = processed
        .knowledge_pages
        .iter()
        .find(|page| page.title == "Maya")
        .expect("Maya is a published page");

    let result = app
        .explore_paths(PathExploreRequest {
            start_page_id: maya.page_id.clone(),
            max_hops: 1,
            page_size: 16,
            relationship_work_budget: 50,
            continuation: None,
        })
        .unwrap();

    assert!(result.paths.iter().any(|path| path.target_title == "V17"));
}

#[test]
fn public_path_exploration_preserves_arrows_and_reports_supporting_evidence() {
    let workspace = tempdir().unwrap();
    let text = "Maya observed V17. Maya also observed V18. V17 occurred at Riverside. V18 occurred at Riverside. Riverside is self-linked. Riverside Park and Paris are separate places.";
    let source = workspace.path().join("field-note.txt");
    std::fs::write(&source, text).unwrap();
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(FixedGraph(fixture_graph(text))),
    )
    .unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();

    let processed = app.open_source(&imported.info.source_id).unwrap();
    let start = processed
        .knowledge_pages
        .iter()
        .find(|page| page.title == "Maya")
        .expect("Maya is a published page");
    let one_hop = app
        .explore_paths(PathExploreRequest {
            start_page_id: start.page_id.clone(),
            max_hops: 1,
            page_size: 16,
            relationship_work_budget: 50,
            continuation: None,
        })
        .unwrap();
    let one_hop = serde_json::to_value(one_hop).unwrap();
    assert_eq!(one_hop["max_hops"], 1);
    assert_eq!(one_hop["start_page_id"], start.page_id);
    assert_eq!(one_hop["paths"].as_array().unwrap().len(), 2);
    let maya_to_v17 = one_hop["paths"]
        .as_array()
        .unwrap()
        .iter()
        .find(|path| path["target_title"] == "V17")
        .unwrap();
    assert!(maya_to_v17.get("steps").is_none());
    let maya_to_v17_steps =
        load_path_steps(&mut app, maya_to_v17["detail_token"].as_str().unwrap());
    assert_eq!(maya_to_v17_steps[0].kind, "observed_by");
    assert_eq!(maya_to_v17_steps[0].from_title, "V17");
    assert_eq!(maya_to_v17_steps[0].to_title, "Maya");
    assert_eq!(
        maya_to_v17_steps[0].traversal_direction,
        knowledge_garden::application::TraversalDirection::AgainstArrow
    );
    assert_eq!(maya_to_v17_steps[0].evidence_quote, "Maya observed V17");
    assert!(one_hop["complete"].as_bool().unwrap());
    assert!(one_hop["next_cursor"].is_null());

    let two_hop = app
        .explore_paths(PathExploreRequest {
            start_page_id: start.page_id.clone(),
            max_hops: 2,
            page_size: 16,
            relationship_work_budget: 50,
            continuation: None,
        })
        .unwrap();
    let two_hop = serde_json::to_value(two_hop).unwrap();
    let path_rows = two_hop["paths"].as_array().unwrap();
    let page_ids = path_rows
        .iter()
        .map(|path| path["target_page_id"].as_str().unwrap())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(path_rows.len(), page_ids.len(), "each page appears once");
    let riverside = path_rows
        .iter()
        .find(|path| path["target_title"] == "Riverside")
        .expect("the second directed relationship reaches Riverside");
    assert!(riverside.get("steps").is_none());
    let riverside_steps = load_path_steps(&mut app, riverside["detail_token"].as_str().unwrap());
    assert_eq!(riverside_steps.len(), 2);
    assert_eq!(riverside_steps[1].kind, "occurred_at");
    let route_event = riverside_steps[1].from_title.as_str();
    assert!(route_event == "V17" || route_event == "V18");
    assert_eq!(
        riverside_steps[1].qualifier,
        Some("location reported with uncertainty".into())
    );
    assert_eq!(
        riverside_steps[1].evidence_quote,
        format!("{route_event} occurred at Riverside")
    );
    let repeated_query = app
        .explore_paths(PathExploreRequest {
            start_page_id: start.page_id.clone(),
            max_hops: 2,
            page_size: 16,
            relationship_work_budget: 50,
            continuation: None,
        })
        .unwrap();
    let repeated_query = serde_json::to_value(repeated_query).unwrap();
    let repeated_riverside = repeated_query["paths"]
        .as_array()
        .unwrap()
        .iter()
        .find(|path| path["target_title"] == "Riverside")
        .unwrap();
    let repeated_steps = load_path_steps(
        &mut app,
        repeated_riverside["detail_token"].as_str().unwrap(),
    );
    assert_eq!(riverside_steps.len(), repeated_steps.len());
    assert_eq!(
        riverside_steps[0].relationship_id,
        repeated_steps[0].relationship_id
    );
    assert_eq!(
        riverside_steps[1].relationship_id,
        repeated_steps[1].relationship_id
    );
    assert!(
        two_hop["paths"]
            .as_array()
            .unwrap()
            .iter()
            .all(|path| path["target_title"] != "Paris")
    );

    let reverse = app
        .explore_paths(PathExploreRequest {
            start_page_id: riverside["target_page_id"].as_str().unwrap().into(),
            max_hops: 1,
            page_size: 16,
            relationship_work_budget: 50,
            continuation: None,
        })
        .unwrap();
    let reverse = serde_json::to_value(reverse).unwrap();
    let reverse_to_event = reverse["paths"]
        .as_array()
        .unwrap()
        .iter()
        .find(|path| path["target_title"] == "V17")
        .expect("incoming event-place link remains navigable from Riverside");
    let reverse_steps =
        load_path_steps(&mut app, reverse_to_event["detail_token"].as_str().unwrap());
    assert_eq!(reverse_steps[0].kind, "occurred_at");
    assert_eq!(reverse_steps[0].from_title, "V17");
    assert_eq!(reverse_steps[0].to_title, "Riverside");
    assert_eq!(
        reverse_steps[0].traversal_direction,
        knowledge_garden::application::TraversalDirection::AgainstArrow
    );
    assert!(
        reverse["paths"]
            .as_array()
            .unwrap()
            .iter()
            .all(|path| path["target_title"] != "Paris")
    );
    assert!(
        !reverse["paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path["target_page_id"] == riverside["target_page_id"])
    );

    let from_riverside = app
        .explore_paths(PathExploreRequest {
            start_page_id: riverside["target_page_id"].as_str().unwrap().into(),
            max_hops: 1,
            page_size: 16,
            relationship_work_budget: 50,
            continuation: None,
        })
        .unwrap();
    let from_riverside = serde_json::to_value(from_riverside).unwrap();
    let returned_ids = from_riverside["paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|path| path["target_page_id"].as_str().unwrap())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        returned_ids.len(),
        from_riverside["paths"].as_array().unwrap().len(),
        "cycles and the self-link must not duplicate stops"
    );
    assert!(!returned_ids.contains(riverside["target_page_id"].as_str().unwrap()));
    assert!(
        from_riverside["paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path["target_title"] == "V17")
    );
}

#[test]
fn path_exploration_cursor_bounds_work_and_pages_unique_stops() {
    let workspace = tempdir().unwrap();
    let text = "Maya observed V17. Maya also observed V18. V17 occurred at Riverside. V18 occurred at Riverside. Riverside is self-linked. Riverside Park and Paris are separate places.";
    let source = workspace.path().join("field-note.txt");
    std::fs::write(&source, text).unwrap();
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(FixedGraph(fixture_graph(text))),
    )
    .unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let start = app
        .open_source(&imported.info.source_id)
        .unwrap()
        .knowledge_pages
        .into_iter()
        .find(|page| page.title == "Maya")
        .expect("Maya is a published page");

    let mut continuation = None::<String>;
    let mut seen = std::collections::HashSet::<String>::new();
    let mut finished = false;
    for _ in 0..64 {
        let page = app
            .explore_paths(PathExploreRequest {
                start_page_id: start.page_id.clone(),
                max_hops: 2,
                page_size: 1,
                relationship_work_budget: 1,
                continuation,
            })
            .unwrap();
        let page = serde_json::to_value(page).unwrap();
        let paths = page["paths"].as_array().unwrap();
        assert!(
            paths.len() <= 1,
            "the result page respects its requested size"
        );
        assert!(
            page["relationships_examined"].as_u64().unwrap() <= 1,
            "one invocation stays within its explicit edge work budget"
        );
        for path in paths {
            let target = path["target_page_id"].as_str().unwrap().to_owned();
            assert!(seen.insert(target), "a continuation must not repeat stops");
        }
        if page["complete"].as_bool().unwrap() {
            finished = true;
            break;
        }
        continuation = Some(
            page["next_cursor"]
                .as_str()
                .expect("partial work exposes a continuation cursor")
                .to_owned(),
        );
    }

    assert!(
        finished,
        "fixture work should complete through resumable calls"
    );
    let target_titles = seen
        .iter()
        .map(|page_id| app.open_knowledge_page(page_id).unwrap().title)
        .collect::<std::collections::HashSet<_>>();
    let expected = ["V17", "V18", "Riverside"]
        .into_iter()
        .map(str::to_owned)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(target_titles, expected);
}

fn assert_dense_neighborhood_inspects_only_the_requested_relationship_budget(edge_count: usize) {
    let workspace = tempdir().unwrap();
    let mut lines = Vec::new();
    for index in 0..edge_count {
        let label = format!("V{index:03}");
        let line = format!("Maya observed {label}.");
        lines.push(line.clone());
    }
    let text = lines.join("\n");
    let mut entities = (0..edge_count)
        .map(|index| {
            let label = format!("V{index:03}");
            EntityDraft {
                kind: "event".into(),
                label: label.clone(),
                evidence: evidence(&text, &label),
            }
        })
        .collect::<Vec<_>>();
    let relationships = lines
        .iter()
        .enumerate()
        .map(|(index, line)| RelationshipDraft {
            from: format!("V{index:03}"),
            to: "Maya".into(),
            kind: "observed_by".into(),
            qualifier: None,
            evidence: evidence(&text, line),
        })
        .collect();
    entities.push(EntityDraft {
        kind: "person".into(),
        label: "Maya".into(),
        evidence: evidence(&text, "Maya"),
    });
    let graph = KnowledgeDraft {
        entities,
        relationships,
        ..KnowledgeDraft::default()
    };
    let source = workspace.path().join("dense-neighborhood.txt");
    std::fs::write(&source, &text).unwrap();
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(FixedGraph(graph)),
    )
    .unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let processed = app.open_source(&imported.info.source_id).unwrap();
    let start = processed
        .knowledge_pages
        .into_iter()
        .find(|page| page.title == "Maya")
        .expect("Maya is the dense start page");

    let batch = app
        .explore_paths(PathExploreRequest {
            start_page_id: start.page_id.clone(),
            max_hops: 1,
            page_size: 50,
            relationship_work_budget: 7,
            continuation: None,
        })
        .unwrap();
    assert_eq!(batch.relationships_examined, 7);
    assert!(batch.paths.len() <= 50);
    assert!(!batch.complete);
    let mut continuation = batch.next_cursor.clone();
    let mut unique_targets = batch
        .paths
        .iter()
        .map(|path| path.target_page_id.clone())
        .collect::<std::collections::HashSet<_>>();
    let mut relationships_examined = batch.relationships_examined;
    let mut maximum_cursor_bytes = continuation.as_ref().unwrap().len();
    let mut total_cursor_bytes = maximum_cursor_bytes;
    let mut maximum_visited_pages = 0_usize;
    let mut calls = 1;
    let mut complete = false;
    while let Some(cursor) = continuation {
        let next = app
            .explore_paths(PathExploreRequest {
                start_page_id: start.page_id.clone(),
                max_hops: 1,
                page_size: 50,
                relationship_work_budget: 7,
                continuation: Some(cursor),
            })
            .unwrap();
        assert!(next.relationships_examined <= 7);
        for path in &next.paths {
            assert!(unique_targets.insert(path.target_page_id.clone()));
        }
        relationships_examined += next.relationships_examined;
        calls += 1;
        continuation = next.next_cursor.clone();
        if let Some(cursor) = &continuation {
            maximum_cursor_bytes = maximum_cursor_bytes.max(cursor.len());
            total_cursor_bytes += cursor.len();
            maximum_visited_pages = maximum_visited_pages.max(unique_targets.len() + 1);
        }
        if next.complete {
            complete = true;
            break;
        }
    }
    println!(
        "dense path experiment: {edge_count} indexed relationships; {calls} calls at budget 7; {relationships_examined} rows inspected; {} unique targets; max opaque cursor {} bytes; cumulative cursor bytes {}; max visited pages {}",
        unique_targets.len(),
        maximum_cursor_bytes,
        total_cursor_bytes,
        maximum_visited_pages
    );
    assert!(complete);
    assert_eq!(relationships_examined, edge_count);
    assert_eq!(unique_targets.len(), edge_count);
    assert!(maximum_cursor_bytes <= 128);
}

#[test]
fn dense_neighborhood_128_inspects_only_the_requested_relationship_budget() {
    assert_dense_neighborhood_inspects_only_the_requested_relationship_budget(128);
}

#[test]
fn dense_neighborhood_160_inspects_only_the_requested_relationship_budget() {
    // This remains below current public Markdown parsing limits for the high-degree Maya page.
    assert_dense_neighborhood_inspects_only_the_requested_relationship_budget(160);
}

#[test]
fn long_chain_paginates_without_loading_the_entire_neighborhood_per_call() {
    const EDGE_COUNT: usize = 256;
    let workspace = tempdir().unwrap();
    let lines = (0..EDGE_COUNT)
        .map(|index| format!("N{index:03} connects to N{:03}.", index + 1))
        .collect::<Vec<_>>();
    let text = lines.join("\n");
    let entities = (0..=EDGE_COUNT)
        .map(|index| {
            let label = format!("N{index:03}");
            EntityDraft {
                kind: "event".into(),
                label: label.clone(),
                evidence: evidence(&text, &label),
            }
        })
        .collect();
    let relationships = lines
        .iter()
        .enumerate()
        .map(|(index, line)| RelationshipDraft {
            from: format!("N{index:03}"),
            to: format!("N{:03}", index + 1),
            kind: "followed_by".into(),
            qualifier: None,
            evidence: evidence(&text, line),
        })
        .collect();
    let source = workspace.path().join("long-chain.txt");
    std::fs::write(&source, &text).unwrap();
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(FixedGraph(KnowledgeDraft {
            entities,
            relationships,
            ..KnowledgeDraft::default()
        })),
    )
    .unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let start = app
        .open_source(&imported.info.source_id)
        .unwrap()
        .knowledge_pages
        .into_iter()
        .find(|page| page.title == "N000")
        .expect("the first chain page is published");

    let mut continuation = None;
    let mut seen = std::collections::HashSet::new();
    let mut calls = 0;
    let mut rows = 0;
    let mut max_cursor_bytes = 0;
    let mut cumulative_cursor_bytes = 0;
    let mut max_visited_pages = 0;
    let mut farthest_detail_token = None;
    loop {
        let batch = app
            .explore_paths(PathExploreRequest {
                start_page_id: start.page_id.clone(),
                max_hops: EDGE_COUNT,
                page_size: 50,
                relationship_work_budget: 7,
                continuation,
            })
            .unwrap();
        calls += 1;
        rows += batch.relationships_examined;
        assert!(batch.relationships_examined <= 7);
        for path in batch.paths {
            assert!(
                serde_json::to_value(&path).unwrap().get("steps").is_none(),
                "stop pages defer route hydration"
            );
            assert!(seen.insert(path.target_page_id.clone()));
            if path.target_title == format!("N{EDGE_COUNT:03}") {
                assert_eq!(path.step_count, EDGE_COUNT);
                farthest_detail_token = Some(path.detail_token);
            }
        }
        let Some(cursor) = batch.next_cursor else {
            assert!(batch.complete);
            break;
        };
        max_cursor_bytes = max_cursor_bytes.max(cursor.len());
        cumulative_cursor_bytes += cursor.len();
        max_visited_pages = max_visited_pages.max(seen.len() + 1);
        continuation = Some(cursor);
    }
    println!(
        "256-edge chain: {calls} calls at budget 7; {rows} rows inspected; {} unique stops; max opaque cursor {max_cursor_bytes} bytes; cumulative cursor bytes {cumulative_cursor_bytes}; max visited pages {max_visited_pages}",
        seen.len()
    );
    // Every reached interior page scans both its incoming and outgoing adjacency;
    // the final page is not expanded at the selected hop limit.
    assert_eq!(rows, EDGE_COUNT * 2 - 1);
    assert_eq!(seen.len(), EDGE_COUNT);
    assert!(
        max_cursor_bytes <= 128,
        "a 256-hop search keeps continuation within 128 bytes despite a long visited chain"
    );

    let detail_token = farthest_detail_token.expect("the farthest route is returned");
    let first_detail_page = app
        .explore_path_details(PathDetailsRequest {
            detail_token: detail_token.clone(),
            continuation: None,
            work_budget: 7,
        })
        .unwrap();
    assert!(first_detail_page.work_units <= 7);
    let mut detail_steps = first_detail_page.steps;
    let mut detail_cursor = first_detail_page.next_cursor;
    let tampered = format!("{}x", detail_cursor.as_deref().unwrap());
    let invalid_detail_cursor = app
        .explore_path_details(PathDetailsRequest {
            detail_token: detail_token.clone(),
            continuation: Some(tampered),
            work_budget: 7,
        })
        .expect_err("tampered selected-path cursor is rejected");
    assert!(invalid_detail_cursor.to_string().contains("cursor"));
    while let Some(cursor) = detail_cursor.take() {
        assert!(cursor.len() <= 128);
        let page = app
            .explore_path_details(PathDetailsRequest {
                detail_token: detail_token.clone(),
                continuation: Some(cursor),
                work_budget: 7,
            })
            .unwrap();
        assert!(page.work_units <= 7);
        detail_steps.extend(page.steps);
        let Some(next) = page.next_cursor else {
            assert!(page.complete);
            break;
        };
        detail_cursor = Some(next);
    }
    detail_steps.reverse();
    assert_eq!(detail_steps.len(), EDGE_COUNT);
    assert_eq!(detail_steps[0].from_title, "N000");
    assert_eq!(detail_steps[0].to_title, "N001");
    assert_eq!(detail_steps[EDGE_COUNT - 1].from_title, "N255");
    assert_eq!(detail_steps[EDGE_COUNT - 1].to_title, "N256");
}

#[test]
fn missing_endpoint_is_reported_without_hiding_other_routes() {
    let workspace = tempdir().unwrap();
    let text = "Maya observed V17. Maya also observed V18. V17 occurred at Riverside. V18 occurred at Riverside. Riverside is self-linked. Riverside Park and Paris are separate places.";
    let source = workspace.path().join("field-note.txt");
    std::fs::write(&source, text).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open_with_semantic_provider(
        &collection,
        Arc::new(FixedGraph(fixture_graph(text))),
    )
    .unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let processed = app.open_source(&imported.info.source_id).unwrap();
    let start = processed
        .knowledge_pages
        .iter()
        .find(|page| page.title == "Maya")
        .unwrap();
    let missing = processed
        .knowledge_pages
        .iter()
        .find(|page| page.title == "Riverside")
        .unwrap();
    let first_batch = app
        .explore_paths(PathExploreRequest {
            start_page_id: start.page_id.clone(),
            max_hops: 2,
            page_size: 1,
            relationship_work_budget: 1,
            continuation: None,
        })
        .unwrap();
    let cursor = first_batch
        .next_cursor
        .expect("a one-edge batch should expose remaining traversal work");

    // Simulate a stale Markdown endpoint, then rebuild the disposable index.
    std::fs::remove_file(collection.join(&missing.path)).unwrap();
    app.rebuild_index().unwrap();
    let stale = app
        .explore_paths(PathExploreRequest {
            start_page_id: start.page_id.clone(),
            max_hops: 2,
            page_size: 1,
            relationship_work_budget: 1,
            continuation: Some(cursor),
        })
        .expect_err("a cursor must not cross a graph revision");
    assert!(stale.to_string().contains("cursor is stale"));
    let explored = app
        .explore_paths(PathExploreRequest {
            start_page_id: start.page_id.clone(),
            max_hops: 2,
            page_size: 16,
            relationship_work_budget: 50,
            continuation: None,
        })
        .unwrap();
    let explored = serde_json::to_value(explored).unwrap();
    assert!(
        explored["paths"]
            .as_array()
            .unwrap()
            .iter()
            .all(|path| path["target_page_id"] != missing.page_id)
    );
    assert!(
        explored["paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path["target_title"] == "V17")
    );
    assert!(
        explored["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| {
                diagnostic["kind"] == "missing_endpoint"
                    && diagnostic["endpoint_page_id"] == missing.page_id
            })
    );
}

#[test]
fn missing_relationship_support_is_reported_without_silently_omitting_the_edge() {
    let workspace = tempdir().unwrap();
    let text = "Maya observed V17. Maya also observed V18.";
    let source = workspace.path().join("field-note.txt");
    std::fs::write(&source, text).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open_with_semantic_provider(
        &collection,
        Arc::new(FixedGraph(KnowledgeDraft {
            entities: vec![entity(text, "person", "Maya"), entity(text, "event", "V17")],
            relationships: vec![RelationshipDraft {
                from: "V17".into(),
                to: "Maya".into(),
                kind: "observed_by".into(),
                qualifier: None,
                evidence: evidence(text, "Maya observed V17"),
            }],
            ..KnowledgeDraft::default()
        })),
    )
    .unwrap();
    let imported = app
        .import_source(&source, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let pages = app
        .open_source(&imported.info.source_id)
        .unwrap()
        .knowledge_pages;
    let maya = pages
        .iter()
        .find(|page| page.title == "Maya")
        .expect("Maya is a published page");
    let v17 = pages
        .iter()
        .find(|page| page.title == "V17")
        .expect("V17 is a published page");
    let maya_page_id = maya.page_id.clone();
    let v17_page_id = v17.page_id.clone();

    // Simulate a disposable index row whose supporting records were lost. The
    // authoritative Markdown remains intact; path exploration must disclose
    // why it omitted the edge rather than silently skipping it.
    let index = Connection::open(collection.join(".derived/lookup.sqlite")).unwrap();
    let (relationship_id, serialized): (String, String) = index
        .query_row(
            "SELECT relationship_id, relationship_json FROM relationship_edges",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let mut relationship: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    relationship["supports"] = serde_json::json!([]);
    index
        .execute(
            "UPDATE relationship_edges SET relationship_json=?1 WHERE relationship_id=?2",
            params![relationship.to_string(), relationship_id],
        )
        .unwrap();
    let result = app
        .explore_paths(PathExploreRequest {
            start_page_id: maya_page_id,
            max_hops: 1,
            page_size: 16,
            relationship_work_budget: 16,
            continuation: None,
        })
        .unwrap();

    assert!(result.paths.is_empty());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].kind,
        knowledge_garden::application::PathExploreDiagnosticKind::MissingSupport
    );
    assert_eq!(result.diagnostics[0].endpoint_page_id, v17_page_id);
}
