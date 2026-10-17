//! Test-only process adapter for the public Application boundary; not a product importer.
use knowledge_garden::application::{
    AcquisitionMethod, Application, GardenError, PageSearchRequest,
};
use knowledge_garden::extraction::LocalExtractor;
use knowledge_garden::media::WhisperAudioProcessor;
use knowledge_garden::providers::{JevSemanticProvider, SystemOneTransport};
use knowledge_garden::semantic::{
    EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft, ProviderError, RelationshipDraft,
    SemanticProvider,
};
use serde::Serialize;
use serde_json::{json, Value};

#[path = "../tests/fixtures/queries/provider.rs"]
mod labeled_query;

struct RecordedPathProvider;
impl SemanticProvider for RecordedPathProvider {
    fn form_knowledge(&self, text: &str) -> Result<KnowledgeDraft, ProviderError> {
        let evidence = |quote: &str| {
            let start = text.find(quote).expect("fixed path fixture span");
            EvidenceDraft {
                quote: quote.into(),
                byte_start: start,
                byte_end: start + quote.len(),
                origin: "recorded_fixture".into(),
                qualifier: None,
                offset_basis: None,
                source_location: None,
            }
        };
        let entities = [
            ("Maya", "person"),
            ("V17", "event"),
            ("Riverside", "place"),
            ("Riverside Park", "place"),
        ]
        .into_iter()
        .map(|(label, kind)| EntityDraft {
            label: label.into(),
            kind: kind.into(),
            evidence: evidence(label),
        })
        .collect();
        let relationships = [
            ("V17", "Maya", "observed_by", "Maya observed V17", None),
            (
                "V17",
                "Riverside",
                "occurred_at",
                "V17 occurred at Riverside",
                Some("uncertain location"),
            ),
            (
                "Riverside",
                "Riverside",
                "related_to",
                "Riverside is self-linked",
                None,
            ),
        ]
        .into_iter()
        .map(|(from, to, kind, quote, qualifier)| RelationshipDraft {
            from: from.into(),
            to: to.into(),
            kind: kind.into(),
            qualifier: qualifier.map(String::from),
            evidence: evidence(quote),
        })
        .collect();
        Ok(KnowledgeDraft {
            entities,
            relationships,
            ..KnowledgeDraft::default()
        })
    }
}

struct RecordedConversationTransport;

impl SystemOneTransport for RecordedConversationTransport {
    fn complete(&self, _key: &str, _request: &Value) -> Result<Value, ProviderError> {
        let mut answers = serde_json::Map::new();
        for (index, intent) in [
            (0, "question"),
            (1, "proposal"),
            (3, "tool_report"),
            (4, "reasoning_event"),
            (5, "recorded_decision"),
        ] {
            answers.insert(
                format!("conversation_intent_{index}"),
                json!({
                    "type":"choice", "choice":intent, "probabilities":{intent:0.99,"uncertain":0.01}
                }),
            );
        }
        Ok(json!({"model":"typesafe/jev-frozen-reader-fixture","answers":answers}))
    }
}

struct RecordedOfficeProvider;

impl SemanticProvider for RecordedOfficeProvider {
    fn form_knowledge(&self, source_text: &str) -> Result<KnowledgeDraft, ProviderError> {
        let quote = if source_text.contains("DOCX table") {
            "12 visits, excluding two unverified reports."
        } else {
            "An unconfirmed correction suggests the count may be 14 visits."
        };
        let start = source_text
            .find(quote)
            .ok_or_else(|| ProviderError::permanent("labeled evidence missing".into()))?;
        let evidence = EvidenceDraft {
            quote: quote.into(),
            byte_start: start,
            byte_end: start + quote.len(),
            origin: "recorded_evaluation".into(),
            qualifier: source_text
                .contains("channel=speaker_notes")
                .then(|| "unconfirmed note, not visible slide text".into()),
            offset_basis: None,
            source_location: None,
        };
        let label = if source_text.contains("DOCX table") {
            "Riverside field visit"
        } else {
            "Field visit summary"
        };
        Ok(KnowledgeDraft {
            entities: vec![EntityDraft {
                kind: "event".into(),
                label: label.into(),
                evidence: evidence.clone(),
            }],
            facts: vec![FactDraft {
                subject: label.into(),
                property: "reported visit count".into(),
                value: if source_text.contains("DOCX table") {
                    "12 visits, excluding two unverified reports".into()
                } else {
                    "possible correction to 14 visits".into()
                },
                evidence,
                record_key: None,
            }],
            ..KnowledgeDraft::default()
        })
    }
}

