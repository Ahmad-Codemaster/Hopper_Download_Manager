use crate::model::{ConflictPolicy, EngineError, JobId, JobSpec, JobState, Segment};
use crate::validation::redact_url;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StoredJob {
    pub id: JobId,
    pub source: String,
    pub destination: String,
    pub state: JobState,
    pub completed_bytes: u64,
    pub total_bytes: Option<u64>,
    pub conflict_policy: ConflictPolicy,
}

pub struct JobStore {
    connection: Mutex<Connection>,
    root: std::path::PathBuf,
}

impl JobStore {
    fn conn(&self) -> Result<MutexGuard<'_, Connection>, EngineError> {
        self.connection
            .lock()
            .map_err(|_| EngineError::InvalidJob("store lock poisoned".into()))
    }

    pub fn open(path: &Path) -> Result<Self, EngineError> {
        let database_path = if path.is_dir() {
            path.join("jobs.db")
        } else {
            path.to_path_buf()
        };
        let connection = Connection::open(&database_path)
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
                    total_bytes INTEGER,
                    conflict_policy TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS segments (
                    job_id TEXT NOT NULL,
                    segment_index INTEGER NOT NULL,
                    start_byte INTEGER NOT NULL,
                    end_byte INTEGER NOT NULL,
                    completed_byte INTEGER NOT NULL,
                    PRIMARY KEY (job_id, segment_index)
                );",
            )
            .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
        let _ = connection.execute("ALTER TABLE jobs ADD COLUMN total_bytes INTEGER", []);
        let root = database_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        Ok(Self {
            connection: Mutex::new(connection),
            root,
        })
    }

    pub fn create_job(&self, spec: &JobSpec) -> Result<JobId, EngineError> {
        let conn = self.conn()?;
        let source = redact_url(&spec.source);
        let source_key = format!("{:x}", {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            spec.source.as_str().hash(&mut hasher);
            hasher.finish()
        });
        let destination = spec.destination.to_string_lossy().to_string();
        let duplicate: bool = conn
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
        conn.execute(
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
        let conn = self.conn()?;
        conn.execute(
            "UPDATE jobs SET completed_bytes = ?1 WHERE id = ?2",
            params![completed_bytes, id.0],
        )
        .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
        Ok(())
    }

    pub fn get_job(&self, id: JobId) -> Result<StoredJob, EngineError> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, source, destination, state, completed_bytes, conflict_policy, total_bytes FROM jobs WHERE id = ?1",
            params![id.0],
            |row| {
                let state: String = row.get(3)?;
                let conflict_policy: String = row.get(5)?;
                let total_bytes: Option<i64> = row.get(6)?;
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
                    total_bytes: total_bytes.map(|v| v as u64),
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

    pub fn list_jobs(&self) -> Result<Vec<StoredJob>, EngineError> {
        let conn = self.conn()?;
        let mut statement = conn
            .prepare("SELECT id, source, destination, state, completed_bytes, conflict_policy, total_bytes FROM jobs ORDER BY id")
            .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
        let rows = statement
            .query_map([], |row| {
                let state: String = row.get(3)?;
                let policy: String = row.get(5)?;
                let destination: String = row.get(2)?;
                let total_bytes: Option<i64> = row.get(6)?;
                let mut state = match state.as_str() {
                    "completed" => JobState::Completed,
                    "failed" => JobState::Failed,
                    "active" => JobState::Active,
                    "paused" => JobState::Paused,
                    "canceled" => JobState::Canceled,
                    _ => JobState::Queued,
                };
                if state == JobState::Completed && !self.root.join(&destination).exists() {
                    state = JobState::Failed;
                }
                Ok(StoredJob {
                    id: JobId(row.get(0)?),
                    source: row.get(1)?,
                    destination,
                    state,
                    completed_bytes: row.get(4)?,
                    total_bytes: total_bytes.map(|v| v as u64),
                    conflict_policy: if policy == "rename" {
                        ConflictPolicy::Rename
                    } else {
                        ConflictPolicy::Reject
                    },
                })
            })
            .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
        rows.map(|row| row.map_err(|error| EngineError::InvalidJob(error.to_string())))
            .collect()
    }

    pub fn set_state(&self, id: JobId, state: JobState) -> Result<(), EngineError> {
        let conn = self.conn()?;
        let value = match state {
            JobState::Active => "active",
            JobState::Paused => "paused",
            JobState::Failed => "failed",
            JobState::Completed => "completed",
            JobState::Canceled => "canceled",
            JobState::Queued => "queued",
        };
        conn.execute(
            "UPDATE jobs SET state = ?1 WHERE id = ?2",
            params![value, id.0],
        )
        .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
        Ok(())
    }

    pub fn update_total_bytes(&self, id: &JobId, total_bytes: u64) -> Result<(), EngineError> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE jobs SET total_bytes = ?1 WHERE id = ?2",
            params![total_bytes as i64, id.0],
        )
        .map_err(|e| EngineError::InvalidJob(e.to_string()))?;
        Ok(())
    }

    pub fn update_source(&self, id: &JobId, new_source: &url::Url) -> Result<(), EngineError> {
        let conn = self.conn()?;
        let source = redact_url(new_source);
        let source_key = format!("{:x}", {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            new_source.as_str().hash(&mut hasher);
            hasher.finish()
        });
        conn.execute(
            "UPDATE jobs SET source = ?1, source_key = ?2 WHERE id = ?3",
            params![source, source_key, id.0],
        )
        .map_err(|e| EngineError::InvalidJob(e.to_string()))?;
        Ok(())
    }

    pub fn save_segments(&self, id: &JobId, segments: &[Segment]) -> Result<(), EngineError> {
        let mut conn = self.conn()?;
        let tx = conn
            .transaction()
            .map_err(|e| EngineError::InvalidJob(e.to_string()))?;
        tx.execute("DELETE FROM segments WHERE job_id = ?1", params![id.0])
            .map_err(|e| EngineError::InvalidJob(e.to_string()))?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO segments (job_id, segment_index, start_byte, end_byte, completed_byte) VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .map_err(|e| EngineError::InvalidJob(e.to_string()))?;
            for (idx, seg) in segments.iter().enumerate() {
                stmt.execute(params![
                    id.0,
                    idx as i64,
                    seg.start as i64,
                    seg.end as i64,
                    seg.completed as i64
                ])
                .map_err(|e| EngineError::InvalidJob(e.to_string()))?;
            }
        }
        tx.commit()
            .map_err(|e| EngineError::InvalidJob(e.to_string()))?;
        Ok(())
    }

    pub fn load_segments(&self, id: &JobId) -> Result<Vec<Segment>, EngineError> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT start_byte, end_byte, completed_byte FROM segments WHERE job_id = ?1 ORDER BY segment_index",
            )
            .map_err(|e| EngineError::InvalidJob(e.to_string()))?;
        let rows = stmt
            .query_map(params![id.0], |row| {
                let start: i64 = row.get(0)?;
                let end: i64 = row.get(1)?;
                let completed: i64 = row.get(2)?;
                Ok(Segment {
                    start: start as u64,
                    end: end as u64,
                    completed: completed as u64,
                })
            })
            .map_err(|e| EngineError::InvalidJob(e.to_string()))?;
        rows.map(|r| r.map_err(|e| EngineError::InvalidJob(e.to_string())))
            .collect()
    }
}
