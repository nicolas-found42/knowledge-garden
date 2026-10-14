use knowledge_garden::application::boolean::{BooleanQueryRequest, BooleanQueryResponse};
use knowledge_garden::application::{AcquisitionMethod, Application};
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::tempdir;
#[path = "fixtures/queries/provider.rs"]
mod labeled;
use labeled::{LabeledGraph, TEXT};
fn request(expression: Value) -> BooleanQueryRequest {
    serde_json::from_value(json!({"expression": expression, "returned": {}, "continuation": null, "work_budget": 2, "page_size": 2})).unwrap()
}
fn collect(
    app: &mut Application,
    mut request: BooleanQueryRequest,
) -> Vec<knowledge_garden::application::boolean::BooleanQueryResult> {
    let mut results = Vec::new();
    for _ in 0..1000 {
        let response: BooleanQueryResponse = app.query_pages(request.clone()).unwrap();
        assert!(response.work_units <= 2);
        results.extend(response.results);
        if response.complete {
            return results;
        }
        let cursor = response.next_cursor.unwrap();
        assert!(cursor.len() < 128);
        request.continuation = Some(cursor);
    }
    panic!("finite typed query should finish");
}
#[test]
fn mixed_boolean_hops_scopes_and_known_set_negation_have_exact_authored_results() {
    let workspace = tempdir().unwrap();
    let source = workspace.path().join("field-notes.txt");
    std::fs::write(&source, TEXT).unwrap();
    let mut app = Application::open_with_semantic_provider(
        workspace.path().join("collection"),
        Arc::new(LabeledGraph),
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
    let id = |name: &str| {
        pages
            .iter()
            .find(|p| p.title == name)
            .unwrap()
            .page_id
            .clone()
    };
    let path = json!({"op":"path","start_page_id":id("Maya"),"hops":[{"kind":"observed_by","direction":"against_arrow"},{"kind":"occurred_at","direction":"with_arrow"}],"endpoint":{},"intermediate":{"date_from":"2024-05-17","date_to":"2024-05-17"}});
    let results = collect(&mut app, request(path.clone()));
    assert_eq!(
        results
            .iter()
            .map(|p| p.page_id.as_str())
            .collect::<Vec<_>>(),
        vec![id("Riverside")]
    );
    assert_eq!(results[0].paths.len(), 1);
    let detail = app
        .explore_path_details(knowledge_garden::application::PathDetailsRequest {
            detail_token: results[0].paths[0].detail_token.clone(),
            continuation: None,
            work_budget: 10,
        })
        .unwrap();
    assert!(detail.steps.iter().any(|s| s.evidence_quote
        == "V17 occurred at Riverside with location uncertainty"
        && s.qualifier.as_deref() == Some("location uncertainty")));
    let mut second = path.clone();
    second["intermediate"] = json!({"date_from":"2024-06-18"});
    let either = json!({"op":"or","children":[path.clone(),second.clone()]});
    let results = collect(&mut app, request(either.clone()));
    let mut ids = results
        .iter()
        .map(|p| p.page_id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    let mut expected = vec![id("Paris"), id("Riverside")];
    expected.sort();
    assert_eq!(ids, expected);
    let conjunction = json!({"op":"and","children":[path.clone(),second]});
    assert!(collect(&mut app, request(conjunction)).is_empty());
    let negated = json!({"op":"and","children":[either,{"op":"not","child":path.clone()}]});
    assert_eq!(
        collect(&mut app, request(negated))
            .iter()
            .map(|p| p.page_id.clone())
            .collect::<Vec<_>>(),
        vec![id("Paris")]
    );
    let pending = workspace.path().join("pending.txt");
    std::fs::write(&pending, "Pending unprocessed source.").unwrap();
    let pending_source = app
        .import_source(pending, AcquisitionMethod::Picker)
        .unwrap();
    let exclusion = json!({"op":"not","child":{"op":"constraint","constraint":{"processing_status":"pending"}}});
    let excluded = collect(&mut app, request(exclusion));
    assert!(!excluded
        .iter()
        .any(|p| p.page_id == pending_source.info.page_id));
    assert!(
        excluded.iter().any(|p| p.page_id == id("Theo")),
        "known-set negation includes unconnected Theo without asserting a fabricated relationship"
    );
    assert!(app.open_source(&pending_source.info.source_id).is_ok());
    assert!(app.open_knowledge_page(&id("Theo")).is_ok());
    let cycle = json!({"op":"path","start_page_id":id("Maya"),"hops":[{"kind":"observed_by","direction":"against_arrow"},{"kind":"occurred_at","direction":"with_arrow"},{"kind":"links_to","direction":"with_arrow"}],"endpoint":{},"intermediate":{}});
    assert_eq!(
        collect(&mut app, request(cycle))
            .iter()
            .map(|p| p.page_id.clone())
            .collect::<Vec<_>>(),
        vec![id("Maya")]
    );
    let mut no_match = path;
    no_match["hops"][1]["direction"] = json!("against_arrow");
    assert!(collect(&mut app, request(no_match)).is_empty());
    let request = request(json!({"op":"constraint","constraint":{}}));
    let first = app.query_pages(request.clone()).unwrap();
    let mut resumed = request.clone();
    resumed.continuation = first.next_cursor.clone();
    app.cancel_query(first.next_cursor.as_deref().unwrap())
        .unwrap();
    assert!(app
        .query_pages(resumed)
        .unwrap_err()
        .to_string()
        .contains("canceled"));
    let first = app.query_pages(request.clone()).unwrap();
    let changed = workspace.path().join("changed.txt");
    std::fs::write(&changed, "New indexed source changes the known set.").unwrap();
    app.import_source(changed, AcquisitionMethod::Picker)
        .unwrap();
    let mut resumed = request;
    resumed.continuation = first.next_cursor;
    assert!(app
        .query_pages(resumed)
        .unwrap_err()
        .to_string()
        .contains("stale"));
}
