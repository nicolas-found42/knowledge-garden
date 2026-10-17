use knowledge_garden::{
    application::Application,
    providers::{JevSemanticProvider, SystemOneTransport},
    semantic::ProviderError,
};
use serde_json::{json, Value};
use std::fs;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tempfile::tempdir;

struct RepositoryConceptTransport(AtomicUsize);

impl SystemOneTransport for RepositoryConceptTransport {
    fn complete(
        &self,
        _api_key: &str,
        request: &Value,
    ) -> std::result::Result<Value, ProviderError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        let source_text = request["state"]["source_text"].as_str().unwrap_or_default();
        if !source_text.contains("pub struct Cache") {
            return Err(ProviderError::permanent(
                "The repository fixture's code was not sent as source text.".into(),
            ));
        }
        let questions = request["questions"]
            .as_object()
            .ok_or_else(|| ProviderError::permanent("Missing typed questions.".into()))?;
        let mut answers = serde_json::Map::new();
        for (key, question) in questions {
            match question["type"].as_str().unwrap_or_default() {
                "noul" => {
                    answers.insert(key.clone(), json!({"type":"noul","noul":0.99}));
                }
                "choice" => {
                    let criteria = question["criteria"].as_object().ok_or_else(|| {
                        ProviderError::permanent(format!("Missing criteria for {key}."))
                    })?;
                    let selected = match key.as_str() {
                        "source_update_role" => "supplement",
                        "source_update_evidence" => criteria
                            .keys()
                            .find(|candidate| candidate.starts_with("span_"))
                            .map(String::as_str)
                            .unwrap_or("span_0"),
                        "source_order_date" | "source_order_revision" => "none",
                        "code_concept_0" => "concept",
                        name if name.contains("concept") && criteria.contains_key("concept") => {
                            "concept"
                        }
                        _ => criteria.keys().next().map(String::as_str).unwrap_or("none"),
                    };
                    if !criteria.contains_key(selected) {
                        return Err(ProviderError::permanent(format!(
                            "Unexpected fixed fixture choice `{selected}` for {key}."
                        )));
                    }
                    answers.insert(
                        key.clone(),
                        json!({"type":"choice","choice":selected,"probabilities":{selected:0.99}}),
                    );
                }
                other => {
                    return Err(ProviderError::permanent(format!(
                        "Unexpected typed question `{other}` for {key}."
                    )))
                }
            }
        }
        Ok(json!({"model":"typesafe/jev-1.13-repository-fixture","answers":answers}))
    }
}

#[test]
fn owner_supplied_repository_enters_the_common_source_pipeline() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("sample-repository");
    fs::create_dir_all(repository.join("docs")).unwrap();
    fs::write(
        repository.join("README.md"),
        "# Garden Sample\n\nCache stays local.\n\n[External guide](https://example.invalid/unacquired/guide.md)\n",
    )
    .unwrap();
    fs::write(
        repository.join("docs/ADR-0001.md"),
        "# Decision: Keep cache local\n",
    )
    .unwrap();
    fs::create_dir_all(repository.join("src")).unwrap();
    fs::write(
        repository.join("src/cache.rs"),
        "// Cache implementation\npub struct Cache;\n",
    )
    .unwrap();

    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    let imported = app.import_repository(&repository).unwrap();

    assert_eq!(imported.sources.len(), 3);
    let readme = imported
        .sources
        .iter()
        .find(|source| source.relative_path == "README.md")
        .unwrap();
    let page = app.open_source(&readme.source_id).unwrap();
    assert_eq!(
        page.info.repository.as_ref().unwrap().relative_path,
        "README.md"
    );
    assert_eq!(
        fs::read(app.original_path(&readme.source_id).unwrap()).unwrap(),
        b"# Garden Sample\n\nCache stays local.\n\n[External guide](https://example.invalid/unacquired/guide.md)\n"
    );
    assert!(page
        .body
        .contains("https://example.invalid/unacquired/guide.md"));
    assert_eq!(
        imported.sources.len(),
        3,
        "the linked remote target was not acquired"
    );
    let code = imported
        .sources
        .iter()
        .find(|source| source.relative_path == "src/cache.rs")
        .unwrap();
    let code_page = app.open_source(&code.source_id).unwrap();
    assert_eq!(
        code_page.info.extraction,
        knowledge_garden::application::ExtractionState::TextPreserved
    );
    assert_eq!(code_page.info.line_count, 2);
    assert!(code_page.body.contains("pub struct Cache;"));
}