fn url_failure(error: GardenError) -> String {
    match error {
        GardenError::Invalid(message) if message.starts_with("URL acquisition returned HTTP ") => {
            message
        }
        _ => "URL acquisition failed; no source material was added.".into(),
    }
}

fn output(value: impl Serialize) {
    println!(
        "{}",
        serde_json::to_string(&value).expect("serialize application result")
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Opt-in held-out audio path: <collection> audio-live-import <fixture> <pinned-audio-assets>.
    // It exercises public import, one bounded audio resume, then semantic resume.
    let args: Vec<String> = std::env::args().collect();
    let operation = args.get(2).map(String::as_str).ok_or("missing operation")?;
    let live = matches!(
        operation,
        "live-import" | "url-live-import" | "audio-live-import" | "repository-live-import"
    );
    if live && std::env::var("KNOWLEDGE_GARDEN_LIVE_SEMANTICS").as_deref() != Ok("1") {
        return Err("live semantic evaluation requires KNOWLEDGE_GARDEN_LIVE_SEMANTICS=1".into());
    }
    let recorded_office = operation == "office-recorded-import";
    let recorded_conversation = operation == "conversation-recorded-import";
    let mut app = if matches!(
        operation,
        "audio-live-import" | "audio-local-import" | "audio-local-resume"
    ) {
        let audio_assets = args.get(4).ok_or("missing pinned audio asset directory")?;
        Application::open_with_all_providers(
            args.get(1).ok_or("missing test collection")?,
            std::sync::Arc::new(JevSemanticProvider::from_environment_and_keychain()),
            std::sync::Arc::new(LocalExtractor),
            std::sync::Arc::new(WhisperAudioProcessor::new(audio_assets)),
        )?
    } else if live {
        Application::open_with_semantic_provider(
            args.get(1).ok_or("missing test collection")?,
            std::sync::Arc::new(JevSemanticProvider::from_environment_and_keychain()),
        )?
    } else if operation == "boolean-recorded-import" {
        Application::open_with_semantic_provider(
            args.get(1).ok_or("missing test collection")?,
            std::sync::Arc::new(labeled_query::LabeledGraph),
        )?
    } else if operation == "path-recorded-import" {
        Application::open_with_semantic_provider(
            args.get(1).ok_or("missing test collection")?,
            std::sync::Arc::new(RecordedPathProvider),
        )?
    } else if recorded_conversation {
        Application::open_with_semantic_provider(
            args.get(1).ok_or("missing test collection")?,
            std::sync::Arc::new(JevSemanticProvider::with_transport(
                "fixture-only".into(),
                std::sync::Arc::new(RecordedConversationTransport),
            )),
        )?
    } else if recorded_office {
        Application::open_with_semantic_provider(
            args.get(1).ok_or("missing test collection")?,
            std::sync::Arc::new(RecordedOfficeProvider),
        )?
    } else {
        Application::open(args.get(1).ok_or("missing test collection")?)?
    };
    match operation {
        "source-exists" => {
            output(app.retained_source_exists(args.get(3).ok_or("missing source")?)?)
        }
        "watch" => output(app.watch_directory(args.get(3).ok_or("missing directory")?)?),
        "watches" => output(app.list_watched_directories()?),
        "watch-step" => {
            app.reconcile_watched_directories(
                args.get(3)
                    .map(|value| value.parse())
                    .transpose()?
                    .unwrap_or(128),
            )?;
            output(app.list_watched_directories()?);
        }
        "repository-import" | "repository-live-import" => {
            let imported = app.import_repository(args.get(3).ok_or("missing repository path")?)?;
            if live {
                app.resume_due_semantic_jobs()?;
            }
            let pages = imported
                .sources
                .iter()
                .map(|source| app.open_source(&source.source_id))
                .collect::<Result<Vec<_>, _>>()?;
            output(json!({"import": imported, "pages": pages}));
        }
        "live-import" => {
            let page = app.import_source(
                args.get(3).ok_or("missing fixture path")?,
                AcquisitionMethod::Picker,
            )?;
            app.resume_due_semantic_jobs()?;
            output(app.open_source(&page.info.source_id)?);
        }
        "audio-local-import" => {
            let page = app.import_source(
                args.get(3).ok_or("missing fixture path")?,
                AcquisitionMethod::Picker,
            )?;
            output(app.open_source(&page.info.source_id)?);
        }
        "audio-local-resume" => {
            app.resume_due_audio_jobs()?;
            output(app.open_source(args.get(3).ok_or("missing source identity")?)?);
        }
        "audio-live-import" => {
            let page = app.import_source(
                args.get(3).ok_or("missing fixture path")?,
                AcquisitionMethod::Picker,
            )?;
            app.resume_due_audio_jobs()?;
            app.resume_due_semantic_jobs()?;
            output(app.open_source(&page.info.source_id)?);
        }
        "url-import" | "url-retry" => {
            let url = args.get(3).ok_or("missing URL")?;
            match app.import_url(url) {
                Ok(page) => output(page),
                Err(error) => return Err(url_failure(error).into()),
            }
        }
        "url-live-import" => {
            let url = args.get(3).ok_or("missing URL")?;
            let page = match app.import_url(url) {
                Ok(page) => page,
                Err(error) => return Err(url_failure(error).into()),
            };
            app.resume_due_semantic_jobs()?;
            output(app.open_source(&page.info.source_id)?);
        }
        "office-recorded-import"
        | "conversation-recorded-import"
        | "path-recorded-import"
        | "boolean-recorded-import" => {
            let page = app.import_source(
                args.get(3).ok_or("missing fixture path")?,
                AcquisitionMethod::Picker,
            )?;
            app.resume_due_semantic_jobs()?;
            output(app.open_source(&page.info.source_id)?);
        }
        "import" => output(app.import_source(
            args.get(3).ok_or("missing fixture path")?,
            match args.get(4).map(String::as_str) {
                Some("picker") => AcquisitionMethod::Picker,
                Some("drop") => AcquisitionMethod::Drop,
                _ => return Err("missing acquisition method".into()),
            },
        )?),
        "open" => output(app.open_source(args.get(3).ok_or("missing source identity")?)?),
        "knowledge" => {
            output(app.open_knowledge_page(args.get(3).ok_or("missing knowledge page identity")?)?)
        }
        "original" => output(app.original_path(args.get(3).ok_or("missing source identity")?)?),
        "original-version" => output(app.original_version_path(
            args.get(3).ok_or("missing source identity")?,
            args.get(4).ok_or("missing source version identity")?,
            args.get(5).ok_or("missing retained asset")?,
        )?),
        "original-asset" => output(app.original_asset_path(
            args.get(3).ok_or("missing source identity")?,
            args.get(4).ok_or("missing retained asset")?,
        )?),
        "claim" => {
            let jobs = app.claim_due_semantic_jobs(8)?;
            output(jobs.iter().map(|job| serde_json::json!({"source_id": job.source_id, "source_text": job.source_text})).collect::<Vec<_>>());
        }
        "recover-open" => {
            let _ = app.claim_due_semantic_jobs(8)?;
            output(app.open_source(args.get(3).ok_or("missing source identity")?)?);
        }
        "query-pages" => output(app.query_pages(serde_json::from_str(
            args.get(3).ok_or("missing Boolean request")?,
        )?)?),
        "cancel-query" => {
            app.cancel_query(args.get(3).ok_or("missing Boolean cursor")?)?;
            output(serde_json::json!({"canceled":true}));
        }
        "path-revision" => output(app.path_graph_revision()?),
        "paths" => output(app.explore_paths(serde_json::from_str(
            args.get(3).ok_or("missing path request")?,
        )?)?),
        "path-details" => output(app.explore_path_details(serde_json::from_str(
            args.get(3).ok_or("missing detail request")?,
        )?)?),
        "list" => output(app.list_sources(args.get(3).ok_or("missing offset")?.parse()?)?),
        "search" => {
            let query_or_request = args.get(3).cloned().unwrap_or_default();
            let request = if query_or_request.trim_start().starts_with('{') {
                serde_json::from_str::<PageSearchRequest>(&query_or_request)?
            } else {
                PageSearchRequest {
                    query: query_or_request,
                    ..PageSearchRequest::default()
                }
            };
            output(app.search_pages(request)?);
        }
        _ => return Err("unknown test operation".into()),
    }
    Ok(())
}
