use crate::application::boolean::{BooleanQueryRequest, BooleanQueryResponse};
use crate::{
    application::{
        AcquisitionMethod, Application, PageSearchRequest, PageSearchResults, PathDetailsRequest,
        PathDetailsResponse, PathExploreRequest, PathExploreResponse, SourceList, SourcePage,
        UrlAcquisitionStatus,
    },
    extraction::LocalExtractor,
    media::WhisperAudioProcessor,
    providers::JevSemanticProvider,
    semantic::{KnowledgePage, SemanticProvider},
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{Manager, State};

struct Engine {
    app: Arc<Mutex<Application>>,
}

async fn with_engine<T: Send + 'static>(
    engine: &Engine,
    operation: impl FnOnce(&mut Application) -> crate::application::Result<T> + Send + 'static,
) -> Result<T, String> {
    let engine = Arc::clone(&engine.app);
    tauri::async_runtime::spawn_blocking(move || {
        let mut app = engine
            .lock()
            .map_err(|_| "The collection worker stopped unexpectedly.".to_owned())?;
        operation(&mut app).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn open_collection_file(engine: State<'_, Engine>, path: String) -> Result<(), String> {
    with_engine(&engine, move |app| {
        let path = app.collection_file_path(&path)?;
        let status = std::process::Command::new("/usr/bin/open")
            .arg(path)
            .status()?;
        if !status.success() {
            return Err(crate::application::GardenError::Invalid(
                "macOS could not open this preserved Markdown file.".into(),
            ));
        }
        Ok(())
    })
    .await
}

#[tauri::command]
async fn open_comparison_original(
    engine: State<'_, Engine>,
    request: crate::application::comparison::ComparisonRequest,
) -> Result<(), String> {
    with_engine(&engine, move |app| {
        let original = app.comparison_original_path(request)?;
        open_retained_original(&original)
    })
    .await
}

#[tauri::command]
async fn comparison_side(
    engine: State<'_, Engine>,
    request: crate::application::comparison::ComparisonRequest,
) -> Result<crate::application::comparison::ComparisonSide, String> {
    with_engine(&engine, move |app| app.comparison_side(request)).await
}

#[tauri::command]
async fn collection_groups(
    engine: State<'_, Engine>,
    request: crate::application::collection::CollectionGroupRequest,
) -> Result<crate::application::collection::CollectionGroups, String> {
    with_engine(&engine, move |app| app.collection_groups(request)).await
}
#[tauri::command]
async fn collection_members(
    engine: State<'_, Engine>,
    request: crate::application::collection::CollectionMemberRequest,
) -> Result<crate::application::collection::CollectionMembers, String> {
    with_engine(&engine, move |app| app.collection_members(request)).await
}

#[tauri::command]
async fn inspect_gaps(
    engine: State<'_, Engine>,
    page_id: String,
) -> Result<crate::application::gaps::GapInspection, String> {
    with_engine(&engine, move |app| app.inspect_gaps(&page_id)).await
}

#[tauri::command]
async fn collection_status(
    engine: State<'_, Engine>,
) -> Result<crate::application::reconciliation::ReconciliationStatus, String> {
    with_engine(&engine, |app| Ok(app.collection_status())).await
}

#[tauri::command]
async fn resolve_page_link(
    engine: State<'_, Engine>,
    page_id: String,
    href: String,
) -> Result<String, String> {
    with_engine(&engine, move |app| app.resolve_page_link(&page_id, &href)).await
}

#[tauri::command]
async fn import_source(
    engine: State<'_, Engine>,
    path: String,
    method: AcquisitionMethod,
) -> Result<SourcePage, String> {
    with_engine(&engine, move |app| app.import_source(path, method)).await
}

#[tauri::command]
async fn import_repository(
    engine: State<'_, Engine>,
    path: String,
) -> Result<crate::application::RepositoryImportResult, String> {
    with_engine(&engine, move |app| app.import_repository(path)).await
}

#[tauri::command]
async fn watch_directory(
    engine: State<'_, Engine>,
    path: String,
) -> Result<crate::application::watched::WatchedDirectory, String> {
    with_engine(&engine, move |app| app.watch_directory(path)).await
}

#[tauri::command]
async fn list_watched_directories(
    engine: State<'_, Engine>,
) -> Result<Vec<crate::application::watched::WatchedDirectory>, String> {
    with_engine(&engine, |app| app.list_watched_directories()).await
}

#[tauri::command]
async fn import_url(engine: State<'_, Engine>, url: String) -> Result<SourcePage, String> {
    with_engine(&engine, move |app| app.import_url(&url)).await
}

#[tauri::command]
async fn list_url_acquisitions(
    engine: State<'_, Engine>,
) -> Result<Vec<UrlAcquisitionStatus>, String> {
    with_engine(&engine, |app| app.list_url_acquisitions()).await
}

#[tauri::command]
async fn open_source(engine: State<'_, Engine>, source_id: String) -> Result<SourcePage, String> {
    with_engine(&engine, move |app| app.open_source(&source_id)).await
}

#[tauri::command]
async fn retained_source_exists(
    engine: State<'_, Engine>,
    source_id: String,
) -> Result<bool, String> {
    with_engine(&engine, move |app| app.retained_source_exists(&source_id)).await
}

#[tauri::command]
async fn open_knowledge_page(
    engine: State<'_, Engine>,
    page_id: String,
) -> Result<KnowledgePage, String> {
    with_engine(&engine, move |app| app.open_knowledge_page(&page_id)).await
}

#[tauri::command]
async fn list_sources(engine: State<'_, Engine>, offset: usize) -> Result<SourceList, String> {
    with_engine(&engine, move |app| app.list_sources(offset)).await
}

#[tauri::command]
async fn search_pages(
    engine: State<'_, Engine>,
    request: PageSearchRequest,
) -> Result<PageSearchResults, String> {
    with_engine(&engine, move |app| app.search_pages(request)).await
}

#[tauri::command]
async fn query_pages(
    engine: State<'_, Engine>,
    request: BooleanQueryRequest,
) -> Result<BooleanQueryResponse, String> {
    with_engine(&engine, move |app| app.query_pages(request)).await
}
#[tauri::command]
async fn cancel_query(engine: State<'_, Engine>, continuation: String) -> Result<(), String> {
    with_engine(&engine, move |app| app.cancel_query(&continuation)).await
}

#[tauri::command]
async fn path_graph_revision(engine: State<'_, Engine>) -> Result<String, String> {
    with_engine(&engine, |app| app.path_graph_revision()).await
}

#[tauri::command]
async fn explore_paths(
    engine: State<'_, Engine>,
    request: PathExploreRequest,
) -> Result<PathExploreResponse, String> {
    with_engine(&engine, move |app| app.explore_paths(request)).await
}

#[tauri::command]
async fn explore_path_details(
    engine: State<'_, Engine>,
    request: PathDetailsRequest,
) -> Result<PathDetailsResponse, String> {
    with_engine(&engine, move |app| app.explore_path_details(request)).await
}

#[tauri::command]
async fn open_original(engine: State<'_, Engine>, source_id: String) -> Result<(), String> {
    with_engine(&engine, move |app| {
        let original = app.original_path(&source_id)?;
        open_retained_original(&original)
    })
    .await
}

#[tauri::command]
async fn preview_original(engine: State<'_, Engine>, source_id: String) -> Result<String, String> {
    with_engine(&engine, move |app| app.preview_original(&source_id)).await
}

#[tauri::command]
async fn preview_video_frame(
    engine: State<'_, Engine>,
    source_id: String,
    source_version_id: String,
    asset: String,
    at_ms: u64,
) -> Result<crate::media::VideoPreview, String> {
    with_engine(&engine, move |app| {
        app.preview_video_frame(&source_id, &source_version_id, &asset, at_ms)
    })
    .await
}

#[tauri::command]
async fn open_original_version(
    engine: State<'_, Engine>,
    source_id: String,
    source_version_id: String,
    asset: String,
) -> Result<(), String> {
    with_engine(&engine, move |app| {
        let original = app.original_version_path(&source_id, &source_version_id, &asset)?;
        open_retained_original(&original)
    })
    .await
}

#[tauri::command]
async fn open_original_asset(
    engine: State<'_, Engine>,
    source_id: String,
    asset: String,
) -> Result<(), String> {
    with_engine(&engine, move |app| {
        let original = app.original_asset_path(&source_id, &asset)?;
        open_retained_original(&original)
    })
    .await
}

fn open_retained_original(original: &Path) -> crate::application::Result<()> {
    let mut command = std::process::Command::new("/usr/bin/open");
    if original
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|format| {
            crate::application::is_timed_media_format(&format.to_ascii_lowercase())
        })
    {
        // Open recordings directly, without importing them into a Music library
        // or depending on its onboarding and cloud-library configuration.
        command.args(["-a", "QuickTime Player"]);
    }
    if !command.arg(original).status()?.success() {
        return Err(crate::application::GardenError::Invalid(
            "macOS could not open the retained original.".into(),
        ));
    }
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let root = std::env::var_os("KNOWLEDGE_GARDEN_COLLECTION")
                .map(PathBuf::from)
                .unwrap_or(app.path().app_data_dir()?.join("collection"));
            let semantic_provider: Arc<dyn SemanticProvider> =
                Arc::new(JevSemanticProvider::from_environment_and_keychain());
            let meaning_assets = app.path().resource_dir()?.join("meaning");
            let audio_processor = Arc::new(WhisperAudioProcessor::new(
                app.path().resource_dir()?.join("audio"),
            ));
            let engine = Arc::new(Mutex::new(Application::open_with_all_providers_and_meaning_assets(
                root,
                Arc::clone(&semantic_provider),
                Arc::new(LocalExtractor),
                audio_processor,
                Some(meaning_assets),
            )?));
            let worker_app = Arc::clone(&engine);
            let worker_provider = Arc::clone(&semantic_provider);
            std::thread::Builder::new()
                .name("knowledge-garden-semantic-queue".into())
                .spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    if let Ok(mut app) = worker_app.lock() {
                        if let Err(error) = app.reconcile_collection(256) {
                            eprintln!("Collection reconciliation will retry: {error}");
                        }
                        if let Err(error) = app.reconcile_watched_directories(8) {
                            eprintln!("Directory watcher could not reconcile: {error}");
                        }
                        if let Err(error) = app.resume_due_url_acquisitions() {
                            eprintln!("URL acquisition queue could not resume work: {error}");
                        }
                        if let Err(error) = app.resume_due_audio_jobs() {
                            eprintln!("Audio transcription queue could not resume work: {error}");
                        }
                    }
                    let jobs = match worker_app.lock() {
                        Ok(mut app) => match app.claim_due_semantic_jobs(2) {
                            Ok(jobs) => jobs,
                            Err(error) => {
                                eprintln!("Semantic queue could not claim work: {error}");
                                continue;
                            }
                        },
                        Err(_) => {
                            eprintln!("Semantic queue could not access the collection.");
                            continue;
                        }
                    };
                    for job in jobs {
                        let result = worker_provider.form_knowledge_with_prior(
                            &job.source_text,
                            job.prior_source_text.as_deref(),
                        );
                        match worker_app.lock() {
                            Ok(mut app) => {
                                if let Err(error) = app.finish_semantic_job(job, result) {
                                    eprintln!("Semantic queue could not publish a result: {error}");
                                }
                            }
                            Err(_) => eprintln!("Semantic queue could not access the collection to publish a result."),
                        }
                    }
                })?;
            app.manage(Engine { app: engine });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_collection_file,
            collection_status,
            inspect_gaps,
            collection_groups,
            comparison_side,
            open_comparison_original,
            collection_members,
            resolve_page_link,
            import_source,
            import_repository,
            watch_directory,
            list_watched_directories,
            import_url,
            list_url_acquisitions,
            open_source,
            retained_source_exists,
            open_knowledge_page,
            list_sources,
            search_pages,
            query_pages,
            cancel_query,
            path_graph_revision,
            explore_paths,
            explore_path_details,
            open_original,
            preview_original,
            preview_video_frame,
            open_original_version,
            open_original_asset
        ])
        .run(tauri::generate_context!())
        .expect("The Knowledge Garden application could not start");
}