#[cfg(unix)]
#[test]
fn repository_symlinks_are_reported_as_visible_coverage_exclusions() {
    use std::os::unix::fs::symlink;

    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("sample-repository");
    fs::create_dir_all(&repository).unwrap();
    fs::write(repository.join("README.md"), "# Garden Sample\n").unwrap();
    let outside = workspace.path().join("outside.md");
    fs::write(&outside, "Must not be followed into the repository.\n").unwrap();
    symlink(&outside, repository.join("linked.md")).unwrap();

    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    let imported = app.import_repository(&repository).unwrap();

    let serialized = serde_json::to_value(&imported).unwrap();
    let exclusions = serialized
        .get("exclusions")
        .and_then(serde_json::Value::as_array)
        .expect("repository import must expose excluded-member coverage");
    assert!(exclusions.iter().any(|exclusion| {
        exclusion
            .get("relative_path")
            .and_then(serde_json::Value::as_str)
            == Some("linked.md")
    }));
    assert_eq!(imported.sources.len(), 1);
    assert!(imported
        .sources
        .iter()
        .all(|source| source.relative_path != "linked.md"));
    assert_eq!(
        fs::read_to_string(outside).unwrap(),
        "Must not be followed into the repository.\n"
    );
}

#[cfg(unix)]
#[test]
fn repository_special_files_are_reported_as_visible_coverage_exclusions() {
    use std::os::unix::net::UnixListener;

    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("sample-repository");
    fs::create_dir_all(&repository).unwrap();
    fs::write(repository.join("README.md"), "# Garden Sample\n").unwrap();
    let socket_path = repository.join("local.sock");
    let _listener = UnixListener::bind(&socket_path).unwrap();

    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    let imported = app.import_repository(&repository).unwrap();

    assert_eq!(imported.sources.len(), 1);
    assert_eq!(imported.sources[0].relative_path, "README.md");
    assert!(imported.exclusions.iter().any(|exclusion| {
        exclusion.relative_path == "local.sock" && exclusion.reason == "special_files_not_imported"
    }));
}

#[cfg(unix)]
#[test]
fn repository_unreadable_files_are_visible_without_aborting_readable_members() {
    use std::os::unix::fs::PermissionsExt;

    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("sample-repository");
    fs::create_dir_all(&repository).unwrap();
    fs::write(repository.join("README.md"), "# Readable member\n").unwrap();
    let unreadable = repository.join("private.txt");
    fs::write(
        &unreadable,
        "This member cannot be read by the importing process.\n",
    )
    .unwrap();
    let mut permissions = fs::metadata(&unreadable).unwrap().permissions();
    permissions.set_mode(0o0);
    fs::set_permissions(&unreadable, permissions).unwrap();
    assert!(fs::File::open(&unreadable).is_err());

    let unreadable_directory = repository.join("private-dir");
    fs::create_dir(&unreadable_directory).unwrap();
    fs::write(unreadable_directory.join("secret.txt"), "Private member.\n").unwrap();
    let mut directory_permissions = fs::metadata(&unreadable_directory).unwrap().permissions();
    directory_permissions.set_mode(0o0);
    fs::set_permissions(&unreadable_directory, directory_permissions).unwrap();
    assert!(fs::read_dir(&unreadable_directory).is_err());

    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    let imported_result = app.import_repository(&repository);
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(&unreadable_directory, fs::Permissions::from_mode(0o700)).unwrap();
    let imported = imported_result.unwrap();

    assert!(imported
        .sources
        .iter()
        .any(|source| source.relative_path == "README.md"));
    assert!(!imported
        .sources
        .iter()
        .any(|source| source.relative_path == "private.txt"));
    assert!(imported.exclusions.iter().any(|exclusion| {
        exclusion.relative_path == "private.txt" && exclusion.reason == "unreadable_member"
    }));
    assert!(imported.exclusions.iter().any(|exclusion| {
        exclusion.relative_path == "private-dir" && exclusion.reason == "unreadable_member"
    }));
}

#[test]
fn repository_metadata_directory_is_reported_without_importing_history_files() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("sample-repository");
    fs::create_dir_all(repository.join(".git/objects")).unwrap();
    fs::write(repository.join("README.md"), "# Garden Sample\n").unwrap();
    fs::write(
        repository.join(".git/config"),
        "[core]\nrepositoryformatversion = 0\n",
    )
    .unwrap();

    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    let imported = app.import_repository(&repository).unwrap();
    let serialized = serde_json::to_value(&imported).unwrap();
    let exclusions = serialized
        .get("exclusions")
        .and_then(serde_json::Value::as_array)
        .expect("repository import must expose excluded-member coverage");

    assert!(exclusions.iter().any(|exclusion| {
        exclusion
            .get("relative_path")
            .and_then(serde_json::Value::as_str)
            == Some(".git")
            && exclusion.get("reason").and_then(serde_json::Value::as_str)
                == Some("repository_metadata_not_imported")
    }));
    assert_eq!(imported.sources.len(), 1);
    assert!(imported
        .sources
        .iter()
        .all(|source| !source.relative_path.starts_with(".git/")));
}

