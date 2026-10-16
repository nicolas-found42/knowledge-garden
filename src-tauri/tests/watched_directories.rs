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
        assert_eq!(
            app.retained_source_exists(&initial.info.source_id).unwrap(),
            independent
        );
        if independent {
            let retained = app.open_source(&initial.info.source_id).unwrap();
            assert_eq!(retained.info.acquisitions.len(), 1);
            assert!(retained
                .info
                .acquisitions
                .iter()
                .all(|acquisition| acquisition.path != path.to_string_lossy()));
            assert!(app.original_path(&initial.info.source_id).unwrap().exists());
        }
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
        assert!(!app.retained_source_exists(&second_id).unwrap());
        assert!(app.retained_source_exists(&first_id).unwrap());
        drop(app);
        let app = open(&collection);
        assert_eq!(app.list_sources(0).unwrap().sources.len(), 1);
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
        assert_eq!(app.retained_source_exists(&ids[1]).unwrap(), ambiguous);
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
            assert!(!app.retained_source_exists(&ids[1]).unwrap());
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
    struct Mounted(PathBuf);
    impl Drop for Mounted {
        fn drop(&mut self) {
            let _ = Command::new("/usr/bin/hdiutil")
                .arg("detach")
                .arg(&self.0)
                .output();
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
            panic!("hdiutil {args:?}: {error}");
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
    };
    attach();
    let _guard = Mounted(mounted.clone());
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
    hdiutil(&["detach"], &mounted);
    fs::create_dir_all(&mounted).unwrap();
    scan(&mut app);
    scan(&mut app);
    assert_eq!(
        app.list_watched_directories().unwrap()[0].state,
        "unavailable"
    );
    assert!(original.exists());
    attach();
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
