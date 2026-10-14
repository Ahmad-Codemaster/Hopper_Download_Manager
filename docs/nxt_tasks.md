is is the definitive blueprint. You have captured the exact mechanics that make IDM legendary: not just basic range headers, but dynamic in-half chunk slicing, connection keep-alive recycling, instant link-refresh without byte loss, and the floating media sniffer.

Consider this our official North Star Specification for the project. Every module we write in Rust from this point forward will be built to serve this architecture.

1. How We Translate the Core Mechanics into Idiomatic Rust
A. The Dynamic In-Half Segmentation Algorithm
In standard download managers, static splitting fails when a single thread crawls. We will implement IDM's Dynamic In-Half Division Rule:

text
Active Stream 1: [===================>                        ] 400 MB remaining
Stream 2 finishes its chunk and becomes idle.
IDM Dynamic Slicing Event:
Stream 1: [===================>          ] (End byte clamped to +200 MB)
Stream 2:                               [----------->          ] (Assigned 200 MB to 400 MB)
Both streams now download in parallel at full network saturation.
In Rust (download-core), we achieve this via:

Dynamic Segment Registry (Arc<tokio::sync::RwLock<SegmentRegistry>>): Tracks all active ranges: (id, start_offset, current_offset, end_offset).
Worker Pool Loop: When an async worker finishes its range, instead of exiting:
It queries the registry for active_segments.iter().max_by_key(|s| s.remaining_bytes()).
If remaining > MIN_SPLIT_THRESHOLD (e.g., 2 MB):
The chunk is halved at mid = current + remaining / 2.
The original chunk's end is updated to mid.
A new segment is registered for mid + 1 .. original_end.
The freed worker immediately begins streaming the new slice using the same connection client.
B. Eliminating the Slow "Stitching" Phase (Windows Direct-Seek Writes)
Old download managers download separate .part1, .part2 files and spend minutes copying them into one file at 100% (causing heavy SSD/HDD thrashing on 50 GB game downloads).

In Rust on Windows:

We pre-allocate the target file immediately using

std::fs::File::set_len
 and set it as a Windows sparse file or pre-allocated block.
Each parallel worker streams its bytes directly into its designated byte offset using non-blocking Windows positioned writes:
rust
tokio::fs::File // with seek / write_all_at
Result: When all streams hit 100%, the file is already fully assembled. Zero wait time, zero disk-stitching overhead.
C. URL Refreshing (Expired Link Fallback)
When a CDN link expires (403 Forbidden on S3/Google Drive/Mega):

The worker transitions to JobState::AwaitingReauthorization.
All written bytes in the pre-allocated file and SQLite database are frozen and preserved.
The desktop UI / browser extension triggers a prompt: "Refresh Download Address".
Once the user clicks the link in Chrome, the extension captures the new tokenized URL + headers and issues UpdateJobSource(job_id, new_url, new_headers).
The engine swaps the operational URL and immediately fires the workers back up against the remaining gaps.
D. The Media Sniffer (Floating Video Grabber & Stream Muxer)
Extension Level: We hook into chrome.webRequest / chrome.declarativeNetRequest to inspect Content-Type headers for video/*, audio/*, .m3u8 (HLS), and .mpd (DASH).
Video/Audio Muxing: For YouTube and modern video sites where 1080p/4K video is delivered as separate video and audio streams, the engine will download both tracks concurrently via range requests and mux them into a container (.mp4 / .mkv) using a bundled lightweight ffmpeg sidecar.
E. Container Previews (ZIP / RAR)
By sending a targeted byte range request to fetch only the final 64 KB of a .zip archive (where the Central Directory End Record resides), the Rust engine parses the table of contents and sends the file tree to the UI before downloading the full archive.

1. Implementation Execution Phases
mermaid
graph TD
    Phase1[Phase 1: Dynamic Multi-Segment Engine] --> Phase2[Phase 2: Instant Controls, Recovery & Link Refresh]
    Phase2 --> Phase3[Phase 3: Browser Interception & Floating Media Grabber]
    Phase3 --> Phase4[Phase 4: Queuing, Scheduler & Speed Throttler]
    Phase4 --> Phase5[Phase 5: Torrent Engine & Power Features]
Phase 1: Core Acceleration Engine
HTTP probe (HEAD/ranged GET for Accept-Ranges, Content-Length).
Dynamic in-half segment divider (up to 32 parallel streams).
Direct positional file writes with sparse pre-allocation.
Real-time aggregate speed and ETA calculation.
Phase 2: Durability & Link Refreshing
Cooperative CancellationToken for instant pause/cancel.
Checkpointing segment ranges in SQLite.
403/410 catch and UpdateJobSource live link refreshing.
Phase 3: Browser Integration & Media Sniffer
Manifest V3 media sniffer with floating "Download this Video" overlay.
HLS/DASH audio+video stream extraction.
One-click file extension interception (.zip, .iso, .exe, .mp4, etc.).
Phase 4: Automation & Power Features
Speed limiter (token bucket algorithm in Tokio).
Batch wildcard downloads ([1-100].jpg).
Scheduler (start at time $X$, shutdown PC on 100%).
ZIP/RAR central directory preview.
Phase 5: Torrents & Native Additions
BitTorrent protocol integration via librqbit.
Antivirus auto-scan integration via Windows Defender CLI.
