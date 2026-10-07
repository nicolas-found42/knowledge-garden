use crate::{
    application::{
        AcquisitionMethod, Application, PageSearchRequest, PageSearchResults, SourceList,
        SourcePage, UrlAcquisitionStatus,
    },
    providers::JevSemanticProvider,
    semantic::{KnowledgePage, SemanticProvider},
};
use std::{
    path::PathBuf,
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
async fn import_source(
    engine: State<'_, Engine>,
    path: String,
    method: AcquisitionMethod,
) -> Result<SourcePage, String> {
    with_engine(&engine, move |app| app.import_source(path, method)).await
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
async fn open_original(engine: State<'_, Engine>, source_id: String) -> Result<(), String> {
    with_engine(&engine, move |app| {
        let original = app.original_path(&source_id)?;
        let status = std::process::Command::new("/usr/bin/open")
            .arg(original)
            .status()?;
        if !status.success() {
            return Err(crate::application::GardenError::Invalid(
                "macOS could not open the retained original.".into(),
            ));
        }
        Ok(())
    })
    .await
}

#[tauri::command]
async fn preview_original(engine: State<'_, Engine>, source_id: String) -> Result<String, String> {
    with_engine(&engine, move |app| app.preview_original(&source_id)).await
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
        let status = std::process::Command::new("/usr/bin/open")
            .arg(original)
            .status()?;
        if !status.success() {
            return Err(crate::application::GardenError::Invalid(
                "macOS could not open the retained original version.".into(),
            ));
        }
        Ok(())
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
        let status = std::process::Command::new("/usr/bin/open")
            .arg(original)
            .status()?;
        if !status.success() {
            return Err(crate::application::GardenError::Invalid(
                "macOS could not open the retained original asset.".into(),
            ));
        }
        Ok(())
    })
    .await
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
            let engine = Arc::new(Mutex::new(Application::open_with_meaning_assets(
                root,
                Some(meaning_assets),
            )?));
            let worker_app = Arc::clone(&engine);
            let worker_provider = Arc::clone(&semantic_provider);
            std::thread::Builder::new()
                .name("knowledge-garden-semantic-queue".into())
                .spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    if let Ok(mut app) = worker_app.lock() {
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
            import_source,
            import_url,
            list_url_acquisitions,
            open_source,
            open_knowledge_page,
            list_sources,
            search_pages,
            open_original,
            preview_original,
            open_original_version,
            open_original_asset
        ])
        .run(tauri::generate_context!())
        .expect("The Knowledge Garden application could not start");
}
