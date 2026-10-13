use crate::model::{ConflictPolicy, EngineError, JobId, JobSpec, JobState};
use crate::validation::redact_url;
use rusqlite::{params, Connection};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredJob {
    pub id: JobId,
    pub source: String,
    pub destination: String,
    pub state: JobState,
    pub completed_bytes: u64,
    pub conflict_policy: ConflictPolicy,
}

pub struct JobStore {
    connection: Connection,
}

impl JobStore {
    pub fn open(path: &Path) -> Result<Self, EngineError> {
        let connection = Connection::open(path)
            .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS jobs (
                    id TEXT PRIMARY KEY,
                    source TEXT NOT NULL,
                    source_key TEXT NOT NULL,
                    destination TEXT NOT NULL,
                    state TEXT NOT NULL,
                    completed_bytes INTEGER NOT NULL DEFAULT 0,
                    conflict_policy TEXT NOT NULL
                );",
            )
            .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
        Ok(Self { connection })
    }

    pub fn create_job(&self, spec: &JobSpec) -> Result<JobId, EngineError> {
        let source = redact_url(&spec.source);
        let source_key = format!("{:x}", {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            spec.source.as_str().hash(&mut hasher);
            hasher.finish()
        });
        let destination = spec.destination.to_string_lossy().to_string();
        let duplicate: bool = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM jobs WHERE source_key = ?1 AND destination = ?2)",
                params![source_key, destination],
                |row| row.get(0),
            )
            .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
        if duplicate {
            return Err(EngineError::InvalidJob("duplicate source and destination".into()));
        }
        let id = JobId(uuid::Uuid::new_v4().to_string());
        let conflict_policy = match spec.conflict_policy {
            ConflictPolicy::Reject => "reject",
            ConflictPolicy::Rename => "rename",
        };
        self.connection
            .execute(
                "INSERT INTO jobs (id, source, source_key, destination, state, conflict_policy) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    id.0,
                    source,
                    source_key,
                    destination,
                    "queued",
                    conflict_policy
                ],
            )
            .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
        Ok(id)
    }

    pub fn update_checkpoint(&self, id: JobId, completed_bytes: u64) -> Result<(), EngineError> {
        self.connection
            .execute(
                "UPDATE jobs SET completed_bytes = ?1 WHERE id = ?2",
                params![completed_bytes, id.0],
            )
            .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
        Ok(())
    }

    pub fn get_job(&self, id: JobId) -> Result<StoredJob, EngineError> {
        self.connection
            .query_row(
                "SELECT id, source, destination, state, completed_bytes, conflict_policy FROM jobs WHERE id = ?1",
                params![id.0],
                |row| {
                    let state: String = row.get(3)?;
                    let conflict_policy: String = row.get(5)?;
                    Ok(StoredJob {
                        id: JobId(row.get(0)?),
                        source: row.get(1)?,
                        destination: row.get(2)?,
                        state: match state.as_str() {
                            "completed" => JobState::Completed,
                            "failed" => JobState::Failed,
                            _ => JobState::Queued,
                        },
                        completed_bytes: row.get(4)?,
                        conflict_policy: if conflict_policy == "rename" {
                            ConflictPolicy::Rename
                        } else {
                            ConflictPolicy::Reject
                        },
                    })
                },
            )
            .map_err(|error| EngineError::InvalidJob(error.to_string()))
    }

    pub fn set_state(&self, id: JobId, state: JobState) -> Result<(), EngineError> {
        let value = match state {
            JobState::Active => "active",
            JobState::Paused => "paused",
            JobState::Failed => "failed",
            JobState::Completed => "completed",
            JobState::Canceled => "canceled",
            JobState::Queued => "queued",
        };
        self.connection
            .execute(
                "UPDATE jobs SET state = ?1 WHERE id = ?2",
                params![value, id.0],
            )
            .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
        Ok(())
    }
}
