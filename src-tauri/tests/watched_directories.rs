use knowledge_garden::application::AcquisitionMethod;
use knowledge_garden::application::Application;
use knowledge_garden::application::PageSearchRequest;
use knowledge_garden::semantic::{
    EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft, ProviderError, SemanticProvider,
    SourceUpdateDraft, SourceUpdateRole,
};
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

struct FixtureProvider;
impl SemanticProvider for FixtureProvider {
    fn form_knowledge(&self, text: &str) -> Result<KnowledgeDraft, ProviderError> {
        let evidence = |quote: &str| EvidenceDraft {
            quote: quote.into(),
            byte_start: text.find(quote).unwrap(),
            byte_end: text.find(quote).unwrap() + quote.len(),
            origin: "observed".into(),
            qualifier: None,
            offset_basis: None,
            source_location: None,
        };
        let mut entities = vec![EntityDraft {
            kind: "place".into(),
            label: "Riverside".into(),
            evidence: evidence("Riverside"),
        }];
        let mut facts = vec![FactDraft {
            subject: "Riverside".into(),
            property: "duration".into(),
            value: "10 minutes".into(),
            evidence: evidence("Duration: 10 minutes."),
            record_key: None,
        }];
        if text.contains("Observation V17") {
            entities.push(EntityDraft {
                kind: "event".into(),
                label: "Observation V17".into(),
                evidence: evidence("Observation V17"),
            });
            let value = if text.contains("Visits: 15") {
                "15"
            } else {
                "12"
            };
            facts.push(FactDraft {
                subject: "Observation V17".into(),
                property: "visits".into(),
                value: value.into(),
                evidence: evidence(&format!("Visits: {value}.")),
                record_key: None,
            });
        }
        let revision = if text.contains("revision 3") {
            3
        } else if text.contains("revision 2") {
            2
        } else {
            1
        };
        let revision_quote = format!("revision {revision}");
        Ok(KnowledgeDraft {
            entities,
            facts,
            source_update: Some(SourceUpdateDraft {
                role: SourceUpdateRole::CompleteReplacement,
                evidence: evidence(text.lines().next().unwrap()),
                certainty: 0.99,
                source_date: None,
                source_date_evidence: None,
                source_date_certainty: 0.0,
                source_revision: Some(revision),
                source_revision_evidence: Some(evidence(&revision_quote)),
                source_revision_certainty: 0.99,
            }),
            ..KnowledgeDraft::default()
        })
    }
}

const V17: &str =
    "Complete report revision 1.\nObservation V17. Visits: 12. Riverside. Duration: 10 minutes.\n";
const INDEPENDENT: &str = "Independent report revision 1.\nRiverside. Duration: 10 minutes.\n";

fn open(collection: &std::path::Path) -> Application {
    Application::open_with_semantic_provider(collection, Arc::new(FixtureProvider)).unwrap()
}

#[test]
fn actual_removal_withdraws_only_sole_support_and_preserves_external_manual_authority() {
    for corrected in [false, true] {
        let workspace = tempdir().unwrap();
        let incoming = workspace.path().join("incoming");
        fs::create_dir(&incoming).unwrap();
        fs::write(incoming.join("V17.txt"), V17).unwrap();
        let collection = workspace.path().join("collection");
        let mut app = open(&collection);
        app.watch_directory(&incoming).unwrap();
        scan(&mut app);
        app.resume_due_semantic_jobs().unwrap();
        let source_id = app.list_sources(0).unwrap().sources[0].source_id.clone();
        let source = app.open_source(&source_id).unwrap();
        let original = app.original_path(&source_id).unwrap();
        let event = source
            .knowledge_pages
            .iter()
            .find(|page| page.title == "Observation V17")
            .unwrap();
        let place = source
            .knowledge_pages
            .iter()
            .find(|page| page.title == "Riverside")
            .unwrap();
        let event_path = collection.join(&event.path);
        if corrected {
            let markdown = fs::read_to_string(&event_path)
                .unwrap()
                .replace("- **visits:** 12", "- **visits:** 99");
            assert!(markdown.contains("- **visits:** 99"));
            fs::write(&event_path, markdown).unwrap();
        }
        fs::write(incoming.join("independent.txt"), INDEPENDENT).unwrap();
        scan(&mut app);
        app.resume_due_semantic_jobs().unwrap();
        assert!(app
            .open_knowledge_page(&place.page_id)
            .unwrap()
            .markdown
            .contains("Source support count: 2"));
        fs::remove_file(incoming.join("V17.txt")).unwrap();
        scan(&mut app);
        scan(&mut app);
        assert!(!original.exists());
        assert!(app.open_source(&source_id).is_err());
        let removed = app.list_removed_sources().unwrap();
        let backup = removed
            .iter()
            .find(|item| item.source_id == source_id)
            .unwrap();
        assert_eq!(
            fs::read_to_string(
                collection
                    .join(".recovery/sources")
                    .join(&backup.recovery_id)
                    .join("bundle/original.txt")
            )
            .unwrap(),
            V17
        );
        let retained_place = app.open_knowledge_page(&place.page_id).unwrap();
        assert!(retained_place.markdown.contains("10 minutes"));
        assert!(retained_place.markdown.contains("Source support count: 1"));
        if corrected {
            let retained = app.open_knowledge_page(&event.page_id).unwrap();
            assert!(retained.markdown.contains("- **visits:** 99"));
            assert!(retained.markdown.contains("manual_correction:"));
        } else {
            assert!(app.open_knowledge_page(&event.page_id).is_err());
        }
        let results = app
            .search_pages(PageSearchRequest {
                query: "Observation V17".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            results
                .pages
                .iter()
                .any(|page| page.page_id == event.page_id),
            corrected
        );
        assert_eq!(
            app.timeline(knowledge_garden::timeline::TimelineRequest::default())
                .unwrap()
                .entries
                .iter()
                .any(|entry| entry.page_id == event.page_id),
            corrected
        );
        assert!(app
            .search_pages(PageSearchRequest {
                query: "10 minutes".into(),
                ..Default::default()
            })
            .unwrap()
            .pages
            .iter()
            .any(|page| page.page_id == place.page_id));
        drop(app);
        let app = open(&collection);
        assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
        assert!(app
            .open_knowledge_page(&place.page_id)
            .unwrap()
            .markdown
            .contains("10 minutes"));
    }
}

fn scan(app: &mut Application) {
    for _ in 0..1000 {
        app.reconcile_watched_directories(1).unwrap();
        if app.list_watched_directories().unwrap()[0].state != "scanning" {
            return;
        }
    }
    panic!("bounded scan did not finish");
}

#[test]
fn interrupted_discovery_disconnect_reconnect_and_confirmed_removal_preserve_retained_assets() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    fs::write(incoming.join("V17.txt"), "Observation V17 at Riverside.").unwrap();
    fs::write(incoming.join("independent.txt"), "Independent observation.").unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    app.watch_directory(&incoming).unwrap();
    app.reconcile_watched_directories(1).unwrap();
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
    let first = app.list_sources(0).unwrap().sources[0].source_id.clone();
    assert!(app.original_path(&first).unwrap().exists());
    drop(app);

    let mut app = Application::open(&collection).unwrap();
    scan(&mut app);
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 2);
    fs::rename(&incoming, workspace.path().join("disconnected")).unwrap();
    scan(&mut app);
    assert_eq!(
        app.list_watched_directories().unwrap()[0].state,
        "unavailable"
    );
    assert!(app.original_path(&first).unwrap().exists());
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 2);

    // An empty mount point at the same pathname is not the selected directory.
    fs::create_dir(&incoming).unwrap();
    scan(&mut app);
    scan(&mut app);
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 2);
    fs::remove_dir(&incoming).unwrap();
    fs::rename(workspace.path().join("disconnected"), &incoming).unwrap();
    scan(&mut app);
    assert_eq!(app.list_watched_directories().unwrap()[0].state, "watching");
    fs::remove_file(incoming.join("V17.txt")).unwrap();
    scan(&mut app);
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 2);
    scan(&mut app);
    let remaining = app.list_sources(0).unwrap();
    assert_eq!(remaining.sources.len(), 1);
    assert_eq!(remaining.sources[0].title, "independent");
    drop(app);
    assert_eq!(
        Application::open(&collection)
            .unwrap()
            .list_sources(0)
            .unwrap()
            .sources
            .len(),
        1
    );
}

