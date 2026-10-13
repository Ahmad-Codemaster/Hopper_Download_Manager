use std::io::{self, Read, Write};
use crate::ipc::{decode, encode, CommandEnvelope, Envelope};
use crate::routing::route;
use download_core::engine::Engine;
use tokio::runtime::Runtime;

pub fn read_message<R: Read>(reader: &mut R) -> io::Result<Vec<u8>> {
    let mut length = [0; 4];
    reader.read_exact(&mut length)?;
    let size = u32::from_le_bytes(length) as usize;
    if size > 64 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "message too large"));
    }
    let mut data = vec![0; size];
    reader.read_exact(&mut data)?;
    Ok(data)
}

pub fn write_message<W: Write>(writer: &mut W, data: &[u8]) -> io::Result<()> {
    if data.len() > 64 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "message too large"));
    }
    writer.write_all(&(data.len() as u32).to_le_bytes())?;
    writer.write_all(data)
}

pub fn process_stream<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> io::Result<()> {
    loop {
        let data = match read_message(reader) {
            Ok(data) => data,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error),
        };
        let response = match decode(&data) {
            Ok(envelope) => envelope,
            Err(error) => Envelope::new(&format!("error: {error}")),
        };
        let encoded = encode(&response)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        write_message(writer, &encoded)?;
    }

    Ok(())
}

pub fn process_engine_stream<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    engine: &Engine,
    runtime: &Runtime,
) -> io::Result<()> {
    loop {
        let data = match read_message(reader) {
            Ok(data) => data,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error),
        };
        let response = match CommandEnvelope::decode(&data) {
            Ok(command) => runtime.block_on(route(engine, command)),
            Err(error) => crate::ipc::ResponseEnvelope::error(error.to_string()),
        };
        let encoded = serde_json::to_vec(&response)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        write_message(writer, &encoded)?;
    }
    Ok(())
}
