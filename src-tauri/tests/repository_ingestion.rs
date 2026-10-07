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
        "# Garden Sample\n\nCache stays local.\n",
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
        b"# Garden Sample\n\nCache stays local.\n"
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
                            && criteria.contains_key("decision") => "decision",
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

#[test]
fn repository_adr_decision_is_navigable_with_its_exact_heading_span() {
    let workspace = tempdir().unwrap();
    let repository = workspace.path().join("sample-repository");
    fs::create_dir_all(repository.join("docs/adr")).unwrap();
    let relative_path = "docs/adr/0001-cache-locality.md";
    let adr = "# ADR-0001: Keep query plans local\n\nStatus: Accepted\n\n## Context\nThe service parses query plans frequently, and sharing cache entries across processes is not currently supported.\n\n## Decision\nKeep parsed query plans local to each process.\n\n## Consequences\nEach process maintains its own parsed-plan cache.\n";
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
        .expect("an accepted ADR decision should be a navigable target");
    let page = app.open_knowledge_page(&decision.page_id).unwrap();
    let exact_decision = "Keep parsed query plans local to each process.";
    let line = adr.lines().position(|text| text == exact_decision).unwrap() + 1;
    let version_id = source.info.current_version_id.as_ref().unwrap();
    assert!(page.markdown.contains(exact_decision), "{}", page.markdown);
    assert!(
        page.markdown.contains(&format!("source lines {line}–{line}")),
        "{}",
        page.markdown
    );
    assert!(page.markdown.contains(relative_path), "{}", page.markdown);
    assert!(page.markdown.contains(&source.info.source_id));
    assert!(page.markdown.contains(version_id));
    assert!(page.markdown.contains("Source page"));
    assert!(page.markdown.contains("Retained original"));
    assert!(!page.markdown.contains("service parses query plans frequently"));
    assert_eq!(
        fs::read(app.original_path(&source.info.source_id).unwrap()).unwrap(),
        adr.as_bytes()
    );
}
