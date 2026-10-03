//! Test-only process adapter for the public Application boundary; not a product importer.
use knowledge_garden::application::{AcquisitionMethod, Application};
use knowledge_garden::providers::JevSemanticProvider;
use serde::Serialize;

fn output(value: impl Serialize) {
    println!(
        "{}",
        serde_json::to_string(&value).expect("serialize application result")
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let operation = args.get(2).map(String::as_str).ok_or("missing operation")?;
    let live = operation == "live-import";
    if live && std::env::var("KNOWLEDGE_GARDEN_LIVE_SEMANTICS").as_deref() != Ok("1") {
        return Err("live semantic evaluation requires KNOWLEDGE_GARDEN_LIVE_SEMANTICS=1".into());
    }
    let mut app = if live {
        Application::open_with_semantic_provider(
            args.get(1).ok_or("missing test collection")?,
            std::sync::Arc::new(JevSemanticProvider::from_environment_and_keychain()),
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
        "list" => output(app.list_sources(args.get(3).ok_or("missing offset")?.parse()?)?),
        _ => return Err("unknown test operation".into()),
    }
    Ok(())
}
