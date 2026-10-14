use crate::model::{EngineError, Segment};
use std::fs::File;
use std::sync::Arc;

pub fn validate_content_range(
    value: &str,
    expected_start: u64,
    expected_end: u64,
    expected_total: u64,
) -> Result<(), EngineError> {
    let (unit, range) = value
        .split_once(' ')
        .ok_or_else(|| EngineError::InvalidJob("malformed Content-Range".into()))?;
    if unit != "bytes" {
        return Err(EngineError::InvalidJob("unsupported Content-Range unit".into()));
    }
    let (bounds, total) = range
        .split_once('/')
        .ok_or_else(|| EngineError::InvalidJob("malformed Content-Range".into()))?;
    let (start, end) = bounds
        .split_once('-')
        .ok_or_else(|| EngineError::InvalidJob("malformed Content-Range bounds".into()))?;
    let parsed = (
        start.parse::<u64>(),
        end.parse::<u64>(),
        total.parse::<u64>(),
    );
    if parsed == (Ok(expected_start), Ok(expected_end), Ok(expected_total)) {
        Ok(())
    } else {
        Err(EngineError::InvalidJob("unexpected Content-Range".into()))
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResourceProbe {
    pub supports_ranges: bool,
    pub content_length: Option<u64>,
    pub filename: Option<String>,
}

pub async fn probe_url(client: &reqwest::Client, url: &str) -> ResourceProbe {
    // 1. Try HEAD request
    if let Ok(resp) = client.head(url).send().await {
        if resp.status().is_success() {
            let accept_ranges = resp
                .headers()
                .get("accept-ranges")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.eq_ignore_ascii_case("bytes"))
                .unwrap_or(false);
            let content_len = resp.content_length();
            let filename = extract_filename(&resp);
            if content_len.is_some() || accept_ranges {
                return ResourceProbe {
                    supports_ranges: accept_ranges,
                    content_length: content_len,
                    filename,
                };
            }
        }
    }

    // 2. Fallback: Range probe request bytes=0-0
    if let Ok(resp) = client.get(url).header("Range", "bytes=0-0").send().await {
        let supports_ranges = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        let content_len = if supports_ranges {
            resp.headers()
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .and_then(|cr| cr.split('/').next_back())
                .and_then(|total| total.parse::<u64>().ok())
        } else {
            resp.content_length()
        };
        let filename = extract_filename(&resp);
        return ResourceProbe {
            supports_ranges,
            content_length: content_len,
            filename,
        };
    }

    ResourceProbe::default()
}

fn extract_filename(resp: &reqwest::Response) -> Option<String> {
    resp.headers()
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .and_then(|cd| {
            cd.split(';').find_map(|part| {
                let part = part.trim();
                part.strip_prefix("filename=")
                    .map(|name| name.trim_matches('"').to_string())
            })
        })
}

pub fn create_initial_segments(total_bytes: u64, max_connections: usize) -> Vec<Segment> {
    const MIN_SEGMENT_SIZE: u64 = 512 * 1024; // 512 KB min segment
    let count = ((total_bytes / MIN_SEGMENT_SIZE) as usize).clamp(1, max_connections);
    let chunk_size = total_bytes / count as u64;
    let mut segments = Vec::with_capacity(count);
    let mut start = 0;
    for i in 0..count {
        let end = if i == count - 1 {
            total_bytes - 1
        } else {
            start + chunk_size - 1
        };
        segments.push(Segment {
            start,
            end,
            completed: 0,
            claimed: false,
        });
        start = end + 1;
    }
    segments
}

/// IDM's Dynamic In-Half Division Rule:
/// Locates the largest remaining un-downloaded portion among all segments.
/// If it exceeds `min_split_bytes`, divides it in half and returns the new second-half Segment.
pub fn try_split_largest_segment(segments: &mut [Segment], min_split_bytes: u64) -> Option<Segment> {
    let (idx, remaining) = segments
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let rem = (s.end + 1).saturating_sub(s.start + s.completed);
            (i, rem)
        })
        .max_by_key(|&(_, rem)| rem)?;

    if remaining >= min_split_bytes {
        let seg = &mut segments[idx];
        let current_pos = seg.start + seg.completed;
        let half = remaining / 2;
        let mid = current_pos + half - 1;
        let old_end = seg.end;
        seg.end = mid; // Sliced in half!

        Some(Segment {
            start: mid + 1,
            end: old_end,
            completed: 0,
            claimed: false,
        })
    } else {
        None
    }
}

pub fn write_at(file: &Arc<File>, buf: &[u8], offset: u64) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut written = 0;
        while written < buf.len() {
            let n = file.seek_write(&buf[written..], offset + written as u64)?;
            if n == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "failed to write whole buffer",
                ));
            }
            written += n;
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::FileExt;
        file.write_all_at(buf, offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_in_half_splitting() {
        let mut segments = vec![
            Segment { start: 0, end: 999, completed: 500, claimed: false }, // 499 remaining
            Segment { start: 1000, end: 2999, completed: 0, claimed: false }, // 2000 remaining (largest)
        ];

        let new_seg = try_split_largest_segment(&mut segments, 1000).expect("should split largest");
        // Original segment index 1 was 1000..2999 (len 2000). Half is 1000. 1000..1999.
        assert_eq!(segments[1].start, 1000);
        assert_eq!(segments[1].end, 1999);
        // New segment is 2000..2999 (len 1000)
        assert_eq!(new_seg.start, 2000);
        assert_eq!(new_seg.end, 2999);
        assert_eq!(new_seg.completed, 0);
    }

    #[test]
    fn test_create_initial_segments() {
        let segs = create_initial_segments(10_000_000, 4);
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0].start, 0);
        assert_eq!(segs[3].end, 9_999_999);
    }

    #[test]
    fn test_dynamic_in_half_splitting_partially_completed() {
        let mut segments = vec![
            Segment { start: 0, end: 1999, completed: 1000, claimed: true }, // remaining is 1000 (1000..1999)
        ];

        let new_seg = try_split_largest_segment(&mut segments, 500).expect("should split remaining");
        // Remaining was 1000. Half is 500.
        // Current pos is 0 + 1000 = 1000.
        // Mid is 1000 + 500 - 1 = 1499.
        // seg[0].end is shortened to 1499.
        assert_eq!(segments[0].start, 0);
        assert_eq!(segments[0].end, 1499);
        assert_eq!(segments[0].completed, 1000);
        // New segment is 1500..1999 (len 500).
        assert_eq!(new_seg.start, 1500);
        assert_eq!(new_seg.end, 1999);
        assert_eq!(new_seg.completed, 0);
        assert_eq!(new_seg.claimed, false);
    }
}