#[test]
fn repository_json_code_is_retained_as_code_not_misrouted_to_conversation_adapter() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("sample-repository");
    fs::create_dir_all(&repository).unwrap();
    let package = br#"{"name":"sample-garden","scripts":{"postinstall":"touch SHOULD_NOT_RUN"}}"#;
    fs::write(repository.join("package.json"), package).unwrap();

    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    let imported = app.import_repository(&repository).unwrap();
    let package_source = imported
        .sources
        .iter()
        .find(|source| source.relative_path == "package.json")
        .unwrap();
    let page = app.open_source(&package_source.source_id).unwrap();

    assert_eq!(
        page.info.extraction,
        knowledge_garden::application::ExtractionState::TextPreserved
    );
    assert!(page.body.contains("postinstall"));
    assert_eq!(
        fs::read(app.original_path(&package_source.source_id).unwrap()).unwrap(),
        package
    );
}

#[test]
fn repository_identity_file_source_and_content_versions_stay_distinct_across_reimport() {
    let workspace = tempdir().unwrap();
    let first_repository = workspace.path().join("first-repository");
    let second_repository = workspace.path().join("second-repository");
    fs::create_dir_all(first_repository.join("src")).unwrap();
    fs::create_dir_all(second_repository.join("src")).unwrap();
    let relative_path = "src/cache.rs";
    let initial = "/// Cache parsed plans locally.\npub struct Cache;\n";
    fs::write(first_repository.join(relative_path), initial).unwrap();
    fs::write(second_repository.join(relative_path), initial).unwrap();

    let provider = Arc::new(JevSemanticProvider::with_transport(
        "repository-fixture-credential".into(),
        Arc::new(RepositoryConceptTransport(AtomicUsize::new(0))),
    ));
    let collection = workspace.path().join("collection");
    let mut app = Application::open_with_semantic_provider(&collection, provider).unwrap();
    let first_import = app.import_repository(&first_repository).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let first_file = first_import
        .sources
        .iter()
        .find(|source| source.relative_path == relative_path)
        .unwrap();
    let first_page = app.open_source(&first_file.source_id).unwrap();
    let first_version = first_page.info.current_version_id.clone().unwrap();
    assert_eq!(
        first_page.info.repository.as_ref().unwrap().relative_path,
        relative_path
    );

    let repeated = app.import_repository(&first_repository).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let repeated_file = repeated
        .sources
        .iter()
        .find(|source| source.relative_path == relative_path)
        .unwrap();
    let repeated_page = app.open_source(&repeated_file.source_id).unwrap();
    assert_eq!(repeated.repository_id, first_import.repository_id);
    assert_eq!(repeated_file.source_id, first_file.source_id);
    assert_eq!(
        repeated_page.info.current_version_id.as_deref(),
        Some(first_version.as_str())
    );
    assert_eq!(repeated_page.info.versions_seen.len(), 1);

    let second_import = app.import_repository(&second_repository).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let second_file = second_import
        .sources
        .iter()
        .find(|source| source.relative_path == relative_path)
        .unwrap();
    let second_page = app.open_source(&second_file.source_id).unwrap();
    assert_ne!(second_import.repository_id, first_import.repository_id);
    assert_ne!(second_file.source_id, first_file.source_id);
    assert_eq!(
        second_page.info.repository.as_ref().unwrap().relative_path,
        relative_path
    );
    assert_eq!(
        second_page.info.current_version_id.as_deref(),
        Some(first_version.as_str())
    );

    let revised = "/// Cache parsed plans locally after validating each key.\npub struct Cache;\n";
    fs::write(first_repository.join(relative_path), revised).unwrap();
    let revised_import = app.import_repository(&first_repository).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let revised_file = revised_import
        .sources
        .iter()
        .find(|source| source.relative_path == relative_path)
        .unwrap();
    let revised_page = app.open_source(&revised_file.source_id).unwrap();
    assert_eq!(revised_import.repository_id, first_import.repository_id);
    assert_eq!(revised_file.source_id, first_file.source_id);
    assert_eq!(revised_page.info.versions_seen.len(), 2);
    let revised_version = revised_page
        .info
        .versions_seen
        .iter()
        .find(|version| version.source_version_id != first_version)
        .expect("changed file bytes should create a distinct retained source version");
    assert_ne!(revised_version.source_version_id, first_version);
    assert_eq!(
        fs::read(
            app.original_version_path(
                &revised_file.source_id,
                &first_version,
                &revised_page
                    .info
                    .versions_seen
                    .iter()
                    .find(|version| version.source_version_id == first_version)
                    .unwrap()
                    .asset,
            )
            .unwrap()
        )
        .unwrap(),
        initial.as_bytes()
    );
    assert_eq!(
        fs::read(
            app.original_version_path(
                &revised_file.source_id,
                &revised_version.source_version_id,
                &revised_version.asset,
            )
            .unwrap()
        )
        .unwrap(),
        revised.as_bytes()
    );
    drop(app);
    let reopened = Application::open(&collection).unwrap();
    let restored = reopened.open_source(&first_file.source_id).unwrap();
    assert_eq!(restored.info.repository, revised_page.info.repository);
    assert_eq!(
        restored.info.current_version_id.as_deref(),
        Some(first_version.as_str())
    );
    assert!(revised_page.info.pending_version_id.is_none());
    assert!(restored
        .info
        .versions_seen
        .iter()
        .any(|version| version.source_version_id == revised_version.source_version_id));
    assert_eq!(
        restored.info.current_version_id,
        revised_page.info.current_version_id
    );
    assert_eq!(
        restored.info.pending_version_id,
        revised_page.info.pending_version_id
    );
    assert_eq!(restored.info.versions_seen.len(), 2);
}

