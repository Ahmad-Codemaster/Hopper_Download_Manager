use serde::{Deserialize, Serialize};

const MAX_MESSAGE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    List,
    Create { source: String, destination: String },
    Start { job_id: String },
    Pause { job_id: String },
    Resume { job_id: String },
    Cancel { job_id: String },
    Retry { job_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEnvelope { pub major: u16, pub minor: u16, pub command: Command }

impl CommandEnvelope {
    pub fn list() -> Self { Self { major: 1, minor: 0, command: Command::List } }
    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> { serde_json::to_vec(self) }
    pub fn decode(data: &[u8]) -> Result<Self, serde_json::Error> { serde_json::from_slice(data) }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub major: u16,
    pub minor: u16,
    pub ok: bool,
    pub payload: serde_json::Value,
}

impl ResponseEnvelope {
    pub fn success(payload: serde_json::Value) -> Self {
        Self { major: 1, minor: 0, ok: true, payload }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self { major: 1, minor: 0, ok: false, payload: serde_json::json!({ "error": message.into() }) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    pub major: u16,
    pub minor: u16,
    pub payload: String,
}

impl Envelope {
    pub fn new(payload: &str) -> Self {
        Self { major: 1, minor: 0, payload: payload.into() }
    }
}

pub fn encode(envelope: &Envelope) -> Result<Vec<u8>, serde_json::Error> {
    let data = serde_json::to_vec(envelope)?;
    if data.len() > MAX_MESSAGE_BYTES {
        return Err(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "message too large",
        )));
    }
    Ok(data)
}

pub fn decode(data: &[u8]) -> Result<Envelope, serde_json::Error> {
    if data.len() > MAX_MESSAGE_BYTES {
        return Err(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "message too large",
        )));
    }
    serde_json::from_slice(data)
}
