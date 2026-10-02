use crate::application::{AcquisitionMethod, Application, SourceList, SourcePage};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{Manager, State};

struct Engine(Arc<Mutex<Application>>);

async fn with_engine<T: Send + 'static>(
    engine: &Engine,
    operation: impl FnOnce(&mut Application) -> crate::application::Result<T> + Send + 'static,
) -> Result<T, String> {
    let engine = Arc::clone(&engine.0);
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
async fn open_source(engine: State<'_, Engine>, source_id: String) -> Result<SourcePage, String> {
    with_engine(&engine, move |app| app.open_source(&source_id)).await
}

#[tauri::command]
async fn list_sources(engine: State<'_, Engine>, offset: usize) -> Result<SourceList, String> {
    with_engine(&engine, move |app| app.list_sources(offset)).await
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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let root = std::env::var_os("KNOWLEDGE_GARDEN_COLLECTION")
                .map(PathBuf::from)
                .unwrap_or(app.path().app_data_dir()?.join("collection"));
            app.manage(Engine(Arc::new(Mutex::new(Application::open(root)?))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            import_source,
            open_source,
            list_sources,
            open_original
        ])
        .run(tauri::generate_context!())
        .expect("The Knowledge Garden application could not start");
}