#[test]
fn watching_an_existing_origin_keeps_its_identity_and_other_acquisitions() {
    for independent in [false, true] {
        let workspace = tempdir().unwrap();
        let incoming = workspace.path().join("incoming");
        let nested = incoming.join("nested");
        fs::create_dir_all(&nested).unwrap();
        let path = nested.join("V17.txt");
        fs::write(&path, V17).unwrap();
        let mut app = open(&workspace.path().join("collection"));
        let initial = app.import_source(&path, AcquisitionMethod::Picker).unwrap();
        app.resume_due_semantic_jobs().unwrap();
        if independent {
            let other = workspace.path().join("independent.txt");
            fs::write(&other, V17).unwrap();
            assert_eq!(
                app.import_source(other, AcquisitionMethod::Picker)
                    .unwrap()
                    .info
                    .source_id,
                initial.info.source_id
            );
        }
        app.watch_directory(&incoming).unwrap();
        scan(&mut app);
        assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
        assert_eq!(
            app.open_source(&initial.info.source_id)
                .unwrap()
                .info
                .versions_seen
                .len(),
            1
        );
        fs::remove_dir_all(&nested).unwrap();
        scan(&mut app);
        scan(&mut app);
        assert!(app.retained_source_exists(&initial.info.source_id).unwrap());
        let retained = app.open_source(&initial.info.source_id).unwrap();
        assert_eq!(
            retained.info.acquisitions.len(),
            if independent { 2 } else { 1 }
        );
        assert!(retained
            .info
            .acquisitions
            .iter()
            .all(|acquisition| acquisition.method == AcquisitionMethod::Picker));
        assert!(app.original_path(&initial.info.source_id).unwrap().exists());
    }
}

#[test]
fn repository_escaped_origins_stay_distinct_through_watching_changes_and_removal() {
    for (first_name, second_name) in [("a%b.txt", "a%25b.txt"), ("a\\b.txt", "a%5Cb.txt")] {
        let workspace = tempdir().unwrap();
        let incoming = workspace.path().join("incoming");
        fs::create_dir(&incoming).unwrap();
        let first_path = incoming.join(first_name);
        let second_path = incoming.join(second_name);
        fs::write(&first_path, V17).unwrap();
        fs::write(&second_path, INDEPENDENT).unwrap();
        let collection = workspace.path().join("collection");
        let mut app = open(&collection);
        let imported = app.import_repository(&incoming).unwrap();
        let encoded_name = |name: &str| name.replace('%', "%25").replace('\\', "%5C");
        let source_for = |name: &str| {
            imported
                .sources
                .iter()
                .find(|source| source.relative_path == encoded_name(name))
                .unwrap()
                .source_id
                .clone()
        };
        let first_id = source_for(first_name);
        let second_id = source_for(second_name);
        assert_ne!(first_id, second_id);
        app.resume_due_semantic_jobs().unwrap();
        drop(app);
        let mut app = open(&collection);
        // Change before adopting the watch: content deduplication cannot hide a wrong origin key.
        let changed = INDEPENDENT.replace("revision 1", "revision 2");
        fs::write(&second_path, &changed).unwrap();
        app.watch_directory(&incoming).unwrap();
        scan(&mut app);
        assert_eq!(app.list_sources(0).unwrap().sources.len(), 2);
        let first = app.open_source(&first_id).unwrap();
        let second = app.open_source(&second_id).unwrap();
        assert_eq!(
            first.info.versions_seen.len(),
            1,
            "wrong origin adopted for {second_name}"
        );
        assert_eq!(second.info.versions_seen.len(), 2);
        assert_eq!(
            fs::read(app.original_path(&first_id).unwrap()).unwrap(),
            V17.as_bytes()
        );
        fs::remove_file(&second_path).unwrap();
        scan(&mut app);
        scan(&mut app);
        assert!(app.retained_source_exists(&second_id).unwrap());
        assert!(app.retained_source_exists(&first_id).unwrap());
        drop(app);
        let app = open(&collection);
        assert_eq!(app.list_sources(0).unwrap().sources.len(), 2);
        assert_eq!(
            fs::read(app.original_path(&first_id).unwrap()).unwrap(),
            V17.as_bytes()
        );
    }
}

#[test]
fn legacy_repository_origins_remain_raw_and_ambiguous_records_are_preserved() {
    fn rewrite_repository_acquisition(
        path: &std::path::Path,
        raw_path: &str,
        encoding: Option<&str>,
    ) {
        fn update(value: &mut serde_yaml_ng::Value, raw_path: &str, encoding: Option<&str>) {
            if value["method"].as_str() == Some("repository") {
                value["path"] = raw_path.into();
                let fields = value.as_mapping_mut().unwrap();
                fields.remove(serde_yaml_ng::Value::from("path_encoding"));
                if let Some(encoding) = encoding {
                    fields.insert("path_encoding".into(), encoding.into());
                }
            }
        }
        let markdown = fs::read_to_string(path).unwrap();
        let (header, body) = markdown
            .strip_prefix("---\n")
            .unwrap()
            .split_once("\n---\n")
            .unwrap();
        let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_str(header).unwrap();
        if let Some(acquisitions) = value["acquisitions"].as_sequence_mut() {
            for acquisition in acquisitions {
                update(acquisition, raw_path, encoding);
            }
        } else {
            update(&mut value, raw_path, encoding);
        }
        fs::write(
            path,
            format!(
                "---\n{}---\n{body}",
                serde_yaml_ng::to_string(&value).unwrap()
            ),
        )
        .unwrap();
    }
    for ambiguous in [false, true] {
        let workspace = tempdir().unwrap();
        let incoming = workspace.path().join("incoming");
        fs::create_dir(&incoming).unwrap();
        let names = if ambiguous {
            ["a%b.txt", "a%25b.txt"]
        } else {
            ["a%b.txt", "a\\b.txt"]
        };
        let paths = names.map(|name| incoming.join(name));
        fs::write(&paths[0], V17).unwrap();
        fs::write(&paths[1], INDEPENDENT).unwrap();
        let collection = workspace.path().join("collection");
        let mut app = open(&collection);
        let imported = app.import_repository(&incoming).unwrap();
        let ids = names.map(|name| {
            imported
                .sources
                .iter()
                .find(|source| {
                    source.relative_path == name.replace('%', "%25").replace('\\', "%5C")
                })
                .unwrap()
                .source_id
                .clone()
        });
        app.watch_directory(&incoming).unwrap();
        scan(&mut app);
        let page_paths = ids.each_ref().map(|id| app.page_path(id).unwrap());
        drop(app);
        let rewrite = |encoding| {
            for (page_path, raw_path) in page_paths.iter().zip(&paths) {
                rewrite_repository_acquisition(page_path, raw_path.to_str().unwrap(), encoding);
                for entry in fs::read_dir(page_path.parent().unwrap().join("acquisitions")).unwrap()
                {
                    rewrite_repository_acquisition(
                        &entry.unwrap().path(),
                        raw_path.to_str().unwrap(),
                        encoding,
                    );
                }
            }
        };
        // Simulate the pre-encoding on-disk contract, including acquisition sidecars.
        rewrite(None);
        let mut app = open(&collection);
        fs::write(&paths[1], INDEPENDENT.replace("revision 1", "revision 2")).unwrap();
        scan(&mut app);
        assert_eq!(
            app.open_source(&ids[0]).unwrap().info.versions_seen.len(),
            1
        );
        assert_eq!(
            app.open_source(&ids[1]).unwrap().info.versions_seen.len(),
            if ambiguous { 1 } else { 2 }
        );
        if ambiguous {
            let watch = &app.list_watched_directories().unwrap()[0];
            assert_eq!(watch.state, "unavailable");
            assert!(watch
                .detail
                .as_deref()
                .unwrap()
                .contains("ambiguous encoding"));
        }
        fs::remove_file(&paths[1]).unwrap();
        scan(&mut app);
        scan(&mut app);
        assert!(app.retained_source_exists(&ids[1]).unwrap());
        assert_eq!(
            fs::read(app.original_path(&ids[0]).unwrap()).unwrap(),
            V17.as_bytes()
        );
        if ambiguous {
            drop(app);
            // An explicit raw interpretation removes quarantine without guessing.
            rewrite(Some("raw"));
            // Refresh the remaining input so its prior failed-input backoff is
            // invalidated; otherwise recovery waits for the normal retry deadline.
            fs::write(&paths[0], V17).unwrap();
            let mut app = open(&collection);
            scan(&mut app);
            scan(&mut app);
            assert!(app.retained_source_exists(&ids[1]).unwrap());
            assert!(app.retained_source_exists(&ids[0]).unwrap());
        }
    }
}

