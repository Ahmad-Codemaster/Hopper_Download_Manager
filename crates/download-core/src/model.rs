use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub String);

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestContext {
    #[serde(skip)]
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictPolicy {
    Reject,
    Rename,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobSpec {
    pub source: Url,
    pub destination: PathBuf,
    #[serde(skip)]
    pub request_context: RequestContext,
    pub conflict_policy: ConflictPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    Queued,
    Active,
    Paused,
    Failed,
    Completed,
    Canceled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobCommand {
    Start(JobId),
    Pause(JobId),
    Resume(JobId),
    Cancel(JobId),
    Retry(JobId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobEvent {
    pub job_id: JobId,
    pub state: JobState,
    pub completed_bytes: u64,
    pub total_bytes: Option<u64>,
    #[serde(default)]
    pub speed_bytes_per_sec: u64,
    pub retry_count: u32,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EngineError {
    #[error("only http and https URLs are supported")]
    UnsupportedScheme,
    #[error("destination must be a relative path without traversal")]
    UnsafeDestination,
    #[error("request context is too large")]
    RequestContextTooLarge,
    #[error("existing destination requires an explicit conflict policy")]
    DestinationExists,
    #[error("invalid job: {0}")]
    InvalidJob(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub start: u64,
    pub end: u64,
    pub completed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub segments: Vec<Segment>,
}
