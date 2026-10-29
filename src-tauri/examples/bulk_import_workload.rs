//! Reproducible bulk-import workload for the bounded-import budgets in #33.
//! It drives only the public Application boundary and prints one JSON record.
use knowledge_garden::application::{Application, PageSearchRequest};
use knowledge_garden::extraction::LocalExtractor;
use knowledge_garden::semantic::{
    EvidenceDraft, FactDraft, KnowledgeDraft, ProviderError, SemanticProvider,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// Deterministic stand-in for remote semantics so throughput measures the
/// application's own bounded work instead of provider latency.
struct RecordedWorkloadProvider;

impl SemanticProvider for RecordedWorkloadProvider {
    fn form_knowledge(&self, text: &str) -> Result<KnowledgeDraft, ProviderError> {
        let quote = text.lines().next().unwrap_or("").trim();
        if quote.is_empty() {
            return Ok(KnowledgeDraft::default());
        }
        let start = text.find(quote).unwrap_or(0);
        let evidence = EvidenceDraft {
            quote: quote.into(),
            byte_start: start,
            byte_end: start + quote.len(),
            origin: "recorded_workload".into(),
            qualifier: None,
            offset_basis: None,
            source_location: None,
        };
        Ok(KnowledgeDraft {
            facts: vec![FactDraft {
                subject: "Bulk report".into(),
                property: "line".into(),
                value: quote.chars().take(80).collect(),
                evidence,
                record_key: None,
            }],
            ..KnowledgeDraft::default()
        })
    }
}

fn peak_rss_bytes() -> u64 {
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output();
    output
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|text| text.trim().parse::<u64>().ok())
        .map(|kilobytes| kilobytes * 1024)
        .unwrap_or(0)
}

struct Corpus {
    root: PathBuf,
    files: Vec<PathBuf>,
}