#[test]
fn identical_watched_acquisitions_share_a_source_until_the_last_origin_is_removed() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    fs::write(incoming.join("first.txt"), V17).unwrap();
    fs::write(incoming.join("copy.txt"), V17).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
    let source_id = app.list_sources(0).unwrap().sources[0].source_id.clone();
    assert_eq!(
        app.open_source(&source_id).unwrap().info.acquisitions.len(),
        2
    );
    app.rebuild_index().unwrap();
    fs::write(
        incoming.join("copy.txt"),
        V17.replace("revision 1", "revision 2"),
    )
    .unwrap();
    scan(&mut app);
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
    assert_eq!(
        app.open_source(&source_id)
            .unwrap()
            .info
            .versions_seen
            .len(),
        2
    );
    fs::remove_file(incoming.join("first.txt")).unwrap();
    scan(&mut app);
    scan(&mut app);
    assert!(app.retained_source_exists(&source_id).unwrap());
    assert_eq!(
        app.open_source(&source_id).unwrap().info.acquisitions.len(),
        1
    );
    fs::remove_file(incoming.join("copy.txt")).unwrap();
    scan(&mut app);
    scan(&mut app);
    assert!(!app.retained_source_exists(&source_id).unwrap());
}

#[test]
fn missed_changes_atomic_replacements_identical_content_and_older_versions_use_the_import_rules() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    let source_path = incoming.join("V17.txt");
    fs::write(&source_path, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let source = app.list_sources(0).unwrap().sources[0].source_id.clone();
    let first = app.open_source(&source).unwrap();
    let event = first
        .knowledge_pages
        .iter()
        .find(|page| page.title == "Observation V17")
        .unwrap()
        .page_id
        .clone();
    scan(&mut app);
    assert_eq!(
        app.open_source(&source).unwrap().info.versions_seen.len(),
        1
    );
    drop(app);

    // No process or event subscription exists when these edits occur.
    let replacement = workspace.path().join("replacement.txt");
    fs::write(
        &replacement,
        V17.replace("revision 1", "revision 3")
            .replace("Visits: 12", "Visits: 15"),
    )
    .unwrap();
    fs::rename(&replacement, &source_path).unwrap();
    fs::write(incoming.join("new.txt"), INDEPENDENT).unwrap();
    let mut app = open(&collection);
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 2);
    assert!(app
        .open_knowledge_page(&event)
        .unwrap()
        .markdown
        .contains("- **visits:** 15"));
    assert_eq!(
        app.open_source(&source).unwrap().info.versions_seen.len(),
        2
    );

    fs::write(&source_path, V17.replace("revision 1", "revision 2")).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    assert!(app
        .open_knowledge_page(&event)
        .unwrap()
        .markdown
        .contains("- **visits:** 15"));
    assert_eq!(
        app.open_source(&source).unwrap().info.versions_seen.len(),
        3
    );
    scan(&mut app);
    assert_eq!(
        app.open_source(&source).unwrap().info.versions_seen.len(),
        3
    );
}

#[test]
fn unreadable_subdirectory_and_unreadable_files_cannot_supply_deletion_evidence() {
    use std::os::unix::fs::PermissionsExt;
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    let nested = incoming.join("nested");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("V17.txt"), V17).unwrap();
    fs::write(incoming.join("other.txt"), INDEPENDENT).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    fs::set_permissions(&nested, fs::Permissions::from_mode(0o000)).unwrap();
    scan(&mut app);
    assert_eq!(
        app.list_watched_directories().unwrap()[0].state,
        "unavailable"
    );
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 2);
    fs::set_permissions(&nested, fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(
        incoming.join("other.txt"),
        fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    fs::write(incoming.join("later.txt"), "A later readable source.").unwrap();
    scan(&mut app);
    assert_eq!(
        app.list_watched_directories().unwrap()[0].state,
        "unavailable"
    );
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 3);
    fs::set_permissions(
        incoming.join("other.txt"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    scan(&mut app);
    assert_eq!(app.list_watched_directories().unwrap()[0].state, "watching");
}

#[test]
fn interrupted_withdrawal_recovers_markdown_and_asset_removal_together() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    fs::write(incoming.join("V17.txt"), V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let id = app.list_sources(0).unwrap().sources[0].source_id.clone();
    let original = app.original_path(&id).unwrap();
    let event = app
        .open_source(&id)
        .unwrap()
        .knowledge_pages
        .into_iter()
        .find(|page| page.title == "Observation V17")
        .unwrap();
    let path = collection.join(&event.path);
    fs::write(
        &path,
        fs::read_to_string(&path)
            .unwrap()
            .replace("- **visits:** 12", "- **visits:** 99"),
    )
    .unwrap();
    fs::remove_file(incoming.join("V17.txt")).unwrap();
    scan(&mut app);
    app.set_publication_failpoint_for_test(1);
    scan(&mut app);
    assert_eq!(
        app.list_watched_directories().unwrap()[0].state,
        "unavailable"
    );
    drop(app);
    let mut app = open(&collection);
    assert!(!original.exists());
    assert!(app.list_sources(0).unwrap().sources.is_empty());
    assert!(app
        .open_knowledge_page(&event.page_id)
        .unwrap()
        .markdown
        .contains("- **visits:** 99"));
    assert!(app
        .search_pages(PageSearchRequest {
            query: "Observation V17".into(),
            ..Default::default()
        })
        .unwrap()
        .pages
        .iter()
        .any(|page| page.page_id == event.page_id));
}

#[test]
fn failed_withdrawal_cannot_replay_over_a_later_reimport_in_the_same_process() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    let path = incoming.join("V17.txt");
    fs::write(&path, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let source_id = app.list_sources(0).unwrap().sources[0].source_id.clone();
    let event = app
        .open_source(&source_id)
        .unwrap()
        .knowledge_pages
        .into_iter()
        .find(|page| page.title == "Observation V17")
        .unwrap();
    let page_path = collection.join(event.path);
    fs::write(
        &page_path,
        fs::read_to_string(&page_path)
            .unwrap()
            .replace("- **visits:** 12", "- **visits:** 99"),
    )
    .unwrap();
    fs::remove_file(&path).unwrap();
    scan(&mut app);
    app.set_publication_failpoint_for_test(1);
    scan(&mut app);
    assert_eq!(
        app.list_watched_directories().unwrap()[0].state,
        "unavailable"
    );
    // Recreate identical content, so it has the same retained source identity.
    fs::write(&path, V17).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    assert!(app.original_path(&source_id).unwrap().exists());
    drop(app);
    let app = open(&collection);
    assert!(app.retained_source_exists(&source_id).unwrap());
    assert!(app.original_path(&source_id).unwrap().exists());
    assert!(app
        .open_knowledge_page(&event.page_id)
        .unwrap()
        .markdown
        .contains("- **visits:** 99"));
}

#[cfg(target_os = "macos")]
#[test]
fn mounted_volume_disconnect_empty_mountpoint_and_remount_recover_automatically() {
    check_mounted_volume(false);
    check_mounted_volume(true);
}

#[cfg(target_os = "macos")]
fn check_mounted_volume(nested: bool) {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    struct Mounted(Option<PathBuf>);
    impl Mounted {
        fn detach(&mut self) {
            hdiutil(&["detach"], self.0.as_ref().unwrap());
            self.0 = None;
        }
    }
    impl Drop for Mounted {
        fn drop(&mut self) {
            if let Some(device) = &self.0 {
                let _ = Command::new("/usr/bin/hdiutil")
                    .arg("detach")
                    .arg(device)
                    .output();
            }
        }
    }
    fn hdiutil(args: &[&str], path: &Path) {
        for attempt in 0..3 {
            let output = Command::new("/usr/bin/hdiutil")
                .args(args)
                .arg(path)
                .output()
                .unwrap();
            if output.status.success() {
                return;
            }
            let error = String::from_utf8_lossy(&output.stderr);
            // macOS services can briefly hold a freshly mounted fixture busy.
            // Require a real, non-forced detach; persistent handles still fail.
            if args == ["detach"] && error.contains("Resource busy") && attempt < 2 {
                std::thread::sleep(std::time::Duration::from_millis(250));
                continue;
            }
            panic!("hdiutil {args:?} {}: {error}", path.display());
        }
    }
    let workspace = tempdir().unwrap();
    let image = workspace.path().join("fixture.dmg");
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    let mounted = incoming.join("mounted");
    hdiutil(
        &[
            "create",
            "-size",
            "32m",
            "-fs",
            "HFS+",
            "-volname",
            "KGWatchFixture",
        ],
        &image,
    );
    fs::create_dir(&mounted).unwrap();
    let attach = || {
        let output = Command::new("/usr/bin/hdiutil")
            .args(["attach", "-nobrowse", "-mountpoint"])
            .arg(&mounted)
            .arg(&image)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        // Detach the device returned by this attach, rather than asking hdiutil
        // to resolve the temporary mountpoint back to a device on the runner.
        let stdout = String::from_utf8(output.stdout).unwrap();
        let device = stdout
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .find(|field| field.starts_with("/dev/disk"))
            .unwrap_or_else(|| panic!("attach returned no device: {stdout}"));
        Mounted(Some(PathBuf::from(device)))
    };
    let mut guard = attach();
    fs::write(mounted.join("V17.txt"), V17).unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    app.watch_directory(if nested { &incoming } else { &mounted })
        .unwrap();
    scan(&mut app);
    let source = app
        .list_sources(0)
        .unwrap()
        .sources
        .into_iter()
        .find(|page| page.title == "V17")
        .unwrap();
    let original = app.original_path(&source.source_id).unwrap();
    guard.detach();
    fs::create_dir_all(&mounted).unwrap();
    scan(&mut app);
    scan(&mut app);
    assert_eq!(
        app.list_watched_directories().unwrap()[0].state,
        "unavailable"
    );
    assert!(original.exists());
    let _remounted = attach();
    scan(&mut app);
    assert_eq!(app.list_watched_directories().unwrap()[0].state, "watching");
    assert_eq!(
        app.open_source(&source.source_id)
            .unwrap()
            .info
            .versions_seen
            .len(),
        1
    );
    if nested {
        let subfolder = mounted.join("ordinary-subfolder");
        fs::create_dir(&subfolder).unwrap();
        fs::write(subfolder.join("independent.txt"), INDEPENDENT).unwrap();
        scan(&mut app);
        assert_eq!(app.list_sources(0).unwrap().sources.len(), 2);
        fs::remove_dir_all(&subfolder).unwrap();
        scan(&mut app);
        scan(&mut app);
        assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
        assert!(app.retained_source_exists(&source.source_id).unwrap());
    }
    fs::remove_file(mounted.join("V17.txt")).unwrap();
    scan(&mut app);
    scan(&mut app);
    assert!(!original.exists());
}

#[test]
fn corrupt_watch_identity_stays_inspectable_without_blocking_a_healthy_watch() {
    let workspace = tempdir().unwrap();
    let collection = workspace.path().join("collection");
    let bad_root = workspace.path().join("bad");
    let good_root = workspace.path().join("good");
    let outside = workspace.path().join("outside");
    for path in [&bad_root, &good_root, &outside] {
        fs::create_dir(path).unwrap();
    }
    fs::write(good_root.join("report.txt"), V17).unwrap();
    fs::write(outside.join("index.md"), "external sentinel").unwrap();
    let mut app = Application::open(&collection).unwrap();
    let bad = app.watch_directory(&bad_root).unwrap();
    app.watch_directory(&good_root).unwrap();
    let record = collection
        .join("watches")
        .join(&bad.watch_id)
        .join("index.md");
    let crafted = fs::read_to_string(&record)
        .unwrap()
        .replace(&bad.watch_id, outside.to_str().unwrap());
    fs::write(&record, &crafted).unwrap();
    app.reconcile_watched_directories(128).unwrap();
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
    assert!(app
        .list_watched_directories()
        .unwrap()
        .iter()
        .any(|watch| watch.state == "invalid_record"));
    assert_eq!(fs::read_to_string(&record).unwrap(), crafted);
    assert_eq!(
        fs::read_to_string(outside.join("index.md")).unwrap(),
        "external sentinel"
    );
}

#[test]
fn removal_recovery_refuses_a_symlinked_collection_ancestor() {
    use std::os::unix::fs::symlink;
    let workspace = tempdir().unwrap();
    let collection = workspace.path().join("collection");
    drop(Application::open(&collection).unwrap());
    let outside = workspace.path().join("outside");
    fs::create_dir_all(outside.join("victim")).unwrap();
    fs::write(outside.join("victim/sentinel"), "keep me").unwrap();
    fs::remove_dir(collection.join("sources")).unwrap();
    symlink(&outside, collection.join("sources")).unwrap();
    let transaction = collection.join(".staging/transactions/escape");
    fs::create_dir_all(&transaction).unwrap();
    fs::write(
        transaction.join("manifest.json"),
        r#"{"entries":[],"removals":["sources/victim"]}"#,
    )
    .unwrap();
    assert!(Application::open(&collection).is_err());
    assert_eq!(
        fs::read_to_string(outside.join("victim/sentinel")).unwrap(),
        "keep me"
    );
    assert!(transaction.join("manifest.json").exists());
}

#[test]
fn multiple_watches_share_one_turn_budget_and_make_fair_progress() {
    let workspace = tempdir().unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    for index in 0..3 {
        let path = workspace.path().join(index.to_string());
        fs::create_dir(&path).unwrap();
        for file in 0..8 {
            fs::write(path.join(format!("{file}.txt")), format!("{index}-{file}")).unwrap();
        }
        app.watch_directory(path).unwrap();
    }
    app.reconcile_watched_directories(2).unwrap();
    assert!(
        app.list_watched_directories()
            .unwrap()
            .iter()
            .map(|watch| watch.processed_files)
            .sum::<u64>()
            <= 2
    );
    for _ in 0..8 {
        app.reconcile_watched_directories(2).unwrap();
    }
    assert!(app
        .list_watched_directories()
        .unwrap()
        .iter()
        .all(|watch| watch.processed_files > 0));
}

#[test]
fn failed_file_retry_deadline_survives_restart_and_changes_reset_it() {
    use std::os::unix::fs::PermissionsExt;
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    let source = incoming.join("report.txt");
    fs::write(&source, V17).unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o000)).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    let watch = app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    let retry = fs::read_dir(
        collection
            .join("watches")
            .join(&watch.watch_id)
            .join("retries"),
    )
    .unwrap()
    .next()
    .unwrap()
    .unwrap()
    .path();
    let before = fs::read(&retry).unwrap();
    drop(app);
    let mut app = Application::open(&collection).unwrap();
    scan(&mut app);
    assert_eq!(fs::read(&retry).unwrap(), before);
    assert!(app.list_sources(0).unwrap().sources.is_empty());
    fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
    scan(&mut app);
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
    assert!(!retry.exists());
}

