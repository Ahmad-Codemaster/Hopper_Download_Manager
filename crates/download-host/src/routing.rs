use crate::ipc::{Command, CommandEnvelope, ResponseEnvelope};
use download_core::engine::Engine;
use download_core::model::{ConflictPolicy, JobSpec, RequestContext};
use std::path::PathBuf;

pub async fn route(engine: &Engine, envelope: CommandEnvelope) -> ResponseEnvelope {
    if envelope.major != 1 {
        return ResponseEnvelope::error("unsupported protocol version");
    }
    let result: Result<serde_json::Value, String> = match envelope.command {
        Command::List => engine.list_jobs().await
            .map(|jobs| serde_json::to_value(jobs).map_err(|error| error.to_string()))
            .map_err(|error| error.to_string())
            .and_then(|value| value),
        Command::Create { source, destination } => {
            match source.parse() {
                Ok(source) => engine.create_job(JobSpec {
                    source,
                    destination: PathBuf::from(destination),
                    request_context: RequestContext::default(),
                    conflict_policy: ConflictPolicy::Rename,
                }).await.map(|id| serde_json::json!({ "job_id": id.0 })).map_err(|error| error.to_string()),
                Err(_) => Err("invalid URL".into()),
            }
        }
        Command::Start { job_id } => engine.start_job(download_core::model::JobId(job_id)).await.map(|_| serde_json::Value::Null).map_err(|error| error.to_string()),
        Command::Pause { job_id } => engine.pause_job(download_core::model::JobId(job_id)).await.map(|_| serde_json::Value::Null).map_err(|error| error.to_string()),
        Command::Resume { job_id } => engine.resume_job(download_core::model::JobId(job_id)).await.map(|_| serde_json::Value::Null).map_err(|error| error.to_string()),
        Command::Cancel { job_id } => engine.cancel_job(download_core::model::JobId(job_id)).await.map(|_| serde_json::Value::Null).map_err(|error| error.to_string()),
        Command::Retry { job_id } => engine.retry_job(download_core::model::JobId(job_id)).await.map(|_| serde_json::Value::Null).map_err(|error| error.to_string()),
    };
    match result {
        Ok(payload) => ResponseEnvelope::success(payload),
        Err(error) => ResponseEnvelope::error(error),
    }
}
