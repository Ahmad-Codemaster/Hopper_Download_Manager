#![windows_subsystem = "windows"]

use download_core::engine::Engine;
use download_core::model::{ConflictPolicy, JobId, JobSpec, RequestContext};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::Mutex;

struct AppState {
    engine: Arc<Mutex<Engine>>,
}

#[tauri::command]
async fn list_jobs(state: tauri::State<'_, AppState>) -> Result<Vec<download_core::store::StoredJob>, String> {
    state.engine.lock().await.list_jobs().await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn create_job(source: String, destination: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let source = source.parse().map_err(|_| "invalid HTTP(S) URL".to_string())?;
    state
        .engine
        .lock()
        .await
        .create_job(JobSpec {
            source,
            destination: PathBuf::from(destination),
            request_context: RequestContext::default(),
            conflict_policy: ConflictPolicy::Rename,
        })
        .await
        .map(|id| id.0)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn start_job(job_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    // Engine::start_job spawns the transfer as a background task (CRIT-01 fix)
    // and returns immediately, so the mutex is held only briefly here.
    state.engine.lock().await.start_job(JobId(job_id)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn pause_job(job_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.engine.lock().await.pause_job(JobId(job_id)).await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn resume_job(job_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.engine.lock().await.resume_job(JobId(job_id)).await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn cancel_job(job_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.engine.lock().await.cancel_job(JobId(job_id)).await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn retry_job(job_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.engine.lock().await.retry_job(JobId(job_id)).await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn refresh_source(job_id: String, new_source: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let source = new_source.parse().map_err(|_| "invalid HTTP(S) URL".to_string())?;
    state
        .engine
        .lock()
        .await
        .refresh_source(JobId(job_id), source)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn set_speed_limit(limit_bytes_per_sec: u64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.engine.lock().await.set_speed_limit(limit_bytes_per_sec).map_err(|error| error.to_string())
}

#[tauri::command]
async fn get_speed_limit(state: tauri::State<'_, AppState>) -> Result<u64, String> {
    state.engine.lock().await.get_speed_limit().map_err(|error| error.to_string())
}

#[tauri::command]
async fn set_max_concurrent(max_concurrent: usize, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.engine.lock().await.set_max_concurrent_jobs(max_concurrent).map_err(|error| error.to_string())
}

#[tauri::command]
async fn get_max_concurrent(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    state.engine.lock().await.get_max_concurrent_jobs().map_err(|error| error.to_string())
}

#[tauri::command]
async fn preview_zip_archive(url: String, total_bytes: u64, state: tauri::State<'_, AppState>) -> Result<Vec<download_core::archive::ZipEntryPreview>, String> {
    state.engine.lock().await.preview_zip_archive(&url, total_bytes).await.map_err(|error| error.to_string())
}

fn main() {
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().expect("current directory"));
    let engine = Engine::new(root.join("HopperDownloadManager")).expect("engine initialization");
    let app_engine = Arc::new(Mutex::new(engine));
    let engine_events = Arc::clone(&app_engine);

    tauri::Builder::default()
        .manage(AppState {
            engine: Arc::clone(&app_engine),
        })
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let rx_res = {
                    let guard = engine_events.lock().await;
                    guard.subscribe_events()
                };
                if let Ok(mut rx) = rx_res {
                    while let Ok(event) = rx.recv().await {
                        let _ = handle.emit("download_progress", &event);
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_jobs,
            create_job,
            start_job,
            pause_job,
            resume_job,
            cancel_job,
            retry_job,
            refresh_source,
            set_speed_limit,
            get_speed_limit,
            set_max_concurrent,
            get_max_concurrent,
            preview_zip_archive
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hopper");
}
