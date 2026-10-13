# Hopper Download Manager

## Project Status and Roadmap

**Project:** Hopper Download Manager  
**Platform:** Windows  
**Repository:** [Ahmad-Codemaster/Hopper_Download_Manager](https://github.com/Ahmad-Codemaster/Hopper_Download_Manager)  
**Current branch:** `master`  
**Status date:** 2026-10-13  
**Current release stage:** Early Windows MVP / development build

---

## 1. Executive Summary

Hopper Download Manager is an original, open-source Windows download manager
built to provide a reliable and privacy-conscious alternative for
user-authorized HTTP(S) downloads.

The project currently contains:

- A Rust download engine and SQLite persistence layer.
- A Windows-compatible Tauri 2 desktop shell.
- A React and TypeScript user interface.
- A Chrome Manifest V3 extension foundation.
- A Native Messaging host protocol with typed command routing.
- Automated Rust, TypeScript, UI, and extension verification.
- A locally generated Windows executable for development use.

The project is functional as a development MVP: the desktop shell can be
built, launched, and used to submit HTTP(S) download jobs. However, it is not
yet a complete production replacement for a mature commercial download
manager. The most important remaining engineering work is cooperative
pause/resume/cancel behavior during active transfers, live progress events,
and unifying the desktop and Chrome clients behind one long-lived engine
instance.

---

## 2. Work Completed

### 2.1 Project foundation

- Created a public GitHub repository.
- Established an original clean-room implementation.
- Added project documentation, design specifications, and implementation
  planning records.
- Configured a Rust workspace containing the download core and Windows host.
- Added repository ignore rules for generated build output and local
  dependencies.

### 2.2 Rust download core

- HTTP and HTTPS URL validation.
- Unsafe destination and path-traversal rejection.
- Redaction of sensitive URL query values for durable records.
- SQLite-backed job persistence.
- Duplicate source/destination detection.
- Durable job identifiers.
- Conflict policies:
  - Reject an existing destination.
  - Choose a collision-free renamed destination.
- Temporary `.part` file transfer flow.
- Atomic final-file rename after transfer completion.
- Checkpoint byte persistence.
- Recovery handling for completed database rows whose final file is missing.
- Strict `Content-Range` validation primitives for future segmented downloads.
- Redirect limit configuration through the HTTP client.
- In-memory operational source handling so signed URLs are not persisted.

### 2.3 Async engine API foundation

The engine exposes asynchronous wrappers for:

- Creating a job.
- Starting a job.
- Listing persisted jobs.
- Pausing a job.
- Resuming a job.
- Canceling a job.
- Retrying a job.

The engine also handles the important reauthorization boundary: an
operational source URL is retained in memory only, so a process restart does
not silently reuse a sensitive or expired signed URL.

### 2.4 Windows host and protocol

- Native Messaging framing with little-endian message lengths.
- Message-size limits.
- Versioned JSON envelopes.
- Typed commands for listing, creating, starting, pausing, resuming,
  canceling, and retrying jobs.
- Structured success and error responses.
- Unsupported protocol-version rejection.
- Recoverable malformed-message handling.
- Long-lived host startup with a persistent local engine and storage location.

### 2.5 Desktop application

- Tauri 2 application shell.
- Windows window configuration.
- React and TypeScript frontend.
- Vite development and production build configuration.
- URL entry field with HTTP(S) validation.
- Download list rendering.
- Initial status and progress display.
- Tauri invoke adapters for job commands.
- Initial persisted-job loading on UI startup.
- Development application icon.
- Local application data directory:

  ```text
  %LOCALAPPDATA%\HopperDownloadManager
  ```

### 2.6 Chrome extension foundation

- Manifest V3 extension structure.
- HTTP(S) handoff validation.
- Sensitive query redaction.
- Explicit consent requirement before handoff.
- Duplicate handoff protection foundation.
- Native host connection helper.

The extension does not automatically intercept every browser download and does
not bypass authentication, DRM, paywalls, or site access controls.

### 2.7 Verification completed

The following checks have passed on the current implementation:

```text
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p hopper
npm run build
npm run bundle
node --test
```

The Tauri release build has also produced:

```text
target\release\hopper.exe
```

This executable is a local development build and is not currently signed or
distributed as an installer.

---

## 3. Current Architecture

```text
Chrome extension
       │ Native Messaging
       ▼
Windows download host
       │ typed commands
       ▼
Rust download engine
       ├── SQLite job store
       ├── checkpoint persistence
       └── HTTP(S) transfer layer

Tauri desktop application
       │ Tauri invoke commands
       ▼
Rust engine instance
```

The intended final architecture is for the Windows host to own one
long-lived engine and serve both the Chrome extension and the desktop client.
The current implementation has the protocol and routing foundations, but the
Tauri application currently initializes its own in-process engine. This
distinction is important and must be resolved before claiming full
cross-client synchronization.

---

## 4. How to Run the Application

### 4.1 Run the existing executable

After building the application, launch:

```text
target\release\hopper.exe
```

From PowerShell:

```powershell
& ".\target\release\hopper.exe"
```

Windows SmartScreen may warn because the development executable is unsigned.

### 4.2 Build the executable

From the repository root:

```powershell
cd "C:\Users\ahmad\AndroidStudioProjects\IDM_project"

$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

npm --prefix ui install
npx --prefix ui tauri build
```

The executable is generated at:

```text
target\release\hopper.exe
```

### 4.3 Development mode

The Tauri configuration starts the frontend development server automatically:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
npx --prefix ui tauri dev
```

### 4.4 Chrome development setup

The Chrome integration is not yet packaged as an installer. Development setup
currently requires:

1. Building `download-host`.
2. Creating a Native Messaging host manifest.
3. Registering the manifest for the current Windows user.
4. Loading the `extension/` directory as an unpacked Chrome extension.
5. Configuring the extension ID in the Native Messaging manifest.

---

## 5. Remaining MVP Work

The following work is required before the project should be considered a
complete MVP.

### 5.1 Replace the blocking transfer wrapper

The current async API wraps the existing blocking transfer implementation.
The transfer itself still runs to completion inside a blocking operation.

Required work:

- Use Tokio-owned worker tasks.
- Give every active job a control channel.
- Stop streaming cooperatively when pause or cancel is requested.
- Persist a non-terminal checkpoint before interruption.
- Ensure an interrupted job never emits `Completed`.
- Bound the number of active workers.
- Implement clean worker shutdown.

### 5.2 Add real progress and event streaming

Required work:

- Emit progress events while bytes are streamed.
- Include completed bytes, optional total bytes, speed, retry count, and state.
- Preserve event ordering per job.
- Allow the UI to reconnect and reload authoritative state.
- Avoid duplicate workers after reconnect.
- Forward engine events through both Tauri and Native Messaging.

### 5.3 Finish durable recovery

Required work:

- Reconcile active jobs on process startup.
- Recover interrupted jobs as paused or explicit recovery-error states.
- Reconcile temporary files and checkpoints.
- Detect inconsistent partial files.
- Require explicit reauthorization after restart when the operational URL is
  unavailable.
- Add tests for interrupted transfers and restart behavior.

### 5.4 Unify host and desktop engine ownership

The Chrome host and Tauri desktop currently do not share the same in-memory
worker manager. Sharing the SQLite file is not enough to share active workers
or operational URLs.

Required work:

- Choose one engine owner, preferably the long-lived Windows host.
- Route Tauri commands and events through that owner.
- Keep workers alive when the desktop window closes.
- Serialize commands from Chrome and Tauri.
- Return typed conflict errors for concurrent commands.

### 5.5 Complete the desktop UI

Required UI additions:

- Sidebar filters for All, Active, Paused, Completed, and Failed.
- Add-download dialog with destination selection.
- Per-job start, pause, resume, cancel, retry, and open-folder actions.
- Live speed and total-size display.
- Inline structured error details.
- Destination conflict feedback.
- Empty, loading, reconnecting, and error states.
- Accessible keyboard and screen-reader behavior.

### 5.6 Finish Chrome consent flow

Required work:

- Add a visible user action for handoff.
- Show the URL and destination context before confirmation.
- Display host connection errors clearly.
- Suppress duplicate handoffs reliably.
- Add end-to-end tests using a local fixture and a test Native Messaging
  boundary.

### 5.7 Add packaging and installation

Required work:

- Build a Windows installer.
- Register the Native Messaging host automatically for the installed
  extension.
- Include the desktop executable and required runtime assets.
- Define upgrade and uninstall behavior.
- Add code signing before public distribution.
- Publish checksums and release notes.

---

## 6. Known Limitations

- Transfers are single-stream only.
- Multi-segment acceleration is not implemented.
- Pause and cancel currently update durable state but do not reliably interrupt
  an already-streaming transfer.
- Live progress event delivery is incomplete.
- The desktop UI is an initial shell rather than a polished production UI.
- Chrome integration requires manual development registration.
- There is no installer.
- There is no automatic update mechanism.
- There is no bandwidth scheduler or queue policy.
- There are no download categories or advanced filtering rules.
- There is no media-site extraction support.
- There is no mobile build.
- There is no telemetry or cloud synchronization.
- The development executable is unsigned.

---

## 7. Recommended Implementation Order

### Priority 1: Correct transfer lifecycle

Implement cooperative worker controls, checkpointing, cancellation, pause,
resume, retries, and event publication. This is the highest priority because
it affects correctness and user trust.

### Priority 2: One shared engine owner

Move desktop and Chrome clients behind the same host-owned engine boundary.
This prevents conflicting state and makes closing/reopening the desktop window
safe.

### Priority 3: Authoritative UI updates

Add event listeners, reconnect handling, filters, action buttons, and
structured errors. The UI should render engine state rather than fabricate
local rows.

### Priority 4: Recovery and integration testing

Add deterministic local HTTP fixtures for:

- Slow streaming.
- Pause and resume.
- Cancellation.
- Retryable failures.
- Redirect limits.
- Missing or inconsistent partial files.
- Restart reauthorization.
- Concurrent client commands.

### Priority 5: Installer and Chrome setup

Package the application, automate host registration, sign the binaries, and
document a normal end-user installation path.

### Priority 6: Performance features

Only after correctness is stable, evaluate segmented transfers, bandwidth
limits, queue scheduling, and connection reuse.

---

## 8. Future Enhancements

Potential future additions, ordered by user value and implementation risk:

### Download management

- Multi-segment downloads with strict range validation.
- Pause/resume across application restarts when authorization remains valid.
- Download queue priorities.
- Maximum simultaneous downloads.
- Bandwidth limits and schedules.
- Automatic retry policies with configurable backoff.
- Checksums and optional integrity verification.
- File hash verification from trusted metadata.
- Duplicate-content detection.

### User experience

- System tray operation.
- Global hotkeys.
- Drag-and-drop URL support.
- Clipboard monitoring with explicit consent.
- Search and advanced filters.
- Download categories and rules.
- Open containing folder.
- Notifications for completion and failure.
- Theme selection and accessibility improvements.
- Localization.

### Browser integration

- Firefox and Chromium-family packaging.
- Better explicit handoff prompts.
- Browser context-menu integration.
- Browser download interception only when enabled by the user.
- Per-site consent preferences without storing credentials.

### Release engineering

- Signed installer.
- Automated GitHub release workflow.
- Reproducible release builds.
- SBOM and dependency auditing.
- Crash reporting that excludes URLs, cookies, credentials, and file contents.
- Automatic updates with signature verification.

### Security and privacy

- More detailed secret-redaction tests.
- Hardened local IPC authorization.
- Instance ownership and lock files for concurrent process protection.
- Secure file permission review.
- Optional encrypted local metadata for sensitive environments.

---

## 9. Safety and Scope Boundaries

Hopper is intended only for downloads the user is authorized to access.

The project must not:

- Circumvent DRM.
- Bypass paywalls or authentication controls.
- Extract protected media streams.
- Steal or persist cookies, passwords, or authorization headers.
- Automatically execute downloaded files.
- Copy proprietary Internet Download Manager code, branding, assets, or
  pixel-identical design.

Operational signed URLs and transient authorization data must remain in memory
only and must never appear in durable job records or logs.

---

## 10. Definition of Production Readiness

The project should not be called production-ready until all of the following
are true:

- Active transfers can be paused and canceled cooperatively.
- Progress events are live, ordered, and reconnect-safe.
- Chrome and Tauri use one authoritative engine owner.
- Restart recovery is deterministic and tested.
- Sensitive operational values are redacted from records and errors.
- The UI exposes all important states and errors.
- A Windows installer can register and remove the Native Messaging host.
- Release binaries are signed.
- Full Rust, UI, extension, and integration tests pass in CI.
- A clean Windows machine can install and run the application without
  development tools.

---

## 11. Current Decision Summary

The project is intentionally proceeding as a native Windows application using
Rust, Tauri, React, TypeScript, SQLite, and Chrome Manifest V3. This keeps the
download engine platform-focused while allowing a maintainable desktop UI and
explicit browser integration.

The next implementation focus should be correctness of the worker lifecycle
and shared engine ownership, not additional visual features. Once those are
stable, packaging and release automation will provide the largest improvement
for normal users.