#[test]
fn repository_binary_oversized_and_generated_members_remain_visible_with_originals() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("coverage-repository");
    fs::create_dir_all(repository.join("dist")).unwrap();
    fs::create_dir_all(repository.join("assets")).unwrap();
    fs::create_dir_all(repository.join("src")).unwrap();
    let binary = [0x00, 0xff, 0x10, 0x80];
    fs::write(repository.join("assets/model.bin"), binary).unwrap();
    let unreadable_utf8 = b"pub struct Broken;\n\xff";
    fs::write(repository.join("src/broken.rs"), unreadable_utf8).unwrap();
    let large = format!("// generated bundle\n{}", "x".repeat(2 * 1024 * 1024));
    fs::write(repository.join("dist/client.min.js"), large.as_bytes()).unwrap();
    let generated = "// generated fixture, still supplied source data\nconst value = 7;\n";
    fs::write(repository.join("dist/runtime.js"), generated).unwrap();

    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    let imported = app.import_repository(&repository).unwrap();
    let find_source = |relative_path: &str| {
        imported
            .sources
            .iter()
            .find(|source| source.relative_path == relative_path)
            .unwrap()
    };

    let binary_source = find_source("assets/model.bin");
    let binary_page = app.open_source(&binary_source.source_id).unwrap();
    assert_eq!(
        binary_page.info.extraction,
        knowledge_garden::application::ExtractionState::Unsupported
    );
    assert_eq!(
        fs::read(app.original_path(&binary_source.source_id).unwrap()).unwrap(),
        binary
    );

    let unreadable_source = find_source("src/broken.rs");
    let unreadable_page = app.open_source(&unreadable_source.source_id).unwrap();
    assert_eq!(
        unreadable_page.info.extraction,
        knowledge_garden::application::ExtractionState::InvalidUtf8
    );
    assert_eq!(
        fs::read(app.original_path(&unreadable_source.source_id).unwrap()).unwrap(),
        unreadable_utf8
    );

    let large_source = find_source("dist/client.min.js");
    let large_page = app.open_source(&large_source.source_id).unwrap();
    assert_eq!(
        large_page.info.extraction,
        knowledge_garden::application::ExtractionState::TooLarge
    );
    assert_eq!(
        fs::read(app.original_path(&large_source.source_id).unwrap()).unwrap(),
        large.as_bytes()
    );

    let generated_source = find_source("dist/runtime.js");
    let generated_page = app.open_source(&generated_source.source_id).unwrap();
    assert!(generated_page.body.contains("generated fixture"));
    assert_eq!(
        fs::read(app.original_path(&generated_source.source_id).unwrap()).unwrap(),
        generated.as_bytes()
    );
}

#[cfg(unix)]
#[test]
fn repository_scripts_are_retained_as_text_without_execution() {
    use std::os::unix::fs::PermissionsExt;

    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("sample-repository");
    fs::create_dir_all(&repository).unwrap();
    fs::write(repository.join("README.md"), "# Garden Sample\n").unwrap();
    let marker = workspace.path().join("script-was-executed");
    let script = repository.join("postinstall.sh");
    fs::write(&script, format!("#!/bin/sh\ntouch {}\n", marker.display())).unwrap();
    let mut permissions = fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).unwrap();

    let collection = workspace.path().join("collection");
    let mut app = Application::open(&collection).unwrap();
    let imported = app.import_repository(&repository).unwrap();
    let script_source = imported
        .sources
        .iter()
        .find(|source| source.relative_path == "postinstall.sh")
        .unwrap();
    let page = app.open_source(&script_source.source_id).unwrap();

    assert_eq!(
        page.info.extraction,
        knowledge_garden::application::ExtractionState::TextPreserved
    );
    assert!(page.body.contains("touch"));
    assert!(
        !marker.exists(),
        "repository scripts are source data, not commands"
    );
}

