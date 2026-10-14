# Hopper Download Manager

[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-v2-blue.svg)](https://tauri.app/)
[![Manifest V3](https://img.shields.io/badge/Chrome-Manifest%20V3-green.svg)](https://developer.chrome.com/docs/extensions/mv3/intro/)
[![Tests](https://img.shields.io/badge/Tests-All%20Pass%20(36%2F36)-brightgreen.svg)](#testing--verification)

**Hopper Download Manager** is an original, high-performance Windows download manager and adaptive stream accelerator built in **Rust (Tokio)**, **Tauri v2**, and **Chrome Manifest V3**. Designed as a clean, reliable, and privacy-conscious alternative to Internet Download Manager (IDM), it delivers dynamic multi-threaded range chunking, an in-page floating media grabber, automatic HLS stream parsing, and standalone desktop operation.

---

## Key Features

- **Dynamic Range Segmentation:** Splits downloads on-the-fly into parallel byte-range streams, utilizing available network bandwidth.
- **In-Page Floating Video Grabber (`⚡ Download Video`):** Non-intrusive floating overlay on streaming websites that sniffs video/audio playback and lets you pick quality variants with one click.
- **HLS (`.m3u8`) & DASH (`.mpd`) Stream Parser:** Automatically intercepts streaming manifests, parses all quality variants (`1080p Full HD`, `720p HD`, `480p`, `360p`), and extracts stream bitrates.
- **Smart Page-Title Naming:** Intelligently replaces obscure CDN hash names (e.g. `2sed-qjYxbC3UNEcc2JZaHdzPxpi...`) with clean, sanitized webpage titles (e.g. `Movie Title [1080p].mp4`).
- **Noise & Telemetry Shield:** Filters out images (`.png`, `.jpg`, `.webp`), fonts (`.woff2`), and tiny telemetry/beacon octet-stream pings (`< 100 KB`).
- **Resilient Handoff & Fallback:** Seamlessly routes downloads to the native Rust engine via Native Messaging; automatically falls back to browser downloads if the native host is ever offline.
- **Token-Bucket Bandwidth Limiter:** Configurable speed throttling with microsecond token replenishment.
- **ZIP Central Directory Inspector:** Previews and inspects files contained within remote ZIP archives before downloading the full archive.
- **Link Refresh (`RefreshSource`):** Resume interrupted downloads even after signed links or pre-signed tokens expire without discarding already-downloaded chunks.
- **Native Windows Standalone GUI:** Clean dark-mode React + TypeScript interface embedded directly into `hopper.exe` with zero black CMD console popups (`#![windows_subsystem = "windows"]`) and full offline operation.
- **Custom Application & Extension Icons:** Multi-resolution branding embedded in Windows PE resources (`.ico`) and browser extensions (`16x16`, `32x32`, `48x48`, `128x128`, `512x512`).

---

## Architecture Overview

```text
┌────────────────────────────────────────────────────────┐
│            Browser (Chrome / Edge / Brave)             │
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

## Quick Start

### 1. Run the Standalone Desktop Application

Launch the compiled release executable directly:

```powershell
& ".\target\release\hopper.exe"
```

The application opens instantly as a native Windows window with no background command prompt.

### 2. Register Browser Integration

Register the native messaging host for Chrome, Edge, and Brave with one command:

```cmd
register_chrome_host.bat
```

This creates the host manifest with the resolved binary path and sets the Windows Registry keys automatically.

### 3. Load the Browser Extension

1. Open **Google Chrome**, **Microsoft Edge**, or **Brave** and navigate to `chrome://extensions`.
2. Toggle on **Developer mode** in the top-right corner.
3. Click **Load unpacked** and select the `extension/` directory from this repository:
   ```
   C:\Users\ahmad\AndroidStudioProjects\IDM_project\extension
   ```
4. Pin the **Hopper Download Manager** icon to your toolbar.
5. Open any video or download page:
   - Click the floating `⚡ Download Video` button on streaming videos to pick qualities.
   - Standard browser downloads are automatically intercepted and accelerated.

---

## Building from Source

### Prerequisites

- [Rust](https://rustup.rs/) (1.75 or newer)
- [Node.js](https://nodejs.org/) (20 or newer) and `npm`

### Build All Components

```powershell
# 1. Build Rust Release Binaries (hopper.exe and download-host.exe)
cargo build --release

# 2. Build UI Production Assets (embedded into hopper.exe)
Push-Location ui
npm install
npm run bundle
Pop-Location

# 3. Test Extension
Push-Location extension
npm test
Pop-Location
```

The production executables will be located at:
- `target\release\hopper.exe` — Native Windows Desktop Manager
- `target\release\download-host.exe` — Native Messaging Host

---

## Testing & Verification

All automated test suites run locally with zero external network dependencies:

```powershell
# Run all 27 Rust engine and host integration tests
cargo test --workspace

# Run Clippy lints
cargo clippy --workspace --all-targets -- -D warnings

# Run all 9 browser extension unit tests
npm --prefix extension test
```

### Test Coverage Highlights:
- **`download-core` (10 tests):** Multi-range dynamic segment splitting, token-bucket throttling, ZIP central directory parsing, HTTP(S) path traversal security, and query parameter redaction.
- **`async_engine_tests` (5 tests):** Queue concurrency bounding, URL refresh updating, active state reconciliation, and lifecycle controls.
- **`host_routing_tests` & `ipc_tests` (5 tests):** Typed command serialization, envelope roundtripping, and payload bounds checking.
- **`native_messaging_tests` (2 tests):** Binary length-prefix framing and resilient stream handling.
- **`extension` (9 tests):** HLS M3U8 variant parsing, URL classification, image/telemetry noise filtering, page-title hash replacement, and native command builders.

---

## Data Storage & Privacy

All application data and state are stored locally in the Windows user profile:

```text
%LOCALAPPDATA%\HopperDownloadManager\
├── downloads.db       # SQLite job metadata & resume checkpoints
└── temp\              # Active multi-threaded segment files (.part)
```

- **Zero Telemetry:** Hopper contains no analytics, telemetry, or remote tracking.
- **Sensitive Query Redaction:** Session keys, auth tokens, and temporary signatures are automatically redacted from persistent database logs.
- **Safety Boundaries:** Hopper does not circumvent DRM, crack paywalls, or bypass site authentication controls.

---

## License

This project is an original, clean-room implementation licensed under the **MIT License**.