#[test]
fn unavailable_watch_roots_still_consume_the_shared_turn_budget() {
    let workspace = tempdir().unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    for index in 0..3 {
        let incoming = workspace.path().join(index.to_string());
        fs::create_dir(&incoming).unwrap();
        app.watch_directory(&incoming).unwrap();
        fs::remove_dir(&incoming).unwrap();
    }
    app.reconcile_watched_directories(1).unwrap();
    assert_eq!(
        app.list_watched_directories()
            .unwrap()
            .iter()
            .filter(|watch| watch.state == "unavailable")
            .count(),
        1
    );
    app.reconcile_watched_directories(1).unwrap();
    assert_eq!(
        app.list_watched_directories()
            .unwrap()
            .iter()
            .filter(|watch| watch.state == "unavailable")
            .count(),
        2
    );
}

#[test]
fn explicit_removal_preserves_backup_and_suppresses_changed_watched_origin_until_readd() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    let file = incoming.join("V17.txt");
    fs::write(&file, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let id = app.list_sources(0).unwrap().sources[0].source_id.clone();
    app.remove_source(&id).unwrap();
    assert!(app.list_sources(0).unwrap().sources.is_empty());
    assert_eq!(fs::read_to_string(&file).unwrap(), V17);
    let backups = collection.join(".recovery/sources");
    let backup = fs::read_dir(backups)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(backup.join("completed").exists());
    assert_eq!(
        fs::read_to_string(backup.join("bundle/original.txt")).unwrap(),
        V17
    );
    drop(app);
    fs::write(&file, V17.replace("Visits: 12", "Visits: 15")).unwrap();
    let mut app = open(&collection);
    scan(&mut app);
    assert!(app.list_sources(0).unwrap().sources.is_empty());
    let removed = app.list_removed_sources().unwrap();
    assert_eq!(removed.len(), 1);
    let restored = app.restore_removed_source(&removed[0].recovery_id).unwrap();
    assert_eq!(
        fs::read_to_string(app.original_path(&restored.info.source_id).unwrap()).unwrap(),
        V17
    );
    scan(&mut app);
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
}

#[test]
fn interrupted_explicit_removal_recovers_and_keeps_independent_support() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    fs::write(incoming.join("V17.txt"), V17).unwrap();
    fs::write(incoming.join("independent.txt"), INDEPENDENT).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let source = app
        .list_sources(0)
        .unwrap()
        .sources
        .into_iter()
        .find_map(|s| {
            let page = app.open_source(&s.source_id).unwrap();
            page.knowledge_pages
                .iter()
                .any(|p| p.title == "Observation V17")
                .then_some(page)
        })
        .unwrap();
    let event = source
        .knowledge_pages
        .iter()
        .find(|p| p.title == "Observation V17")
        .unwrap()
        .page_id
        .clone();
    let place = source
        .knowledge_pages
        .iter()
        .find(|p| p.title == "Riverside")
        .unwrap()
        .page_id
        .clone();
    app.set_publication_failpoint_for_test(1);
    assert!(app.remove_source(&source.info.source_id).is_err());
    drop(app);
    let mut app = open(&collection);
    assert!(app.open_source(&source.info.source_id).is_err());
    assert!(app.open_knowledge_page(&event).is_err());
    assert!(!app
        .search_pages(PageSearchRequest {
            query: "Observation V17".into(),
            ..Default::default()
        })
        .unwrap()
        .pages
        .iter()
        .any(|page| page.page_id == event));
    assert!(app
        .open_knowledge_page(&place)
        .unwrap()
        .markdown
        .contains("Source support count: 1"));
    scan(&mut app);
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
}