#[test]
fn repository_code_description_preserves_all_adjacent_doc_lines_and_qualifications() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("repository");
    fs::create_dir_all(&repository).unwrap();
    let code = "\u{feff}/// Reuses parsed plans.\r\n/// Only temporary test processes use this cache.\r\n///     let cache = Cache;\r\npub struct Cache;\r\n";
    fs::write(repository.join("cache.rs"), code).unwrap();
    let provider = Arc::new(JevSemanticProvider::with_transport(
        "fixture".into(),
        Arc::new(RepositoryConceptTransport(AtomicUsize::new(0))),
    ));
    let collection = workspace.path().join("collection");
    let mut app = Application::open_with_semantic_provider(&collection, provider).unwrap();
    let imported = app.import_repository(&repository).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&imported.sources[0].source_id).unwrap();
    let concept = source
        .knowledge_pages
        .iter()
        .find(|page| page.title == "Cache")
        .unwrap();
    let page = app.open_knowledge_page(&concept.page_id).unwrap();
    let header: serde_yaml_ng::Value = serde_yaml_ng::from_str(
        page.markdown
            .strip_prefix("---\n")
            .unwrap()
            .split_once("\n---\n")
            .unwrap()
            .0,
    )
    .unwrap();
    let description = header["facts"]
        .as_sequence()
        .unwrap()
        .iter()
        .find(|fact| fact["property"] == "description")
        .unwrap();
    assert_eq!(description["value"].as_str().unwrap(), "Reuses parsed plans.\nOnly temporary test processes use this cache.\n    let cache = Cache;");
    let normalized = code;
    let start = description["evidence"]["byte_start"].as_u64().unwrap() as usize;
    let end = description["evidence"]["byte_end"].as_u64().unwrap() as usize;
    assert_eq!(
        &normalized[start..end],
        description["evidence"]["quote"].as_str().unwrap()
    );
    assert!(
        page.markdown.contains("Reuses parsed plans."),
        "{}",
        page.markdown
    );
    assert!(page
        .markdown
        .contains("Only temporary test processes use this cache."));
    assert!(
        page.markdown.contains("source lines 1–3"),
        "{}",
        page.markdown
    );
    assert_eq!(
        fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        code.as_bytes()
    );
    let concept_id = concept.page_id.clone();
    drop(app);
    let mut reopened = Application::open(&collection).unwrap();
    let results = reopened
        .search_pages(knowledge_garden::application::PageSearchRequest {
            query: "temporary".into(),
            ..Default::default()
        })
        .unwrap();
    let location = results
        .pages
        .iter()
        .find(|result| result.page_id == concept_id)
        .unwrap()
        .match_location
        .as_ref()
        .unwrap();
    assert_eq!(
        location.source_version_id.as_deref(),
        Some(
            source
                .info
                .current_version_id
                .as_deref()
                .expect("source revision must exist")
        )
    );
    assert_eq!(location.source_location.as_deref(), Some("cache.rs"));
    assert_eq!(location.line_start, 1);
    assert_eq!(location.line_end, 3);
    assert_eq!(
        reopened.open_knowledge_page(&concept_id).unwrap().markdown,
        page.markdown
    );
}

#[test]
fn repository_code_concept_is_navigable_with_exact_file_and_line_evidence() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("sample-repository");
    fs::create_dir_all(repository.join("src")).unwrap();
    let code = "/// A process-local cache reuses parsed query plans.\npub struct Cache {\n    entries: Vec<String>,\n}\n";
    fs::write(repository.join("src/cache.rs"), code).unwrap();
    let transport = Arc::new(RepositoryConceptTransport(AtomicUsize::new(0)));
    let provider = Arc::new(JevSemanticProvider::with_transport(
        "repository-fixture-credential".into(),
        transport.clone(),
    ));
    let collection = workspace.path().join("collection");
    let mut app = Application::open_with_semantic_provider(&collection, provider).unwrap();

    let imported = app.import_repository(&repository).unwrap();
    assert_eq!(imported.sources.len(), 1);
    app.resume_due_semantic_jobs().unwrap();

    let source = app.open_source(&imported.sources[0].source_id).unwrap();
    assert_eq!(source.info.semantic_state, "complete", "{source:?}");
    assert_eq!(
        source.info.repository.as_ref().unwrap().relative_path,
        "src/cache.rs"
    );
    assert!(source.info.current_version_id.is_some());
    assert_eq!(transport.0.load(Ordering::SeqCst), 1);
    let concept = source
        .info
        .knowledge_pages
        .iter()
        .find(|page| page.kind == "concept" && page.title == "Cache")
        .expect("the documented code symbol must become a concept page");
    let page = app.open_knowledge_page(&concept.page_id).unwrap();
    let version_id = source.info.current_version_id.as_ref().unwrap();
    assert!(
        page.markdown.contains("reuses parsed query plans"),
        "{}",
        page.markdown
    );
    assert!(
        page.markdown.contains("source lines 1–1"),
        "{}",
        page.markdown
    );
    assert!(page.markdown.contains("src/cache.rs"), "{}", page.markdown);
    assert!(
        page.markdown.contains(&source.info.source_id),
        "{}",
        page.markdown
    );
    assert!(page.markdown.contains(version_id), "{}", page.markdown);
    assert!(page.markdown.contains("Source page"), "{}", page.markdown);
    assert!(
        page.markdown.contains("Retained original"),
        "{}",
        page.markdown
    );
    let original = app.original_path(&source.info.source_id).unwrap();
    assert_eq!(fs::read(original).unwrap(), code.as_bytes());
}

