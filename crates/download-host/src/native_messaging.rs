use std::io::{self, Read, Write};

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
