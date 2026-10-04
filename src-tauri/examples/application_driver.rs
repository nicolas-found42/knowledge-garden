//! Test-only process adapter for the public Application boundary; not a product importer.
use knowledge_garden::application::{
    AcquisitionMethod, Application, GardenError, PageSearchRequest,
};
use knowledge_garden::providers::JevSemanticProvider;
use knowledge_garden::semantic::{
    EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft, ProviderError, SemanticProvider,
};
use serde::Serialize;

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
    let args: Vec<String> = std::env::args().collect();
    let operation = args.get(2).map(String::as_str).ok_or("missing operation")?;
    let live = matches!(operation, "live-import" | "url-live-import");
    if live && std::env::var("KNOWLEDGE_GARDEN_LIVE_SEMANTICS").as_deref() != Ok("1") {
        return Err("live semantic evaluation requires KNOWLEDGE_GARDEN_LIVE_SEMANTICS=1".into());
    }
    let recorded_office = operation == "office-recorded-import";
    let mut app = if live {
        Application::open_with_semantic_provider(
            args.get(1).ok_or("missing test collection")?,
            std::sync::Arc::new(JevSemanticProvider::from_environment_and_keychain()),
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
        "live-import" => {
            let page = app.import_source(
                args.get(3).ok_or("missing fixture path")?,
                AcquisitionMethod::Picker,
            )?;
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
        "office-recorded-import" => {
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