#[test]
fn overlapping_watches_do_not_resurrect_removed_source() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    let nested = incoming.join("nested");
    fs::create_dir_all(&nested).unwrap();
    let file = nested.join("V17.txt");
    fs::write(&file, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    app.watch_directory(&incoming).unwrap();
    app.watch_directory(&nested).unwrap();
    for _ in 0..10 {
        app.reconcile_watched_directories(128).unwrap();
    }
    let id = app.list_sources(0).unwrap().sources[0].source_id.clone();
    app.remove_source(&id).unwrap();
    fs::write(&file, V17.replace("Visits: 12", "Visits: 15")).unwrap();
    drop(app);
    let mut app = open(&collection);
    for _ in 0..10 {
        app.reconcile_watched_directories(128).unwrap();
    }
    assert!(app.list_sources(0).unwrap().sources.is_empty());
}

#[test]
fn interrupted_restore_finishes_on_restart() {
    let workspace = tempdir().unwrap();
    let file = workspace.path().join("source.txt");
    fs::write(&file, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    let source = app.import_source(&file, AcquisitionMethod::Picker).unwrap();
    app.remove_source(&source.info.source_id).unwrap();
    let backup = app.list_removed_sources().unwrap().remove(0);
    let backup_dir = collection
        .join(".recovery/sources")
        .join(&backup.recovery_id);
    fs::write(backup_dir.join("restore-pending"), "fixture-generation").unwrap();
    drop(app);
    let app = open(&collection);
    let restored = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(
        fs::read_to_string(app.original_path(&restored.info.source_id).unwrap()).unwrap(),
        V17
    );
    assert!(!collection
        .join(".recovery/sources")
        .join(&backup.recovery_id)
        .join("restore-pending")
        .exists());
}

#[test]
fn restore_keeps_changed_source_version_history_and_rebuilds_knowledge() {
    let workspace = tempdir().unwrap();
    let file = workspace.path().join("source.txt");
    fs::write(&file, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    let source = app.import_source(&file, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let revision = V17
        .replace("revision 1", "revision 2")
        .replace("Visits: 12", "Visits: 15");
    fs::write(&file, &revision).unwrap();
    app.import_source(&file, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let before = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(before.info.versions_seen.len(), 2);
    let versions: Vec<_> = before
        .info
        .versions_seen
        .iter()
        .map(|v| {
            (
                v.source_version_id.clone(),
                v.asset.clone(),
                fs::read(
                    app.original_version_path(
                        &source.info.source_id,
                        &v.source_version_id,
                        &v.asset,
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
        })
        .collect();
    app.remove_source(&source.info.source_id).unwrap();
    let backup = app.list_removed_sources().unwrap().remove(0);
    app.restore_removed_source(&backup.recovery_id).unwrap();
    let restored = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(restored.info.versions_seen.len(), 2);
    for (version_id, asset, bytes) in versions {
        assert_eq!(
            fs::read(
                app.original_version_path(&source.info.source_id, &version_id, &asset)
                    .unwrap()
            )
            .unwrap(),
            bytes
        );
    }
    app.resume_due_semantic_jobs().unwrap();
    let restored = app.open_source(&source.info.source_id).unwrap();
    let event = restored
        .knowledge_pages
        .iter()
        .find(|p| p.title == "Observation V17")
        .unwrap();
    assert!(app
        .open_knowledge_page(&event.page_id)
        .unwrap()
        .markdown
        .contains("15"));
}

#[test]
fn corrupted_recovery_copy_is_not_restored_or_left_pending() {
    let workspace = tempdir().unwrap();
    let file = workspace.path().join("source.txt");
    fs::write(&file, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    let source = app.import_source(&file, AcquisitionMethod::Picker).unwrap();
    app.remove_source(&source.info.source_id).unwrap();
    let backup = app.list_removed_sources().unwrap().remove(0);
    let directory = collection
        .join(".recovery/sources")
        .join(&backup.recovery_id);
    fs::write(directory.join("bundle/original.txt"), "damaged").unwrap();
    assert!(app.restore_removed_source(&backup.recovery_id).is_err());
    assert!(!directory.join("restore-pending").exists());
    drop(app);
    let app = open(&collection);
    assert!(app.list_sources(0).unwrap().sources.is_empty());
}

#[test]
fn removal_recovers_at_each_durable_checkpoint() {
    for checkpoint in ["backup", "intent", "generation", "suppression", "withdrawn"] {
        let workspace = tempdir().unwrap();
        let file = workspace.path().join("source.txt");
        fs::write(&file, V17).unwrap();
        let collection = workspace.path().join("collection");
        let mut app = open(&collection);
        let source = app.import_source(&file, AcquisitionMethod::Picker).unwrap();
        app.resume_due_semantic_jobs().unwrap();
        app.set_source_recovery_failpoint_for_test(checkpoint);
        assert!(
            app.remove_source(&source.info.source_id).is_err(),
            "{checkpoint}"
        );
        drop(app);
        let app = open(&collection);
        if checkpoint == "backup" {
            assert!(
                app.open_source(&source.info.source_id).is_ok(),
                "{checkpoint}"
            );
        } else {
            assert!(
                app.open_source(&source.info.source_id).is_err(),
                "{checkpoint}"
            );
            let backup = app.list_removed_sources().unwrap().remove(0);
            assert_eq!(
                fs::read_to_string(
                    collection
                        .join(".recovery/sources")
                        .join(backup.recovery_id)
                        .join("bundle/original.txt")
                )
                .unwrap(),
                V17
            );
        }
        assert_eq!(fs::read_to_string(&file).unwrap(), V17);
    }
}

#[test]
fn restore_recovers_at_each_durable_checkpoint() {
    for checkpoint in ["restore-intent", "restore-staged", "restore-published"] {
        let workspace = tempdir().unwrap();
        let file = workspace.path().join("source.txt");
        fs::write(&file, V17).unwrap();
        let collection = workspace.path().join("collection");
        let mut app = open(&collection);
        let source = app.import_source(&file, AcquisitionMethod::Picker).unwrap();
        app.remove_source(&source.info.source_id).unwrap();
        let backup = app.list_removed_sources().unwrap().remove(0);
        app.set_source_recovery_failpoint_for_test(checkpoint);
        assert!(
            app.restore_removed_source(&backup.recovery_id).is_err(),
            "{checkpoint}"
        );
        drop(app);
        let app = open(&collection);
        assert_eq!(
            fs::read_to_string(app.original_path(&source.info.source_id).unwrap()).unwrap(),
            V17,
            "{checkpoint}"
        );
        assert!(!collection
            .join(".recovery/sources")
            .join(backup.recovery_id)
            .join("restore-pending")
            .exists());
    }
}

#[test]
fn withdrawing_one_acquisition_requeues_inflight_semantic_work() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    let first = incoming.join("first.txt");
    let second = incoming.join("second.txt");
    fs::write(&first, V17).unwrap();
    fs::write(&second, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    let independent_path = workspace.path().join("independent.txt");
    fs::write(&independent_path, INDEPENDENT).unwrap();
    let independent = app
        .import_source(&independent_path, AcquisitionMethod::Picker)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let place = app
        .open_source(&independent.info.source_id)
        .unwrap()
        .knowledge_pages
        .iter()
        .find(|page| page.title == "Riverside")
        .unwrap()
        .page_id
        .clone();
    app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    let source_id = app
        .list_sources(0)
        .unwrap()
        .sources
        .into_iter()
        .find(|source| source.source_id != independent.info.source_id)
        .unwrap()
        .source_id;
    assert_eq!(
        app.open_source(&source_id).unwrap().info.acquisitions.len(),
        2
    );
    let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    fs::remove_file(&first).unwrap();
    scan(&mut app);
    scan(&mut app);
    assert_eq!(
        app.open_source(&source_id).unwrap().info.acquisitions.len(),
        1
    );
    let draft = FixtureProvider.form_knowledge(&job.source_text).unwrap();
    assert!(app.finish_semantic_job(job, Ok(draft)).is_err());
    let queued = app.open_source(&source_id).unwrap();
    assert_eq!(queued.info.semantic_state, "pending");
    assert_eq!(queued.info.versions_seen[0].processing_interruptions, 0);
    let retry = app.claim_due_semantic_jobs(1).unwrap().remove(0);
    let draft = FixtureProvider.form_knowledge(&retry.source_text).unwrap();
    app.finish_semantic_job(retry, Ok(draft)).unwrap();
    let source = app.open_source(&source_id).unwrap();
    assert_eq!(source.info.semantic_state, "complete");
    assert!(app
        .open_knowledge_page(&place)
        .unwrap()
        .markdown
        .contains("Source support count: 2"));
    assert!(source
        .knowledge_pages
        .iter()
        .any(|page| page.title == "Observation V17"));
    drop(app);
    let app = open(&collection);
    let source = app.open_source(&source_id).unwrap();
    assert!(app
        .open_knowledge_page(&place)
        .unwrap()
        .markdown
        .contains("Source support count: 2"));
    assert_eq!(source.info.acquisitions.len(), 1);
    assert!(source
        .knowledge_pages
        .iter()
        .any(|page| page.title == "Observation V17"));
}

#[test]
fn direct_readd_finishes_interrupted_withdrawal_before_new_work() {
    for restart in [false, true] {
        let workspace = tempdir().unwrap();
        let incoming = workspace.path().join("incoming");
        fs::create_dir(&incoming).unwrap();
        let file = incoming.join("source.txt");
        fs::write(&file, V17).unwrap();
        let collection = workspace.path().join("collection");
        let mut app = open(&collection);
        app.watch_directory(&incoming).unwrap();
        scan(&mut app);
        let source_id = app.list_sources(0).unwrap().sources[0].source_id.clone();
        let old = app.claim_due_semantic_jobs(1).unwrap().remove(0);
        fs::remove_file(&file).unwrap();
        scan(&mut app);
        app.set_source_recovery_failpoint_for_test("suppression");
        scan(&mut app);
        assert_eq!(
            app.list_watched_directories().unwrap()[0].state,
            "unavailable"
        );
        fs::write(&file, V17).unwrap();
        if restart {
            drop(app);
            app = open(&collection);
        }
        let added = app.import_source(&file, AcquisitionMethod::Picker).unwrap();
        assert_eq!(added.info.source_id, source_id);
        let new = app.claim_due_semantic_jobs(1).unwrap().remove(0);
        let old_draft = FixtureProvider.form_knowledge(&old.source_text).unwrap();
        assert!(app.finish_semantic_job(old, Ok(old_draft)).is_err());
        let draft = FixtureProvider.form_knowledge(&new.source_text).unwrap();
        app.finish_semantic_job(new, Ok(draft)).unwrap();
        drop(app);
        let app = open(&collection);
        let source = app.open_source(&source_id).unwrap();
        assert_eq!(source.info.semantic_state, "complete");
        assert!(source
            .knowledge_pages
            .iter()
            .any(|page| page.title == "Observation V17"));
    }
}

#[test]
fn withdrawn_origin_does_not_leave_its_unique_changed_version_as_evidence() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    let changed = incoming.join("changed.txt");
    let unchanged = incoming.join("unchanged.txt");
    fs::write(&changed, V17).unwrap();
    fs::write(&unchanged, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let id = app.list_sources(0).unwrap().sources[0].source_id.clone();
    let revision = V17
        .replace("revision 1", "revision 2")
        .replace("Visits: 12", "Visits: 15");
    fs::write(&changed, &revision).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&id).unwrap();
    let withdrawn_version = source.info.current_version_id.clone().unwrap();
    let withdrawn_asset = source
        .info
        .versions_seen
        .iter()
        .find(|v| v.source_version_id == withdrawn_version)
        .unwrap()
        .asset
        .clone();
    let event = source
        .knowledge_pages
        .iter()
        .find(|p| p.title == "Observation V17")
        .unwrap()
        .page_id
        .clone();
    assert!(app
        .open_knowledge_page(&event)
        .unwrap()
        .markdown
        .contains("**visits:** 15"));
    fs::remove_file(&changed).unwrap();
    scan(&mut app);
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let knowledge = app.open_knowledge_page(&event).unwrap().markdown;
    assert!(knowledge.contains("**visits:** 12"), "{knowledge}");
    assert!(!knowledge.contains("**visits:** 15"), "{knowledge}");
    assert!(app
        .original_version_path(&id, &withdrawn_version, &withdrawn_asset)
        .is_err());
    assert_eq!(fs::read_to_string(&unchanged).unwrap(), V17);
    let backup_root = collection.join(".recovery/sources");
    let backup = fs::read_dir(backup_root)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        fs::read_to_string(
            backup
                .join("bundle/versions")
                .join(&withdrawn_version)
                .join(&withdrawn_asset)
        )
        .unwrap(),
        revision
    );
    drop(app);
    let app = open(&collection);
    assert!(app
        .original_version_path(&id, &withdrawn_version, &withdrawn_asset)
        .is_err());
    assert!(app
        .open_knowledge_page(&event)
        .unwrap()
        .markdown
        .contains("**visits:** 12"));
}

#[test]
fn same_version_remains_supported_by_a_second_origin() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    let first = incoming.join("first.txt");
    let second = incoming.join("second.txt");
    fs::write(&first, V17).unwrap();
    fs::write(&second, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let id = app.list_sources(0).unwrap().sources[0].source_id.clone();
    let revision = V17
        .replace("revision 1", "revision 2")
        .replace("Visits: 12", "Visits: 15");
    fs::write(&first, &revision).unwrap();
    fs::write(&second, &revision).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&id).unwrap();
    let version = source.info.current_version_id.unwrap();
    let event = source
        .knowledge_pages
        .iter()
        .find(|p| p.title == "Observation V17")
        .unwrap()
        .page_id
        .clone();
    fs::remove_file(&first).unwrap();
    scan(&mut app);
    scan(&mut app);
    let source = app.open_source(&id).unwrap();
    assert_eq!(
        source.info.current_version_id.as_deref(),
        Some(version.as_str())
    );
    assert!(app
        .open_knowledge_page(&event)
        .unwrap()
        .markdown
        .contains("**visits:** 15"));
    drop(app);
    let app = open(&collection);
    assert!(app
        .open_knowledge_page(&event)
        .unwrap()
        .markdown
        .contains("**visits:** 15"));
}

#[test]
fn ordered_remaining_versions_restore_the_newest_supported_version() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    let first = incoming.join("first.txt");
    let second = incoming.join("second.txt");
    let third = incoming.join("third.txt");
    for path in [&first, &second, &third] {
        fs::write(path, V17).unwrap();
    }
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let id = app.list_sources(0).unwrap().sources[0].source_id.clone();
    let rev2 = V17
        .replace("revision 1", "revision 2")
        .replace("Visits: 12", "Visits: 15");
    fs::write(&second, &rev2).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let version2 = app
        .open_source(&id)
        .unwrap()
        .info
        .current_version_id
        .unwrap();
    let rev3 = rev2.replace("revision 2", "revision 3");
    fs::write(&first, &rev3).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    fs::remove_file(&first).unwrap();
    scan(&mut app);
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&id).unwrap();
    assert_eq!(
        source.info.current_version_id.as_deref(),
        Some(version2.as_str())
    );
    let event = source
        .knowledge_pages
        .iter()
        .find(|p| p.title == "Observation V17")
        .unwrap();
    assert!(app
        .open_knowledge_page(&event.page_id)
        .unwrap()
        .markdown
        .contains("**visits:** 15"));
    drop(app);
    let app = open(&collection);
    assert_eq!(
        app.open_source(&id)
            .unwrap()
            .info
            .current_version_id
            .as_deref(),
        Some(version2.as_str())
    );
}

#[test]
fn interrupted_fallback_recovers_before_search_serves_removed_version() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    let changed = incoming.join("changed.txt");
    let retained = incoming.join("retained.txt");
    fs::write(&changed, V17).unwrap();
    fs::write(&retained, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let id = app.list_sources(0).unwrap().sources[0].source_id.clone();
    fs::write(
        &changed,
        V17.replace("revision 1", "revision 2")
            .replace("Visits: 12", "Visits: 15"),
    )
    .unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let version = app
        .open_source(&id)
        .unwrap()
        .info
        .current_version_id
        .unwrap();
    fs::remove_file(&changed).unwrap();
    scan(&mut app);
    app.set_source_recovery_failpoint_for_test("fallback-original");
    scan(&mut app);
    assert_eq!(
        app.list_watched_directories().unwrap()[0].state,
        "unavailable"
    );
    drop(app);
    let mut app = open(&collection);
    assert!(app
        .original_version_path(&id, &version, "original.txt")
        .is_err());
    assert_eq!(
        fs::read_to_string(app.original_path(&id).unwrap()).unwrap(),
        V17
    );
    assert!(!app
        .search_pages(PageSearchRequest {
            query: "Visits: 15".into(),
            ..Default::default()
        })
        .unwrap()
        .pages
        .iter()
        .any(|page| page.source_id == id));
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&id).unwrap();
    let event = source
        .knowledge_pages
        .iter()
        .find(|p| p.title == "Observation V17")
        .unwrap();
    assert!(app
        .open_knowledge_page(&event.page_id)
        .unwrap()
        .markdown
        .contains("**visits:** 12"));
}

#[test]
fn legacy_version_ownership_remains_unknown_until_reacquired() {
    let workspace = tempdir().unwrap();
    let first = workspace.path().join("first.txt");
    let second = workspace.path().join("second.txt");
    fs::write(&first, V17).unwrap();
    fs::write(&second, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    let source = app
        .import_source(&first, AcquisitionMethod::Picker)
        .unwrap();
    app.import_source(&second, AcquisitionMethod::Picker)
        .unwrap();
    let page_path = app.page_path(&source.info.source_id).unwrap();
    drop(app);
    let markdown = fs::read_to_string(&page_path).unwrap();
    let (header, body) = markdown
        .strip_prefix("---\n")
        .unwrap()
        .split_once("\n---\n")
        .unwrap();
    let mut data: serde_yaml_ng::Value = serde_yaml_ng::from_str(header).unwrap();
    for version in data["versions_seen"].as_sequence_mut().unwrap() {
        version
            .as_mapping_mut()
            .unwrap()
            .remove(serde_yaml_ng::Value::String("acquisition_owners".into()));
        version
            .as_mapping_mut()
            .unwrap()
            .remove(serde_yaml_ng::Value::String("withdrawn".into()));
    }
    fs::write(
        &page_path,
        format!(
            "---\n{}---\n{}",
            serde_yaml_ng::to_string(&data).unwrap(),
            body
        ),
    )
    .unwrap();
    let mut app = open(&collection);
    assert!(app
        .open_source(&source.info.source_id)
        .unwrap()
        .info
        .versions_seen[0]
        .acquisition_owners
        .is_empty());
    assert_eq!(
        fs::read_to_string(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        V17
    );
    app.import_source(&first, AcquisitionMethod::Picker)
        .unwrap();
    let source = app.open_source(&source.info.source_id).unwrap();
    assert_eq!(source.info.versions_seen[0].acquisition_owners.len(), 1);
}

#[test]
fn reverting_an_origin_to_seen_older_bytes_retracts_unsupported_newer_version() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    let first = incoming.join("first.txt");
    let second = incoming.join("second.txt");
    fs::write(&first, V17).unwrap();
    fs::write(&second, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let id = app.list_sources(0).unwrap().sources[0].source_id.clone();
    fs::write(
        &first,
        V17.replace("revision 1", "revision 2")
            .replace("Visits: 12", "Visits: 15"),
    )
    .unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let version2 = app
        .open_source(&id)
        .unwrap()
        .info
        .current_version_id
        .unwrap();
    fs::write(&first, V17).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&id).unwrap();
    let event = source
        .knowledge_pages
        .iter()
        .find(|p| p.title == "Observation V17")
        .unwrap();
    let markdown = app.open_knowledge_page(&event.page_id).unwrap().markdown;
    assert!(markdown.contains("**visits:** 12"), "{markdown}");
    assert!(!markdown.contains("**visits:** 15"), "{markdown}");
    assert!(app
        .original_version_path(&id, &version2, "original.txt")
        .is_err());
    drop(app);
    let app = open(&collection);
    assert!(app
        .original_version_path(&id, &version2, "original.txt")
        .is_err());
}

#[test]
fn interrupted_reacquisition_reconciles_before_restart_search() {
    for checkpoint in ["intent", "generation", "fallback-original"] {
        let workspace = tempdir().unwrap();
        let incoming = workspace.path().join("incoming");
        fs::create_dir(&incoming).unwrap();
        let first = incoming.join("first.txt");
        let second = incoming.join("second.txt");
        fs::write(&first, V17).unwrap();
        fs::write(&second, V17).unwrap();
        let collection = workspace.path().join("collection");
        let mut app = open(&collection);
        app.watch_directory(&incoming).unwrap();
        scan(&mut app);
        app.resume_due_semantic_jobs().unwrap();
        let id = app.list_sources(0).unwrap().sources[0].source_id.clone();
        fs::write(
            &first,
            V17.replace("revision 1", "revision 2")
                .replace("Visits: 12", "Visits: 15"),
        )
        .unwrap();
        scan(&mut app);
        app.resume_due_semantic_jobs().unwrap();
        let version2 = app
            .open_source(&id)
            .unwrap()
            .info
            .current_version_id
            .unwrap();
        fs::write(&first, V17).unwrap();
        app.set_source_recovery_failpoint_for_test(checkpoint);
        let mut interrupted = false;
        for _ in 0..1000 {
            app.reconcile_watched_directories(1).unwrap();
            if app.list_watched_directories().unwrap()[0]
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("Injected source recovery interruption"))
            {
                interrupted = true;
                break;
            }
        }
        assert!(interrupted, "{checkpoint}");
        drop(app);
        let mut app = open(&collection);
        assert!(
            app.original_version_path(&id, &version2, "original.txt")
                .is_err(),
            "{checkpoint}"
        );
        assert_eq!(
            fs::read_to_string(app.original_path(&id).unwrap()).unwrap(),
            V17,
            "{checkpoint}"
        );
        app.resume_due_semantic_jobs().unwrap();
        let source = app.open_source(&id).unwrap();
        let event = source
            .knowledge_pages
            .iter()
            .find(|p| p.title == "Observation V17")
            .unwrap();
        assert!(app
            .open_knowledge_page(&event.page_id)
            .unwrap()
            .markdown
            .contains("**visits:** 12"));
    }
}

struct UnorderedProvider;
impl SemanticProvider for UnorderedProvider {
    fn form_knowledge(&self, text: &str) -> Result<KnowledgeDraft, ProviderError> {
        let mut draft = FixtureProvider.form_knowledge(text)?;
        if let Some(update) = draft.source_update.as_mut() {
            update.source_revision = None;
            update.source_revision_evidence = None;
            update.source_revision_certainty = 0.0;
        }
        Ok(draft)
    }
}

#[test]
fn unorderable_remaining_versions_are_reported_as_uncertain() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    let first = incoming.join("first.txt");
    let second = incoming.join("second.txt");
    let third = incoming.join("third.txt");
    for path in [&first, &second, &third] {
        fs::write(path, V17).unwrap();
    }
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(UnorderedProvider),
    )
    .unwrap();
    app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    let id = app.list_sources(0).unwrap().sources[0].source_id.clone();
    let rev2 = V17
        .replace("revision 1", "revision 2")
        .replace("Visits: 12", "Visits: 15");
    fs::write(&second, &rev2).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    fs::write(&first, rev2.replace("revision 2", "revision 3")).unwrap();
    scan(&mut app);
    app.resume_due_semantic_jobs().unwrap();
    fs::remove_file(&first).unwrap();
    scan(&mut app);
    scan(&mut app);
    let source = app.open_source(&id).unwrap();
    assert!(source.info.ordering_uncertain);
    assert!(source
        .info
        .semantic_error
        .as_deref()
        .unwrap()
        .contains("no reliable ordering"));
    assert!(source.info.current_version_id.is_none());
    assert!(source.info.knowledge_pages.is_empty());
}

