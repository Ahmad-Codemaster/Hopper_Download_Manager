# Tauri Download Manager MVP Design

**Date:** 2026-10-13  
**Status:** Proposed for implementation

## 1. Goal

Deliver the first runnable Windows MVP of Hopper Download Manager: a Tauri
desktop client and Chrome handoff flow backed by one asynchronous Rust download
engine. A user must be able to add or approve an HTTP(S) download, observe
durable progress, control the job, close and reopen the UI, and finish the
file without false completion.

This is an original implementation. It must not copy Internet Download
Manager source code, branding, proprietary assets, exact internal data flows,
or pixel-identical design.

## 2. In scope

- Tauri desktop window with a modern download list.
- URL and destination form for adding a download.
- Start, pause, resume, cancel, and retry controls.
- Job status, progress, bytes, speed, destination, and structured errors.
- Durable job list and recovery after UI or host restart.
- Chrome Manifest V3 user-approved handoff through Native Messaging.
- One Windows host process serving the Chrome and Tauri clients.
- One asynchronous Tokio-based Rust engine instance.
- Direct HTTP(S) downloads with single-stream transfer.
- SQLite persistence, checkpoints, and explicit destination conflict policy.
- Local HTTP-fixture integration tests and protocol tests.

## 3. Out of scope

- DRM, paywall, authentication-control, or access-restriction bypass.
- Automatic extraction of protected or site-specific media streams.
- YouTube-specific or other site-specific extraction.
- Scheduling, categories, bandwidth policies, or advanced queue rules.
- Multi-segment transfer implementation in this milestone.
- Installer automation, code signing, automatic updates, telemetry, or cloud
  synchronization.
- Mobile or non-Windows release targets.

## 4. Architecture

```text
Chrome extension
       │ Native Messaging
       ▼
Windows host process
       │ typed command/event protocol
       ▼
Async Rust download engine
       ├── SQLite job store
       ├── checkpoint/recovery manager
       └── HTTP(S) worker tasks

Tauri desktop UI
       │ Tauri commands/events
       ▼
Same engine instance through the host boundary
```

### Rust engine

The engine owns job lifecycle, validation, HTTP transfers, pause/cancel
signals, retry policy, checkpoint persistence, destination finalization, and
event publication. It must not block the host's message loop while a transfer
is running.

The engine exposes a typed asynchronous API equivalent to:

- `create_job(spec) -> JobId`
- `start_job(job_id) -> Result<()>`
- `pause_job(job_id) -> Result<()>`
- `resume_job(job_id) -> Result<()>`
- `cancel_job(job_id) -> Result<()>`
- `retry_job(job_id) -> Result<()>`
- `list_jobs(filter) -> Vec<JobSummary>`
- `subscribe_events() -> EventReceiver`

The API must support concurrent commands safely and must not expose SQLite
connections or filesystem internals to clients.

### Windows host

The host owns the engine instance and routes validated commands from Native
Messaging and the Tauri client. It keeps the engine alive when the desktop
window closes. Protocol errors are returned as structured error envelopes; a
malformed recoverable message must not terminate the host.

The host must distinguish:

- Native Messaging framing and JSON errors.
- Unsupported protocol versions.
- Invalid engine commands.
- Engine/network/filesystem/integrity errors.
- EOF or unrecoverable transport errors.

### Tauri client

The Tauri frontend renders state received through typed commands/events. It
must not open the database, read temporary files, or implement download logic.
Closing the window disconnects the UI only; it does not cancel jobs.

The initial UI has:

- Sidebar filters: All, Active, Paused, Completed, Failed.
- Main list columns: filename, status, progress, size, speed, destination.
- Add URL action.
- Per-job actions: start/pause, cancel, retry, and open destination folder.
- Inline structured error details with sensitive values redacted.

### Chrome extension

The extension does not automatically intercept and hand off every download.
It requires a user action or explicit consent state before sending a request.
It sends only a validated HTTP(S) URL and narrowly scoped metadata required for
the authorized operation. It suppresses duplicate handoffs and does not store
passwords, cookies, or long-lived authorization secrets.

## 5. Job and event model

The durable job state machine is:

```text
Queued → Active → Paused → Active
                 ├→ Completed
                 ├→ Failed → Active (retry)
                 └→ Canceled
```

An interrupted active job is recovered as `Paused` or an explicit recovery
error, never `Completed`. Terminal completion is emitted only after the final
file has been verified and atomically finalized.

Each progress event includes:

- job ID
- current state
- completed bytes
- optional total bytes
- instantaneous or recent speed
- retry count
- structured error when state is failed

Events must be ordered per job and must be safe for a reconnecting UI to
consume. The client can always reload the authoritative job list.

## 6. Persistence and recovery

SQLite stores job identity, redacted source identity, destination, conflict
policy, state, expected size when known, completed bytes, retry count, and
schema version. Operational signed URLs and transient request headers remain
in memory only. After a process restart, a job whose operational source is no
longer available requires explicit reauthorization rather than using a
redacted URL.

Checkpoint writes happen at bounded intervals while streaming. On startup the
engine reconciles temporary files and checkpoints:

- matching state resumes safely;
- missing or inconsistent temporary data becomes an explicit recovery state;
- a completed database row without finalized output is not treated as
  completed;
- ambiguous files are not silently overwritten or discarded.

Destination resolution is persisted before transfer. `Reject` fails when the
destination exists. `Rename` chooses a collision-free name and finalization
must not replace a file created by a race.

## 7. Transfer behavior

- Accept only HTTP and HTTPS URLs.
- Follow redirects up to a configured bound.
- Prefer streamed responses and avoid loading complete files into memory.
- Retry only classified transient failures with bounded backoff.
- Support pause and cancellation through cooperative worker signals.
- Write to a controlled `.part` file.
- Verify expected size when available.
- Atomically finalize only after successful transfer and verification.
- Never execute the downloaded file automatically.
- Preserve the current strict `Content-Range` validation helper, but defer
  multi-segment scheduling to a later milestone.

## 8. Error and privacy requirements

Errors must remain visible and typed; no broad catch may turn a failure into a
success-shaped result. Logs and UI errors must redact signed URL query values,
cookies, passwords, and authorization headers. The host must cap message size,
concurrent work, redirect count, retry count, and event payload size.

The product is for user-authorized downloads. It must not implement DRM,
paywall, login, credential, or other access-control circumvention.

## 9. Testing and acceptance

### Engine

- Async local-fixture tests for start, progress, pause, resume, cancel, retry,
  redirect limits, and structured failures.
- Restart/reconciliation tests that interrupt transfers and reject false
  completion.
- Destination conflict and race tests for reject and rename policies.
- Tests that prove sensitive operational values are not persisted or logged.

### Host and protocols

- Command routing and event serialization tests.
- Native Messaging framing, malformed input, oversized input, unknown version,
  and continued processing after a recoverable invalid message.
- Tauri command/event adapter tests.

### UI and extension

- Job list rendering and filter tests.
- Action dispatch tests for add, pause, resume, cancel, and retry.
- Consent-gated Chrome handoff and duplicate suppression tests.
- Redacted error-display tests.

### MVP acceptance

On Windows, a user can:

1. Launch the host and Tauri UI.
2. Add a local-fixture HTTP(S) URL from the UI.
3. See a queued job become active with progress and speed.
4. Pause, resume, and cancel the job.
5. Close and reopen the UI without canceling the host job.
6. Recover a forced-interruption job without false completion.
7. Approve a Chrome handoff and see the same job in the UI.
8. Complete the file with correct bytes and explicit destination behavior.

Automated tests must cover this flow without depending on a third-party site.