#[test]
fn repeated_repository_symbol_names_in_different_files_keep_separate_concepts() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("sample-repository");
    fs::create_dir_all(repository.join("src/query")).unwrap();
    fs::create_dir_all(repository.join("src/http")).unwrap();
    let query_source = "/// A process-local cache reuses parsed query plans.\npub struct Cache { entries: Vec<String> }\n";
    let http_source = "/// A response cache stores HTTP payloads by request key.\npub struct Cache { entries: Vec<String> }\n";
    fs::write(repository.join("src/query/cache.rs"), query_source).unwrap();
    fs::write(repository.join("src/http/cache.rs"), http_source).unwrap();
    let transport = Arc::new(RepositoryConceptTransport(AtomicUsize::new(0)));
    let provider = Arc::new(JevSemanticProvider::with_transport(
        "repository-fixture-credential".into(),
        transport,
    ));
    let collection = workspace.path().join("collection");
    let mut app = Application::open_with_semantic_provider(&collection, provider).unwrap();

    let imported = app.import_repository(&repository).unwrap();
    assert_eq!(imported.sources.len(), 2);
    app.resume_due_semantic_jobs().unwrap();

    let mut concepts = Vec::new();
    for relative_path in ["src/query/cache.rs", "src/http/cache.rs"] {
        let source = imported
            .sources
            .iter()
            .find(|source| source.relative_path == relative_path)
            .unwrap();
        let projection = app.open_source(&source.source_id).unwrap();
        assert_eq!(projection.info.semantic_state, "complete", "{projection:?}");
        let concept = projection
            .info
            .knowledge_pages
            .iter()
            .find(|page| page.kind == "concept" && page.title == "Cache")
            .expect("each documented symbol should have a navigable concept");
        let page = app.open_knowledge_page(&concept.page_id).unwrap();
        assert!(page.markdown.contains(relative_path), "{}", page.markdown);
        concepts.push((concept.page_id.clone(), page.markdown));
    }

    assert_ne!(concepts[0].0, concepts[1].0);
    assert!(concepts[0].1.contains("parsed query plans"));
    assert!(!concepts[0].1.contains("HTTP payloads"));
    assert!(concepts[1].1.contains("HTTP payloads"));
    assert!(!concepts[1].1.contains("parsed query plans"));
}

struct RepositoryDecisionTransport;

impl SystemOneTransport for RepositoryDecisionTransport {
    fn complete(
        &self,
        _api_key: &str,
        request: &Value,
    ) -> std::result::Result<Value, ProviderError> {
        let source_text = request["state"]["source_text"].as_str().unwrap_or_default();
        if !source_text.contains("## Decision\nKeep parsed query plans local to each process.") {
            return Err(ProviderError::permanent(
                "The explicit ADR decision section was not supplied as source text.".into(),
            ));
        }
        let questions = request["questions"]
            .as_object()
            .ok_or_else(|| ProviderError::permanent("Missing typed questions.".into()))?;
        let mut answers = serde_json::Map::new();
        for (key, question) in questions {
            match question["type"].as_str().unwrap_or_default() {
                "noul" => {
                    answers.insert(key.clone(), json!({"type":"noul","noul":0.99}));
                }
                "choice" => {
                    let criteria = question["criteria"].as_object().ok_or_else(|| {
                        ProviderError::permanent(format!("Missing criteria for {key}."))
                    })?;
                    let selected = match key.as_str() {
                        "source_update_role" => "supplement",
                        "source_update_evidence" => criteria
                            .keys()
                            .find(|candidate| candidate.starts_with("span_"))
                            .map(String::as_str)
                            .unwrap_or("span_0"),
                        "source_order_date" | "source_order_revision" => "none",
                        name if name.contains("repository_decision")
                            && criteria.contains_key("decision") =>
                        {
                            "decision"
                        }
                        _ => criteria.keys().next().map(String::as_str).unwrap_or("none"),
                    };
                    if !criteria.contains_key(selected) {
                        return Err(ProviderError::permanent(format!(
                            "Unexpected fixed fixture choice `{selected}` for {key}."
                        )));
                    }
                    answers.insert(
                        key.clone(),
                        json!({"type":"choice","choice":selected,"probabilities":{selected:0.99}}),
                    );
                }
                other => {
                    return Err(ProviderError::permanent(format!(
                        "Unexpected typed question `{other}` for {key}."
                    )))
                }
            }
        }
        Ok(json!({"model":"typesafe/jev-1.13-adr-fixture","answers":answers}))
    }
}

struct RepositoryOpenQuestionTransport;

