use knowledge_garden::application::{views::ViewDefinition, AcquisitionMethod, Application};
use serde_json::json;
use std::{fs, sync::Arc};
#[path = "fixtures/queries/provider.rs"]
mod labeled;
use labeled::{LabeledGraph, TEXT};
#[path = "fixtures/views/mod.rs"]
mod legacy_views;
fn definition(start: &str) -> ViewDefinition {
    serde_json::from_value(json!({
        "active":"search",
        "search":{"query":"Riverside","tags":[],"date_from":null,"date_to":null,"format":"txt","processing_status":"complete","offset":0},
        "query":{"expression":{"op":"path","start_page_id":start,"hops":[{"kind":"observed_by","direction":"against_arrow"},{"kind":"occurred_at","direction":"with_arrow"}],"endpoint":{},"intermediate":{"date_from":"2024-05-17","date_to":"2024-05-17"}},"returned":{},"continuation":null,"work_budget":50,"page_size":50},
        "collection":{"axis":"topic","query":"River","presentation":"canvas","expanded_groups":[{"group_id":format!("subject:{start}"),"label":"Maya"}],"selected":null},
        "timeline":{"date_meaning":"source_event","order":"descending","query":"River","tags":[],"offset":0},
        "paths":null,"comparison":[],"target":null
    })).unwrap()
}
#[test]
fn saved_composite_criteria_survive_restart_and_rebuild_with_current_membership() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let mut app = Application::open_with_semantic_provider(&root, Arc::new(LabeledGraph)).unwrap();
    let input = temp.path().join("old.txt");
    fs::write(&input, TEXT).unwrap();
    let old = app.import_source(input, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    let old = app.open_source(&old.info.source_id).unwrap();
    let event = old
        .knowledge_pages
        .iter()
        .find(|p| p.title == "Maya")
        .unwrap();
    let criteria = definition(&event.page_id);
    assert_eq!(
        app.query_pages(criteria.query.clone().unwrap())
            .unwrap()
            .results
            .len(),
        1
    );
    // Running ordinary exploration creates no durable view.
    app.search_pages(criteria.search.clone().unwrap()).unwrap();
    app.timeline(criteria.timeline.clone().unwrap()).unwrap();
    assert!(app.list_saved_views("", 0).unwrap().views.is_empty());
    let saved = legacy_views::seed(&root, "River fieldwork", &criteria);
    let path = root.join(&app.list_saved_views("", 0).unwrap().views[0].path);
    let markdown = fs::read_to_string(&path).unwrap();
    assert!(markdown.contains("# River fieldwork"));
    assert!(!markdown.contains("graph_revision") && !markdown.contains("results:"));
    let input = temp.path().join("new.txt");
    fs::write(&input, format!("{TEXT}\nAdditional independent fieldwork.")).unwrap();
    let new = app.import_source(input, AcquisitionMethod::Picker).unwrap();
    app.resume_due_semantic_jobs().unwrap();
    fs::remove_dir_all(
        app.page_path(&old.info.source_id)
            .unwrap()
            .parent()
            .unwrap(),
    )
    .unwrap();
    drop(app);
    let mut app = Application::open(&root).unwrap();
    app.rebuild_index().unwrap();
    let reopened = app.open_saved_view(&saved).unwrap();
    assert_eq!(
        serde_json::to_value(&reopened.definition).unwrap(),
        serde_json::to_value(criteria).unwrap()
    );
    let matches = app
        .search_pages(reopened.definition.search.unwrap())
        .unwrap();
    let sources: Vec<_> = matches
        .pages
        .iter()
        .filter(|p| p.page_type == "source")
        .map(|p| p.source_id.as_str())
        .collect();
    assert_eq!(sources, vec![new.info.source_id.as_str()]);
    let result = app.query_pages(reopened.definition.query.unwrap()).unwrap();
    assert!(result.complete);
    // The removed start identity is never silently replaced by a same-named new entity.
    assert!(result.results.is_empty());
    assert_eq!(app.list_saved_views("fieldwork", 0).unwrap().views.len(), 1);
    assert_eq!(fs::read_to_string(path).unwrap(), markdown);
}
#[test]
fn legacy_filename_and_owner_prose_remain_unchanged_when_opened() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("garden");
    let app = Application::open(&root).unwrap();
    let saved = legacy_views::seed(&root, "River fieldwork", &definition("page-v17"));
    let path = root.join("views").join(format!("{saved}.md"));
    let renamed = root.join("views/owner-file.md");
    let text = fs::read_to_string(&path).unwrap() + "\nOwner commentary stays here.\n";
    fs::write(&path, &text).unwrap();
    fs::rename(path, &renamed).unwrap();
    assert!(!app.open_saved_view(&saved).unwrap().warnings.is_empty());
    assert_eq!(fs::read_to_string(renamed).unwrap(), text);
}
#[test]
fn malformed_duplicate_and_symlinked_owner_definitions_are_preserved_without_choosing_one() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("garden");
    let app = Application::open(&root).unwrap();
    let saved = legacy_views::seed(&root, "River fieldwork", &definition("page-v17"));
    let path = root.join(&app.list_saved_views("", 0).unwrap().views[0].path);
    let text = fs::read_to_string(&path).unwrap();
    fs::write(root.join("views/duplicate.md"), &text).unwrap();
    assert!(app.open_saved_view(&saved).is_err());
    fs::remove_file(root.join("views/duplicate.md")).unwrap();
    fs::write(
        root.join("views/future.md"),
        text.replacen("schema: 1", "schema: 99", 1),
    )
    .unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&path, root.join("views/link.md")).unwrap();
    let catalog = app.list_saved_views("", 0).unwrap();
    assert_eq!(catalog.views.len(), 1);
    assert!(catalog
        .diagnostics
        .iter()
        .any(|d| d.path.ends_with("future.md")));
    #[cfg(unix)]
    assert!(catalog
        .diagnostics
        .iter()
        .any(|d| d.path.ends_with("link.md")));
    assert_eq!(fs::read_to_string(&path).unwrap(), text);
    #[cfg(unix)]
    {
        let outside = temp.path().join("outside-views");
        fs::rename(root.join("views"), &outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("views")).unwrap();
        assert!(app.list_saved_views("", 0).is_err());
        assert_eq!(
            fs::read_to_string(outside.join(path.file_name().unwrap())).unwrap(),
            text
        );
    }
}
