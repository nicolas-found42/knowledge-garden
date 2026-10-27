use knowledge_garden::{
    application::{
        collection::{CollectionAxis, CollectionGroupRequest, CollectionMemberRequest},
        AcquisitionMethod, Application,
    },
    semantic::{
        EntityDraft, EvidenceDraft, KnowledgeDraft, ProviderError, SemanticProvider,
        SourceUpdateDraft, SourceUpdateRole, TagDraft,
    },
};
use std::{fs, sync::Arc};
struct Topics;
impl SemanticProvider for Topics {
    fn form_knowledge(&self, text: &str) -> Result<KnowledgeDraft, ProviderError> {
        if !text.contains("Maya") {
            return Err(ProviderError::recoverable("Unclassified fixture".into()));
        }
        let evidence = |quote: &str| {
            let start = text.find(quote).unwrap();
            EvidenceDraft {
                quote: quote.into(),
                byte_start: start,
                byte_end: start + quote.len(),
                origin: "recorded fixture".into(),
                qualifier: Some("Location is uncertain".into()),
                offset_basis: None,
                source_location: None,
            }
        };
        let place = if text.contains("Riverside") {
            "Riverside"
        } else {
            "Bay"
        };
        Ok(KnowledgeDraft {
            entities: vec![
                EntityDraft {
                    kind: "person".into(),
                    label: "Maya".into(),
                    evidence: evidence("Maya"),
                },
                EntityDraft {
                    kind: "place".into(),
                    label: place.into(),
                    evidence: evidence(place),
                },
            ],
            tags: ["fieldwork", "water"]
                .into_iter()
                .filter(|tag| text.contains(tag))
                .map(|tag| TagDraft {
                    subject: "Maya".into(),
                    label: tag.into(),
                    evidence: evidence(tag),
                })
                .collect(),
            source_update: Some(SourceUpdateDraft {
                role: SourceUpdateRole::CompleteReplacement,
                evidence: evidence("Maya"),
                certainty: 1.0,
                source_date: None,
                source_date_evidence: None,
                source_date_certainty: 0.0,
                source_revision: None,
                source_revision_evidence: None,
                source_revision_certainty: 0.0,
            }),
            ..Default::default()
        })
    }
}
#[test]
fn overlapping_subject_and_tag_groups_keep_unclassified_sources_and_refresh_from_current_pages() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("report.txt");
    fs::write(&input, "Maya at Riverside #fieldwork #water.").unwrap();
    let mut app =
        Application::open_with_semantic_provider(temp.path().join("collection"), Arc::new(Topics))
            .unwrap();
    let source = app
        .import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let unknown = temp.path().join("unknown.txt");
    fs::write(&unknown, "Unclassified material").unwrap();
    let unknown = app
        .import_source(unknown, AcquisitionMethod::Picker)
        .unwrap();
    let groups = app.collection_groups(Default::default()).unwrap();
    assert!(groups.groups.iter().any(|g| g.label == "Maya"));
    assert!(groups.groups.iter().any(|g| g.label == "Riverside"));
    assert!(groups.groups.iter().any(|g| g.label == "water"));
    for title in ["Maya", "Riverside"] {
        let group = groups.groups.iter().find(|g| g.label == title).unwrap();
        let members = app
            .collection_members(CollectionMemberRequest {
                group_id: group.group_id.clone(),
                offset: 0,
                revision: Some(groups.revision.clone()),
            })
            .unwrap();
        assert!(members
            .items
            .iter()
            .any(|i| i.page_id == source.info.page_id));
    }
    let unclassified = app
        .collection_members(CollectionMemberRequest {
            group_id: "unclassified".into(),
            offset: 0,
            revision: None,
        })
        .unwrap();
    assert!(unclassified
        .items
        .iter()
        .any(|i| i.page_id == unknown.info.page_id));
    let places = app
        .collection_groups(CollectionGroupRequest {
            axis: CollectionAxis::Place,
            ..Default::default()
        })
        .unwrap();
    assert!(places.groups.iter().all(|g| g.label == "Riverside"));
    fs::write(&input, "Maya at Bay #fieldwork.").unwrap();
    app.import_source(&input, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let fresh = app.collection_groups(Default::default()).unwrap();
    assert_ne!(fresh.revision, groups.revision);
    assert!(fresh.groups.iter().any(|g| g.label == "Bay"));
    assert!(!fresh.groups.iter().any(|g| g.label == "water"));
    let changed = app
        .collection_members(CollectionMemberRequest {
            group_id: "unclassified".into(),
            offset: 50,
            revision: Some(groups.revision),
        })
        .unwrap();
    assert!(changed.changed);
    assert!(changed
        .items
        .iter()
        .any(|i| i.page_id == unknown.info.page_id));
    app.rebuild_index().unwrap();
    let rebuilt = app.collection_groups(Default::default()).unwrap();
    assert_eq!(fresh.revision, rebuilt.revision);
    assert!(rebuilt.groups.iter().any(|g| g.label == "Bay"));
}
#[test]
fn collection_members_continue_past_fifty_and_literal_filters_do_not_become_sql() {
    let temp = tempfile::tempdir().unwrap();
    let mut app = Application::open(temp.path().join("collection")).unwrap();
    for i in 0..55 {
        let input = temp.path().join(format!("source-{i}.txt"));
        fs::write(&input, format!("Unclassified item {i}")).unwrap();
        app.import_source(input, AcquisitionMethod::Picker).unwrap();
    }
    let groups = app.collection_groups(Default::default()).unwrap();
    let group = groups
        .groups
        .iter()
        .find(|g| g.group_id == "unclassified")
        .unwrap();
    assert_eq!(group.members, 55);
    let first = app
        .collection_members(CollectionMemberRequest {
            group_id: group.group_id.clone(),
            offset: 0,
            revision: Some(groups.revision.clone()),
        })
        .unwrap();
    assert_eq!(first.items.len(), 50);
    assert_eq!(first.next_offset, Some(50));
    let next = app
        .collection_members(CollectionMemberRequest {
            group_id: group.group_id.clone(),
            offset: 50,
            revision: Some(groups.revision),
        })
        .unwrap();
    assert_eq!(next.items.len(), 5);
    assert!(next.next_offset.is_none());
    assert!(!first
        .items
        .iter()
        .any(|a| next.items.iter().any(|b| a.page_id == b.page_id)));
    let filtered = app
        .collection_groups(CollectionGroupRequest {
            query: "' OR 1=1 --".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(filtered.groups.is_empty());
    let dates = app
        .collection_groups(CollectionGroupRequest {
            axis: CollectionAxis::Date,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(dates.groups[0].label, "Unknown event date");
}

#[test]
fn organization_updates_only_changed_source_memberships_and_rebuilds_without_rewriting_them() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let first_path = temp.path().join("first.txt");
    let second_path = temp.path().join("second.txt");
    fs::write(&first_path, "Maya at Riverside #water.").unwrap();
    fs::write(&second_path, "Maya at Bay #fieldwork.").unwrap();
    let mut app = Application::open_with_semantic_provider(&root, Arc::new(Topics)).unwrap();
    let first = app
        .import_source(&first_path, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let second = app
        .import_source(&second_path, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let audit = rusqlite::Connection::open(root.join(".derived/lookup.sqlite")).unwrap();
    audit.execute_batch("CREATE TABLE group_writes(source_id TEXT); CREATE TRIGGER group_insert AFTER INSERT ON collection_source_pages BEGIN INSERT INTO group_writes VALUES(NEW.source_id); END; CREATE TRIGGER group_delete AFTER DELETE ON collection_source_pages BEGIN INSERT INTO group_writes VALUES(OLD.source_id); END;").unwrap();
    app.rebuild_index().unwrap();
    let writes: i64 = audit
        .query_row("SELECT COUNT(*) FROM group_writes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(writes, 0);
    fs::write(&first_path, "Maya at Bay #fieldwork.").unwrap();
    app.import_source(&first_path, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let unrelated: i64 = audit
        .query_row(
            "SELECT COUNT(*) FROM group_writes WHERE source_id=?1",
            [&second.info.source_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(unrelated, 0);
    let affected: i64 = audit
        .query_row(
            "SELECT COUNT(*) FROM group_writes WHERE source_id=?1",
            [&first.info.source_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(affected > 0);
}