impl SystemOneTransport for RepositoryOpenQuestionTransport {
    fn complete(
        &self,
        _api_key: &str,
        request: &Value,
    ) -> std::result::Result<Value, ProviderError> {
        let source_text = request["state"]["source_text"].as_str().unwrap_or_default();
        if !source_text.contains("## Open Questions\n\n- Should cache eviction use LRU?") {
            return Err(ProviderError::permanent(
                "The explicit repository question was not supplied as source text.".into(),
            ));
        }
        let questions = request["questions"]
            .as_object()
            .ok_or_else(|| ProviderError::permanent("Missing typed questions.".into()))?;
        let mut answers = serde_json::Map::new();
        for (key, question) in questions {
            match question["type"].as_str().unwrap_or_default() {
                "noul" => {
                    answers.insert(key.clone(), json!({"type":"noul","noul":0.99}));
                }
                "choice" => {
                    let criteria = question["criteria"].as_object().ok_or_else(|| {
                        ProviderError::permanent(format!("Missing criteria for {key}."))
                    })?;
                    let selected = match key.as_str() {
                        "source_update_role" => "supplement",
                        "source_update_evidence" => criteria
                            .keys()
                            .find(|candidate| candidate.starts_with("span_"))
                            .map(String::as_str)
                            .unwrap_or("span_0"),
                        "source_order_date" | "source_order_revision" => "none",
                        name if name.contains("repository_question")
                            && criteria.contains_key("question") =>
                        {
                            "question"
                        }
                        _ => criteria.keys().next().map(String::as_str).unwrap_or("none"),
                    };
                    if !criteria.contains_key(selected) {
                        return Err(ProviderError::permanent(format!(
                            "Unexpected fixed fixture choice `{selected}` for {key}."
                        )));
                    }
                    answers.insert(
                        key.clone(),
                        json!({"type":"choice","choice":selected,"probabilities":{selected:0.99}}),
                    );
                }
                other => {
                    return Err(ProviderError::permanent(format!(
                        "Unexpected typed question `{other}` for {key}."
                    )))
                }
            }
        }
        Ok(json!({"model":"typesafe/jev-1.13-open-question-fixture","answers":answers}))
    }
}

#[test]
fn repository_adr_decision_without_acceptance_status_is_navigable_with_exact_heading_span() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("sample-repository");
    fs::create_dir_all(repository.join("docs/adr")).unwrap();
    let relative_path = "docs/adr/0001-cache-locality.md";
    let adr = "# ADR-0001: Keep query plans local\n\n## Context\nThe service parses query plans frequently, and sharing cache entries across processes is not currently supported.\n\n## Decision\nKeep parsed query plans local to each process.\n\n## Consequences\nEach process maintains its own parsed-plan cache.\n";
    fs::write(repository.join(relative_path), adr).unwrap();
    let provider = Arc::new(JevSemanticProvider::with_transport(
        "repository-fixture-credential".into(),
        Arc::new(RepositoryDecisionTransport),
    ));
    let collection = workspace.path().join("collection");
    let mut app = Application::open_with_semantic_provider(&collection, provider).unwrap();

    let imported = app.import_repository(&repository).unwrap();
    let source_summary = imported
        .sources
        .iter()
        .find(|source| source.relative_path == relative_path)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();

    let source = app.open_source(&source_summary.source_id).unwrap();
    assert_eq!(source.info.semantic_state, "complete", "{source:?}");
    let decision = source
        .info
        .knowledge_pages
        .iter()
        .find(|page| page.kind == "decision" && page.title == "Keep query plans local")
        .expect("the explicit ADR decision should be a navigable target");
    let page = app.open_knowledge_page(&decision.page_id).unwrap();
    let exact_decision = "Keep parsed query plans local to each process.";
    let line = adr.lines().position(|text| text == exact_decision).unwrap() + 1;
    let version_id = source.info.current_version_id.as_ref().unwrap();
    assert!(page.markdown.contains(exact_decision), "{}", page.markdown);
    assert!(
        page.markdown
            .contains(&format!("source lines {line}–{line}")),
        "{}",
        page.markdown
    );
    assert!(page.markdown.contains(relative_path), "{}", page.markdown);
    assert!(page.markdown.contains(&source.info.source_id));
    assert!(page.markdown.contains(version_id));
    assert!(page.markdown.contains("Source page"));
    assert!(page.markdown.contains("Retained original"));
    assert!(!page.markdown.contains("Accepted"));
    assert!(!page.markdown.contains("implemented"));
    assert!(!page
        .markdown
        .contains("service parses query plans frequently"));
    assert_eq!(
        fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        adr.as_bytes()
    );
}