#[test]
fn unregister_one_of_overlapping_watches_preserves_shared_source_and_original() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    let nested = incoming.join("nested");
    fs::create_dir_all(&nested).unwrap();
    let file = nested.join("source.txt");
    fs::write(&file, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    let outer = app.watch_directory(&incoming).unwrap();
    let inner = app.watch_directory(&nested).unwrap();
    for _ in 0..10 {
        app.reconcile_watched_directories(128).unwrap();
    }
    app.resume_due_semantic_jobs().unwrap();
    let id = app.list_sources(0).unwrap().sources[0].source_id.clone();
    assert_eq!(app.open_source(&id).unwrap().info.acquisitions.len(), 2);
    app.unregister_watched_directory(&outer.watch_id).unwrap();
    assert_eq!(app.list_watched_directories().unwrap().len(), 1);
    assert_eq!(app.open_source(&id).unwrap().info.acquisitions.len(), 1);
    assert_eq!(fs::read_to_string(&file).unwrap(), V17);
    drop(app);
    let mut app = open(&collection);
    for _ in 0..5 {
        app.reconcile_watched_directories(128).unwrap();
    }
    assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
    app.unregister_watched_directory(&inner.watch_id).unwrap();
    assert!(app.list_sources(0).unwrap().sources.is_empty());
    assert_eq!(fs::read_to_string(&file).unwrap(), V17);
    drop(app);
    let mut app = open(&collection);
    app.reconcile_watched_directories(128).unwrap();
    assert!(app.list_watched_directories().unwrap().is_empty());
    assert!(app.list_sources(0).unwrap().sources.is_empty());
}

