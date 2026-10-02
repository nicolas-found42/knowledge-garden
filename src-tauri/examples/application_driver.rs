//! Test-only process adapter for the public Application boundary; not a product importer.
use knowledge_garden::application::{AcquisitionMethod, Application};
use serde::Serialize;

fn output(value: impl Serialize) {
    println!(
        "{}",
        serde_json::to_string(&value).expect("serialize application result")
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut app = Application::open(args.get(1).ok_or("missing test collection")?)?;
    match args.get(2).map(String::as_str) {
        Some("import") => output(app.import_source(
            args.get(3).ok_or("missing fixture path")?,
            match args.get(4).map(String::as_str) {
                Some("picker") => AcquisitionMethod::Picker,
                Some("drop") => AcquisitionMethod::Drop,
                _ => return Err("missing acquisition method".into()),
            },
        )?),
        Some("open") => output(app.open_source(args.get(3).ok_or("missing source identity")?)?),
        Some("original") => {
            output(app.original_path(args.get(3).ok_or("missing source identity")?)?)
        }
        Some("list") => output(app.list_sources(args.get(3).ok_or("missing offset")?.parse()?)?),
        _ => return Err("unknown test operation".into()),
    }
    Ok(())
}