#[test]
fn repository_explicit_open_question_is_navigable_with_its_exact_source_span() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("sample-repository");
    fs::create_dir_all(repository.join("docs")).unwrap();
    let relative_path = "docs/cache-design.md";
    let document = "# Cache design\n\n## Open Questions\n\n- Should cache eviction use LRU?\n";
    fs::write(repository.join(relative_path), document).unwrap();
    let provider = Arc::new(JevSemanticProvider::with_transport(
        "repository-fixture-credential".into(),
        Arc::new(RepositoryOpenQuestionTransport),
    ));
    let collection = workspace.path().join("collection");
    let mut app = Application::open_with_semantic_provider(&collection, provider).unwrap();

    let imported = app.import_repository(&repository).unwrap();
    let source_summary = imported
        .sources
        .iter()
        .find(|source| source.relative_path == relative_path)
        .unwrap();
    app.resume_due_semantic_jobs().unwrap();

    let source = app.open_source(&source_summary.source_id).unwrap();
    assert_eq!(source.info.semantic_state, "complete", "{source:?}");
    let question = source
        .info
        .knowledge_pages
        .iter()
        .find(|page| page.kind == "question" && page.title == "Should cache eviction use LRU?")
        .expect("an explicit open question should become a navigable target");
    let page = app.open_knowledge_page(&question.page_id).unwrap();
    let line = document
        .lines()
        .position(|text| text == "- Should cache eviction use LRU?")
        .unwrap()
        + 1;
    let version_id = source.info.current_version_id.as_ref().unwrap();
    assert!(page.markdown.contains("Should cache eviction use LRU?"));
    assert!(page
        .markdown
        .contains(&format!("source lines {line}–{line}")));
    assert!(page.markdown.contains(relative_path));
    assert!(page.markdown.contains(&source.info.source_id));
    assert!(page.markdown.contains(version_id));
    assert!(page.markdown.contains("Source page"));
    assert!(page.markdown.contains("Retained original"));
    assert_eq!(
        fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        document.as_bytes()
    );
}

#[test]
fn repository_adr_decision_preserves_its_multiline_scope_qualification() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("qualified-repository");
    fs::create_dir_all(repository.join("docs/adr")).unwrap();
    let adr = "# ADR-0001: Keep query plans local\n\n## Decision\nKeep parsed query plans local to each process.\nOnly temporary test processes use this policy; production processes keep plans remotely.\n\n## Consequences\nTests remain isolated.\n";
    fs::write(repository.join("docs/adr/0001-cache.md"), adr).unwrap();
    let provider = Arc::new(JevSemanticProvider::with_transport(
        "repository-fixture-credential".into(),
        Arc::new(RepositoryDecisionTransport),
    ));
    let mut app =
        Application::open_with_semantic_provider(workspace.path().join("collection"), provider)
            .unwrap();
    let imported = app.import_repository(&repository).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&imported.sources[0].source_id).unwrap();
    assert_eq!(source.info.semantic_state, "complete", "{source:?}");
    let decision = source
        .knowledge_pages
        .iter()
        .find(|page| page.kind == "decision")
        .unwrap();
    let page = app.open_knowledge_page(&decision.page_id).unwrap();
    assert!(page.markdown.contains("Only temporary test processes use this policy; production processes keep plans remotely."), "the source's scope qualification must survive: {}", page.markdown);
    assert!(
        page.markdown.contains("source lines 4–5"),
        "{}",
        page.markdown
    );
    assert!(!page.markdown.contains("Tests remain isolated."));
}

#[cfg(unix)]
#[test]
fn repository_paths_retain_distinct_raw_names_and_repositories() {
    use std::ffi::OsString;
    let workspace = tempdir().unwrap();
    let mut app = Application::open(workspace.path().join("collection")).unwrap();
    let mut repository_ids = std::collections::HashSet::new();
    for name in ["first", "second"] {
        let repository = workspace.path().join(name);
        fs::create_dir_all(repository.join("a")).unwrap();
        for name in [
            OsString::from("a/b.rs"),
            OsString::from("a\\b.rs"),
            OsString::from("a%5Cb.rs"),
        ] {
            fs::write(repository.join(name), "/// A cache.\npub struct Cache;\n").unwrap();
        }
        let imported = app.import_repository(&repository).unwrap();
        assert!(repository_ids.insert(imported.repository_id.clone()));
        assert_eq!(imported.sources.len(), 3);
        assert_eq!(
            imported
                .sources
                .iter()
                .map(|s| &s.source_id)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            3
        );
        assert_eq!(
            imported
                .sources
                .iter()
                .map(|s| &s.relative_path)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            3
        );
        let repeated = app.import_repository(&repository).unwrap();
        assert_eq!(
            serde_json::to_value(repeated).unwrap(),
            serde_json::to_value(imported).unwrap()
        );
    }
}

#[test]
fn repository_ordinary_four_slash_comments_do_not_form_code_concepts() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("repository");
    fs::create_dir(&repository).unwrap();
    fs::write(
        repository.join("cache.rs"),
        "//// Ordinary comment.\npub struct Cache;\n",
    )
    .unwrap();
    let transport = Arc::new(RepositoryConceptTransport(AtomicUsize::new(0)));
    let provider = Arc::new(JevSemanticProvider::with_transport(
        "fixture".into(),
        transport.clone(),
    ));
    let mut app =
        Application::open_with_semantic_provider(workspace.path().join("collection"), provider)
            .unwrap();
    let imported = app.import_repository(&repository).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let source = app.open_source(&imported.sources[0].source_id).unwrap();
    assert!(source.knowledge_pages.is_empty());
    assert_eq!(transport.0.load(Ordering::SeqCst), 0);
}
