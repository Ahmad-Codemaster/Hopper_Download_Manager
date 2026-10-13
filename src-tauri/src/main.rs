use download_core::engine::Engine;
use download_core::model::{ConflictPolicy, JobId, JobSpec, RequestContext};
use std::path::PathBuf;
use std::sync::Arc;
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
    state.engine.lock().await.create_job(JobSpec {
        source,
        destination: PathBuf::from(destination),
        request_context: RequestContext::default(),
        conflict_policy: ConflictPolicy::Rename,
    }).await.map(|id| id.0).map_err(|error| error.to_string())
}

#[tauri::command]
async fn start_job(job_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.engine.lock().await.start_job(JobId(job_id)).await.map_err(|error| error.to_string())
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

fn main() {
    let root = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().expect("current directory"));
    let engine = Engine::new(root.join("HopperDownloadManager")).expect("engine initialization");
    tauri::Builder::default()
        .manage(AppState { engine: Arc::new(Mutex::new(engine)) })
        .invoke_handler(tauri::generate_handler![list_jobs, create_job, start_job, pause_job, resume_job, cancel_job])
        .run(tauri::generate_context!())
        .expect("error while running Hopper");
}