#[test]
fn unregister_watch_preserves_picker_acquisition_at_same_path() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    let file = incoming.join("source.txt");
    fs::write(&file, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    let picked = app.import_source(&file, AcquisitionMethod::Picker).unwrap();
    let watch = app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    app.unregister_watched_directory(&watch.watch_id).unwrap();
    let retained = app.open_source(&picked.info.source_id).unwrap();
    assert_eq!(retained.info.acquisitions.len(), 1);
    assert_eq!(
        retained.info.acquisitions[0].method,
        AcquisitionMethod::Picker
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), V17);
    drop(app);
    let app = open(&collection);
    assert!(app.open_source(&picked.info.source_id).is_ok());
}

#[test]
fn unregister_overlapping_repositories_preserves_shared_source_and_original() {
    let workspace = tempdir().unwrap();
    let outer = workspace.path().join("repository");
    let inner = outer.join("nested");
    fs::create_dir_all(&inner).unwrap();
    let file = inner.join("source.txt");
    fs::write(&file, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    let first = app.import_repository(&outer).unwrap();
    let second = app.import_repository(&inner).unwrap();
    let first_id = &first.sources[0].source_id;
    let second_id = &second.sources[0].source_id;
    assert_ne!(first_id, second_id);
    app.resume_due_semantic_jobs().unwrap();
    let page = app
        .open_source(second_id)
        .unwrap()
        .knowledge_pages
        .into_iter()
        .find(|page| page.title == "Riverside")
        .unwrap();
    assert!(app
        .open_knowledge_page(&page.page_id)
        .unwrap()
        .markdown
        .contains("Source support count: 2"));
    app.unregister_repository(&first.repository_id).unwrap();
    assert_eq!(app.list_repositories().unwrap().len(), 1);
    assert!(app.open_source(first_id).is_err());
    assert!(app.open_source(second_id).is_ok());
    assert!(app
        .open_knowledge_page(&page.page_id)
        .unwrap()
        .markdown
        .contains("Source support count: 1"));
    assert_eq!(fs::read_to_string(&file).unwrap(), V17);
    drop(app);
    let mut app = open(&collection);
    assert!(app.open_source(second_id).is_ok());
    app.unregister_repository(&second.repository_id).unwrap();
    assert!(app.list_sources(0).unwrap().sources.is_empty());
    assert_eq!(fs::read_to_string(&file).unwrap(), V17);
    drop(app);
    let app = open(&collection);
    assert!(app.list_repositories().unwrap().is_empty());
    assert!(app.list_sources(0).unwrap().sources.is_empty());
}

#[test]
fn interrupted_repository_unregister_finishes_on_restart() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("repository");
    fs::create_dir(&repository).unwrap();
    let file = repository.join("source.txt");
    fs::write(&file, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    let imported = app.import_repository(&repository).unwrap();
    app.set_source_recovery_failpoint_for_test("intent");
    assert!(app.unregister_repository(&imported.repository_id).is_err());
    drop(app);
    let app = open(&collection);
    assert!(app.list_repositories().unwrap().is_empty());
    assert!(app.list_sources(0).unwrap().sources.is_empty());
    assert_eq!(fs::read_to_string(&file).unwrap(), V17);
}

#[test]
fn interrupted_watch_unregister_finishes_on_restart_without_resurrection() {
    let workspace = tempdir().unwrap();
    let incoming = workspace.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    let file = incoming.join("source.txt");
    fs::write(&file, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    let watch = app.watch_directory(&incoming).unwrap();
    scan(&mut app);
    app.set_source_recovery_failpoint_for_test("intent");
    assert!(app.unregister_watched_directory(&watch.watch_id).is_err());
    drop(app);
    let mut app = open(&collection);
    app.reconcile_watched_directories(128).unwrap();
    assert!(app.list_watched_directories().unwrap().is_empty());
    assert!(app.list_sources(0).unwrap().sources.is_empty());
    assert_eq!(fs::read_to_string(&file).unwrap(), V17);
}

#[test]
fn explicit_legacy_repository_can_be_unregistered_without_deleting_originals() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("repository");
    fs::create_dir(&repository).unwrap();
    let file = repository.join("source.txt");
    fs::write(&file, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    let imported = app.import_repository(&repository).unwrap();
    let page_path = app.page_path(&imported.sources[0].source_id).unwrap();
    drop(app);
    fs::remove_file(
        collection
            .join("repositories")
            .join(format!("{}.json", imported.repository_id)),
    )
    .unwrap();
    let markdown = fs::read_to_string(&page_path).unwrap();
    let (header, body) = markdown
        .strip_prefix("---\n")
        .unwrap()
        .split_once("\n---\n")
        .unwrap();
    let mut data: serde_yaml_ng::Value = serde_yaml_ng::from_str(header).unwrap();
    for acquisition in data["acquisitions"].as_sequence_mut().unwrap() {
        acquisition
            .as_mapping_mut()
            .unwrap()
            .remove(serde_yaml_ng::Value::String("registration_id".into()));
    }
    for version in data["versions_seen"].as_sequence_mut().unwrap() {
        version
            .as_mapping_mut()
            .unwrap()
            .remove(serde_yaml_ng::Value::String("acquisition_owners".into()));
    }
    fs::write(
        &page_path,
        format!(
            "---\n{}---\n{}",
            serde_yaml_ng::to_string(&data).unwrap(),
            body
        ),
    )
    .unwrap();
    let mut app = open(&collection);
    let listed = app.list_repositories().unwrap();
    assert_eq!(listed[0].state, "legacy_explicit");
    app.unregister_repository(&imported.repository_id).unwrap();
    assert!(app.list_sources(0).unwrap().sources.is_empty());
    assert_eq!(fs::read_to_string(&file).unwrap(), V17);
    drop(app);
    let app = open(&collection);
    assert!(app.list_repositories().unwrap().is_empty());
}

#[test]
fn ambiguous_legacy_repository_is_visible_and_cannot_be_unregistered() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("repository");
    fs::create_dir(&repository).unwrap();
    let file = repository.join("source.txt");
    fs::write(&file, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    let imported = app.import_repository(&repository).unwrap();
    let page_path = app.page_path(&imported.sources[0].source_id).unwrap();
    drop(app);
    fs::remove_file(
        collection
            .join("repositories")
            .join(format!("{}.json", imported.repository_id)),
    )
    .unwrap();
    let markdown = fs::read_to_string(&page_path).unwrap();
    let (header, body) = markdown
        .strip_prefix("---\n")
        .unwrap()
        .split_once("\n---\n")
        .unwrap();
    let mut data: serde_yaml_ng::Value = serde_yaml_ng::from_str(header).unwrap();
    let acquisition = &mut data["acquisitions"][0];
    acquisition
        .as_mapping_mut()
        .unwrap()
        .remove(serde_yaml_ng::Value::String("registration_id".into()));
    acquisition["path"] = serde_yaml_ng::Value::String(
        workspace
            .path()
            .join("other.txt")
            .to_string_lossy()
            .into_owned(),
    );
    fs::write(
        &page_path,
        format!(
            "---\n{}---\n{}",
            serde_yaml_ng::to_string(&data).unwrap(),
            body
        ),
    )
    .unwrap();
    let mut app = open(&collection);
    assert_eq!(
        app.list_repositories().unwrap()[0].state,
        "legacy_ambiguous"
    );
    assert!(app.unregister_repository(&imported.repository_id).is_err());
    assert!(app.open_source(&imported.sources[0].source_id).is_ok());
    assert_eq!(fs::read_to_string(&file).unwrap(), V17);
}

#[test]
fn unregister_repository_preserves_picker_acquisition_of_same_file() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("repository");
    fs::create_dir(&repository).unwrap();
    let file = repository.join("source.txt");
    fs::write(&file, V17).unwrap();
    let collection = workspace.path().join("collection");
    let mut app = open(&collection);
    let imported = app.import_repository(&repository).unwrap();
    let picked = app.import_source(&file, AcquisitionMethod::Picker).unwrap();
    assert_eq!(picked.info.source_id, imported.sources[0].source_id);
    app.unregister_repository(&imported.repository_id).unwrap();
    let source = app.open_source(&picked.info.source_id).unwrap();
    assert_eq!(source.info.acquisitions.len(), 1);
    assert_eq!(
        source.info.acquisitions[0].method,
        AcquisitionMethod::Picker
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), V17);
    drop(app);
    let app = open(&collection);
    assert!(app.open_source(&picked.info.source_id).is_ok());
}
