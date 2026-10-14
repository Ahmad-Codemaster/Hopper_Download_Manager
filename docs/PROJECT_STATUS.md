# Hopper Download Manager

## Project Status and Roadmap

**Project:** Hopper Download Manager  
**Platform:** Windows  
**Repository:** [Ahmad-Codemaster/Hopper_Download_Manager](https://github.com/Ahmad-Codemaster/Hopper_Download_Manager)  
**Current branch:** `master`  
**Status date:** 2026-10-14  
**Current release stage:** Alpha Feature Complete / Standalone Windows Build & Extension Handoff

---

## 1. Executive Summary

Hopper Download Manager is a high-performance, open-source Windows download accelerator and stream grabber built in Rust, Tokio, Tauri v2, and Chrome Manifest V3. It provides an efficient, multi-threaded alternative to Internet Download Manager (IDM) with native Windows integration, dynamic range segmentation, and in-page video sniffing.

The project currently contains:

- **High-Speed Multi-Threaded Engine (`download-core`):** Tokio-based async engine with dynamic split-in-half range chunking, token-bucket speed limiter, ZIP central directory previewing, and SQLite persistence.
- **Standalone Windows Desktop GUI (`hopper.exe`):** Native Windows application (`#![windows_subsystem = "windows"]`) embedding the dark-mode React/TypeScript UI directly into the binary with zero console popup and offline local operation.
- **Native Messaging Host (`download-host.exe`):** Low-latency STDIO IPC bridge between browser extensions and the Rust download engine with flushed binary framing.
- **Chrome / Edge / Brave MV3 Extension:** Full-featured browser integration with automatic download interception, in-page floating media grabber (`⚡ Download Video`), automatic HLS (`.m3u8`) variant parsing (1080p/720p/480p), smart page-title naming, and automatic browser download fallbacks.
- **Original Branding & Assets:** Complete high-resolution icon set for Windows PE executables (`.ico`) and browser extensions (`16x16`, `32x32`, `48x48`, `128x128`, `512x512`).
- **Comprehensive Test Suite:** 27 Rust engine/host integration tests and 9 Node.js extension unit tests passing at 100% green.

---

## 2. Work Completed

### 2.1 Multi-Threaded Rust Download Engine (`crates/download-core`)

- **Dynamic Range Segmentation:** Implemented `split_segment_in_half` dynamic worker allocation to split active segments into parallel sub-streams whenever bandwidth allows.
- **Token-Bucket Bandwidth Limiter:** Configurable speed throttling with deficit-based token consumption (`limiter.rs`).
- **ZIP Archive Central Directory Inspector:** Remote central directory parsing allowing file inspection inside ZIP archives before full transfer (`archive.rs`).
- **Resilient Recovery & URL Refresh:** `RefreshSource` protocol command allowing expired signed download links to be refreshed on the fly without discarding downloaded chunks.
- **Durable Checkpointing:** SQLite-backed byte tracking with temporary `.part` file staging and atomic rename on completion.
- **Safe Path & Security Policies:** Traversal protection, sensitive query redaction, and collision-free rename policies.

### 2.2 Native Messaging Host & IPC Protocol (`crates/download-host`)

- **Framed Binary Protocol:** Little-endian length-prefixed JSON messaging over standard input/output.
- **Immediate Stdout Flushing:** Added explicit `writer.flush()` to ensure Chrome receives IPC responses instantaneously without hanging.
- **Typed Command Router:** Full lifecycle controls for `List`, `Create`, `Start`, `Pause`, `Resume`, `Cancel`, and `RefreshSource`.
- **Automated Windows Registration:** Created [`register_chrome_host.bat`](file:///c:/Users/ahmad/AndroidStudioProjects/IDM_project/register_chrome_host.bat) with absolute script directory resolution (`%~dp0`), linking Chrome, Edge, and Brave native host registry entries to the compiled release binary.

### 2.3 Standalone Windows Desktop GUI (`src-tauri` & `ui`)

- **Subsystem Flag:** Configured `#![windows_subsystem = "windows"]` in `src-tauri/src/main.rs` to completely eliminate black CMD console popups when launching the app.
- **Offline Embedded UI:** Removed Vite dev-server dependencies from `tauri.conf.json`; the compiled production bundle (`ui/dist`) is embedded directly into `hopper.exe`.
- **Modern Dark-Mode Dashboard:** React + TypeScript UI featuring live download status, progress bars, pause/resume/cancel controls, and category filtering.
- **Embedded Application Icons:** Configured multi-resolution icons (`icon.ico`, `icon.png`, `32x32.png`, `128x128.png`) in `tauri.conf.json` and compiled directly into the Windows executable resources.

### 2.4 Browser Extension & In-Page Video Grabber (`extension`)

- **Manifest V3 Service Worker Isolation:** Wrapped [`extension/src/protocol.js`](file:///c:/Users/ahmad/AndroidStudioProjects/IDM_project/extension/src/protocol.js) in a scoped IIFE to eliminate `validateHandoff` global identifier collisions in the Chrome Service Worker.
- **In-Page Floating Video Grabber:**
  - Non-intrusive floating button overlay (`⚡ Download Video`) injected into video streaming websites.
  - DOM inspection for `<video>` and `<audio>` tags, tracking resolutions (`videoWidth` x `videoHeight`).
- **Automatic HLS Multi-Quality Parsing:**
  - Network sniffing for HLS master playlists (`.m3u8`) and DASH (`.mpd`), including queries with security tokens.
  - Background variant playlist extraction parsing distinct resolutions (`1080p Full HD`, `720p HD`, `480p`, `360p`) and bitrates.
- **Smart Page-Title Naming:**
  - Replaces meaningless CDN hash segments (e.g. `2sed-qjYxbC3UNEcc2JZaHdzPxpi...`) with sanitized webpage titles (e.g. `Movie Title [1080p].mp4`).
- **Noise & Telemetry Filtering:**
  - Explicitly filters out images (`.png`, `.jpg`, `.webp`, `.svg`), web fonts (`.woff2`), and tiny `< 100 KB` telemetry octet-stream pings.
- **Resilient Fallback Mechanism:**
  - Automatically falls back to `chrome.downloads.download` if the native host is ever unavailable, guaranteeing downloads never fail with unhandled errors.

### 2.5 Verification & Automated Test Status

All workspace checks and unit tests are passing:

```text
==================================================
Rust Cargo Tests: 27 / 27 PASS (0 failed)
- download-core unit tests: 10 passed
- async_engine_tests: 5 passed
- range_tests: 2 passed
- recovery_tests: 2 passed
- store_tests: 1 passed
- transfer_tests: 2 passed
- host_routing_tests: 2 passed
- ipc_tests: 3 passed
- native_messaging_tests: 2 passed

Extension Tests: 9 / 9 PASS (0 failed)
- validateHandoff & redaction
- isMediaUrl & isDownloadableUrl
- Asset & octet-stream noise filtering
- Filename extraction & hash replacement with page title
- HLS m3u8 playlist variant parsing
- Native IPC command formatting
==================================================
```

---

## 3. Current Architecture

```text
┌────────────────────────────────────────────────────────┐
│               Web Browser (Chrome / Edge / Brave)      │
│  ┌───────────────────────┐   ┌──────────────────────┐  │
│  │ In-Page Media Grabber │   │  Download Sniffer &  │  │
│  │ (HLS / DASH / Video)  │   │  Auto-Interceptor    │  │
│  └───────────┬───────────┘   └───────────┬──────────┘  │
│              └─────────────┬─────────────┘             │
│                            │ Native Messaging (STDIO)   │
└────────────────────────────┼───────────────────────────┘
                             ▼
┌────────────────────────────────────────────────────────┐
│       Windows Native Messaging Host (download-host)     │
│        - Flushed binary framing (LE u32 + JSON)        │
│        - Command routing: List, Create, Start, Pause   │
└────────────────────────────┬───────────────────────────┘
                             ▼
┌────────────────────────────────────────────────────────┐
│           Rust Engine Core (download-core)             │
│  ├── Dynamic Range Slicer (Multi-connection HTTP)      │
│  ├── Token-Bucket Bandwidth Limiter                    │
│  ├── ZIP Central Directory Inspector                   │
│  └── SQLite Durable State & Checkpoint Store           │
└────────────────────────────▲───────────────────────────┘
                             │ Tauri Invoke IPC
┌────────────────────────────┴───────────────────────────┐
│           Hopper Desktop Shell (hopper.exe)            │
│  ├── Native Windows Subsystem (No CMD Console)         │
│  ├── Embedded Offline React + TypeScript Dashboard     │
│  └── PE Embedded Application Icons (.ico)              │
└────────────────────────────────────────────────────────┘
```

---

## 4. How to Run & Verify

### 4.1 Launch Desktop Application
Execute the standalone release binary directly:
```powershell
& "c:\Users\ahmad\AndroidStudioProjects\IDM_project\target\release\hopper.exe"
```
The application opens instantly with zero background console window and no network dependencies.

### 4.2 Register Browser Integration
Run the automated registration batch script:
```cmd
cd c:\Users\ahmad\AndroidStudioProjects\IDM_project
register_chrome_host.bat
```
This updates Windows registry keys for Google Chrome, Microsoft Edge, and Brave.

### 4.3 Load Extension in Chrome
1. Open `chrome://extensions` and enable **Developer mode**.
2. Click **Load unpacked** and select `c:\Users\ahmad\AndroidStudioProjects\IDM_project\extension`.
3. Open any video streaming website: the `⚡ Download Video` floating button will detect media streams and list available qualities.

---

## 5. Next Planned Milestones

1. **BitTorrent Engine Integration:** Integrate `librqbit` in `crates/download-core` to download `.torrent` files and magnet links within the same unified dashboard.
2. **Audio/Video Stream Muxer (FFmpeg Sidecar):** Automatic demuxing and combining of separate adaptive video and audio tracks into a single `.mp4` file.
3. **MSI / InnoSetup Installer:** One-click Windows installer automating binary extraction, desktop shortcut creation, and browser registry configuration.