/// Mixed sizes so a single large file cannot dominate the throughput figure.
fn build_corpus(root: &Path, files: usize) -> Result<Corpus, Box<dyn std::error::Error>> {
    fs::create_dir_all(root)?;
    let mut paths = Vec::with_capacity(files);
    for index in 0..files {
        let lines = match index % 10 {
            0 => 400,
            1..=6 => 40,
            _ => 4,
        };
        let path = root.join(format!("report-{index:06}.txt"));
        let mut body = format!("Bulk report {index}.\n");
        for line in 0..lines {
            body.push_str(&format!(
                "Observation {index}-{line} at Riverside; measurement {line} recorded for the bulk workload.\n"
            ));
        }
        fs::write(&path, body)?;
        paths.push(path);
    }
    Ok(Corpus {
        root: root.to_path_buf(),
        files: paths,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let collection = PathBuf::from(args.get(1).ok_or("missing collection path")?);
    let incoming = PathBuf::from(args.get(2).ok_or("missing corpus path")?);
    let files: usize = args
        .get(3)
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(2000);
    if collection.exists() && fs::read_dir(&collection)?.next().is_some() {
        return Err("the workload requires an empty collection".into());
    }

    let corpus = build_corpus(&incoming, files)?;
    let corpus_bytes: u64 = corpus
        .files
        .iter()
        .filter_map(|path| fs::metadata(path).ok())
        .map(|metadata| metadata.len())
        .sum();

    let mut app = Application::open_with_providers(
        &collection,
        Arc::new(RecordedWorkloadProvider),
        Arc::new(LocalExtractor),
    )?;
    app.watch_directory(&corpus.root)?;

    let import_started = Instant::now();
    let mut turns = 0u32;
    let mut navigation_samples = Vec::new();
    let mut searched_sources: Vec<String> = Vec::new();
    loop {
        turns += 1;
        app.reconcile_watched_directories(4)?;
        app.resume_due_semantic_jobs()?;
        let watch = app
            .list_watched_directories()?
            .into_iter()
            .next()
            .ok_or("the watched directory disappeared")?;
        // Every turn, do what a reader does while the import runs: search, then
        // open the most recent processed source. Both are timed individually.
        let search_started = Instant::now();
        let results = app.search_pages(PageSearchRequest {
            query: "Riverside".into(),
            ..PageSearchRequest::default()
        })?;
        navigation_samples.push(search_started.elapsed());
        if let Some(page) = results.pages.first() {
            if !searched_sources.iter().any(|id| id == &page.source_id) {
                let open_started = Instant::now();
                app.open_source(&page.source_id)?;
                navigation_samples.push(open_started.elapsed());
                searched_sources.push(page.source_id.clone());
            }
        }
        if watch.state != "scanning" && watch.processed_files >= files as u64 {
            break;
        }
        if turns > 100_000 {
            return Err("the watched import did not finish".into());
        }
    }
    let import_elapsed = import_started.elapsed();

    let mut sources = 0usize;
    let mut offset = 0usize;
    loop {
        let page = app.list_sources(offset)?;
        sources += page.sources.len();
        match page.next_offset {
            Some(next) => offset = next,
            None => break,
        }
    }
    if sources != files {
        return Err(format!("expected {files} sources, imported {sources}").into());
    }

    // A newer revision of a processed source must be retained as a second
    // version without a full reimport.
    let revision_name = "report-000000";
    let mut revised_source = None;
    let mut find_offset = 0usize;
    loop {
        let page = app.list_sources(find_offset)?;
        if let Some(source) = page
            .sources
            .into_iter()
            .find(|source| source.title == revision_name)
        {
            revised_source = Some(source.source_id);
            break;
        }
        match page.next_offset {
            Some(next) => find_offset = next,
            None => break,
        }
    }
    let revised_source =
        revised_source.ok_or("the corpus source is missing from the collection")?;
    let revision_path = corpus.root.join(format!("{revision_name}.txt"));
    let mut revised = fs::read_to_string(&revision_path)?;
    revised.push_str("Revision two records a newer observation for the same report.\n");
    fs::write(&revision_path, revised)?;
    let update_started = Instant::now();
    let mut versions = 0usize;
    for _ in 0..20_000 {
        app.reconcile_watched_directories(4)?;
        versions = app.open_source(&revised_source)?.info.versions_seen.len();
        if versions >= 2 {
            break;
        }
    }
    let update_elapsed = update_started.elapsed();
    if versions < 2 {
        return Err("the newer revision was not retained as a second version".into());
    }

    let rebuild_started = Instant::now();
    app.rebuild_index()?;
    let rebuild_elapsed = rebuild_started.elapsed();

    let mut sorted = navigation_samples.clone();
    sorted.sort();
    let percentile = |p: usize| -> u128 {
        if sorted.is_empty() {
            return 0;
        }
        sorted[(sorted.len() - 1) * p / 100].as_micros()
    };
    let search_only: Vec<u128> = navigation_samples
        .iter()
        .take(navigation_samples.len() / 2)
        .map(|value| value.as_micros())
        .collect();

    let record = json!({
        "files": files,
        "corpus_bytes": corpus_bytes,
        "import_seconds": import_elapsed.as_secs_f64(),
        "sources_per_second": files as f64 / import_elapsed.as_secs_f64(),
        "bytes_per_second": corpus_bytes as f64 / import_elapsed.as_secs_f64(),
        "worker_turns": turns,
        "concurrent_navigation_samples": navigation_samples.len(),
        "concurrent_navigation_p50_us": percentile(50),
        "concurrent_navigation_p95_us": percentile(95),
        "concurrent_search_samples": search_only.len(),
        "incremental_update_seconds": update_elapsed.as_secs_f64(),
        "cold_rebuild_seconds": rebuild_elapsed.as_secs_f64(),
        "peak_rss_bytes": peak_rss_bytes(),
        "hardware": {
            "model": command("sysctl", &["-n", "hw.model"]),
            "memory_bytes": command("sysctl", &["-n", "hw.memsize"]),
            "macos": command("sw_vers", &["-productVersion"]),
            "build": command("sw_vers", &["-buildVersion"]),
        },
        "build_profile": if cfg!(debug_assertions) { "debug" } else { "release" },
    });
    println!("{}", serde_json::to_string(&record)?);
    Ok(())
}

fn command(binary: &str, args: &[&str]) -> String {
    std::process::Command::new(binary)
        .args(args)
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim().to_owned())
        .unwrap_or_default()
}
