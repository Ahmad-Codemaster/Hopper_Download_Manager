use crate::model::EngineError;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZipEntryPreview {
    pub name: String,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub is_dir: bool,
}

const EOCD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
const CD_HEADER_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];

/// Parses Central Directory entries from a byte slice containing the Central Directory.
pub fn parse_central_directory(data: &[u8]) -> Vec<ZipEntryPreview> {
    let mut entries = Vec::new();
    let mut cursor = 0;

    while cursor + 46 <= data.len() {
        if data[cursor..cursor + 4] != CD_HEADER_SIGNATURE {
            break;
        }

        let compressed_size = u32::from_le_bytes(
            data[cursor + 20..cursor + 24].try_into().unwrap_or_default(),
        ) as u64;
        let uncompressed_size = u32::from_le_bytes(
            data[cursor + 24..cursor + 28].try_into().unwrap_or_default(),
        ) as u64;
        let filename_len = u16::from_le_bytes(
            data[cursor + 28..cursor + 30].try_into().unwrap_or_default(),
        ) as usize;
        let extra_len = u16::from_le_bytes(
            data[cursor + 30..cursor + 32].try_into().unwrap_or_default(),
        ) as usize;
        let comment_len = u16::from_le_bytes(
            data[cursor + 32..cursor + 34].try_into().unwrap_or_default(),
        ) as usize;

        let name_start = cursor + 46;
        let name_end = name_start + filename_len;

        if name_end <= data.len() {
            let name = String::from_utf8_lossy(&data[name_start..name_end]).to_string();
            let is_dir = name.ends_with('/');
            entries.push(ZipEntryPreview {
                name,
                uncompressed_size,
                compressed_size,
                is_dir,
            });
        }

        cursor = name_end + extra_len + comment_len;
    }

    entries
}

/// Locates the EOCD record in a trailing byte buffer of a ZIP file.
pub fn find_eocd(buffer: &[u8]) -> Option<(usize, u32, u32, u16)> {
    if buffer.len() < 22 {
        return None;
    }

    // Scan backwards from the end for the EOCD signature
    for i in (0..=buffer.len() - 22).rev() {
        if buffer[i..i + 4] == EOCD_SIGNATURE {
            let total_entries = u16::from_le_bytes(
                buffer[i + 10..i + 12].try_into().unwrap_or_default(),
            );
            let cd_size = u32::from_le_bytes(
                buffer[i + 12..i + 16].try_into().unwrap_or_default(),
            );
            let cd_offset = u32::from_le_bytes(
                buffer[i + 16..i + 20].try_into().unwrap_or_default(),
            );
            return Some((i, cd_size, cd_offset, total_entries));
        }
    }
    None
}

/// Fast targeted range preview for remote ZIP archives without downloading the full file.
pub async fn preview_remote_zip(
    client: &Client,
    url: &str,
    total_bytes: u64,
) -> Result<Vec<ZipEntryPreview>, EngineError> {
    if total_bytes < 22 {
        return Err(EngineError::InvalidJob("file too small to be a zip archive".into()));
    }

    let tail_size = 65536u64.min(total_bytes);
    let tail_start = total_bytes - tail_size;
    let range_header = format!("bytes={tail_start}-{}", total_bytes - 1);

    let resp = client
        .get(url)
        .header("Range", range_header)
        .send()
        .await
        .map_err(|e| EngineError::InvalidJob(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(EngineError::InvalidJob("remote server does not support range requests for zip preview".into()));
    }

    let tail_bytes = resp
        .bytes()
        .await
        .map_err(|e| EngineError::InvalidJob(e.to_string()))?;

    let (_eocd_pos, cd_size, cd_offset, _entries_count) = find_eocd(&tail_bytes)
        .ok_or_else(|| EngineError::InvalidJob("end of central directory not found in archive tail".into()))?;

    // Check if the entire Central Directory is already contained in the tail buffer
    if (cd_offset as u64) >= tail_start && (cd_offset as u64 + cd_size as u64) <= total_bytes {
        let local_offset = (cd_offset as u64 - tail_start) as usize;
        let cd_data = &tail_bytes[local_offset..local_offset + (cd_size as usize)];
        return Ok(parse_central_directory(cd_data));
    }

    // Otherwise, fetch precisely the central directory range
    let cd_range = format!("bytes={cd_offset}-{}", (cd_offset + cd_size).saturating_sub(1));
    let cd_resp = client
        .get(url)
        .header("Range", cd_range)
        .send()
        .await
        .map_err(|e| EngineError::InvalidJob(e.to_string()))?;

    let cd_bytes = cd_resp
        .bytes()
        .await
        .map_err(|e| EngineError::InvalidJob(e.to_string()))?;

    Ok(parse_central_directory(&cd_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_zip_central_directory_bytes() {
        // Construct a synthetic Central Directory record
        let mut cd = Vec::new();
        cd.extend_from_slice(&CD_HEADER_SIGNATURE);
        cd.extend_from_slice(&[0; 16]); // version, flags, method, time, date, crc
        cd.extend_from_slice(&120u32.to_le_bytes()); // compressed size: 120
        cd.extend_from_slice(&250u32.to_le_bytes()); // uncompressed size: 250
        let filename = b"documents/report.pdf";
        cd.extend_from_slice(&(filename.len() as u16).to_le_bytes()); // filename len
        cd.extend_from_slice(&0u16.to_le_bytes()); // extra len
        cd.extend_from_slice(&0u16.to_le_bytes()); // comment len
        cd.extend_from_slice(&[0; 12]); // disk, attrs, offset
        cd.extend_from_slice(filename);

        let entries = parse_central_directory(&cd);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "documents/report.pdf");
        assert_eq!(entries[0].compressed_size, 120);
        assert_eq!(entries[0].uncompressed_size, 250);
        assert!(!entries[0].is_dir);
    }
}
