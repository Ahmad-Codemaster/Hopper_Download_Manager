use serde::{Deserialize, Serialize};

const MAX_MESSAGE_BYTES: usize = 64 * 1024;

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
