# Open-Source Windows Download Manager Design

**Date:** 2026-10-13  
**Status:** Proposed for implementation

## 1. Product intent

Build an original, open-source Windows download manager with a familiar
download-manager workflow and a modern, smooth interface. The product is
intended for general users and will include a Chrome extension that can hand
off user-authorized downloads to the Windows application.

The implementation must be clean-room and original. It must not copy Internet
Download Manager source code, branding, proprietary assets, exact internal
data flows, or pixel-identical UI. The application must not bypass DRM,
paywalls, authentication controls, or other access restrictions. Media-source
detection is limited to streams the browser exposes and the user is
authorized to download, subject to the applicable site's rules.

## 2. Scope

### First milestone

The first milestone is the reliable transfer engine and browser handoff:

- Direct HTTP and HTTPS downloads.
- User-initiated handoff from Chrome through Native Messaging.
- Safe forwarding of relevant request metadata without persisting passwords.
- Metadata probing through `HEAD` when useful, with a safe fallback to `GET`.
- Redirect handling with bounded redirect counts.
- Single-stream downloads and ranged segmented downloads when the server
  advertises and honors byte ranges.
- Pause, resume, cancel, retry, and queue operations.
- Persistent job state and recovery after application restart.
- Temporary-file writes followed by atomic completion rename.
- Optional size and checksum verification when trustworthy metadata is
  available.
- Explicit job states and actionable error reporting.
- Duplicate-download prevention for the same source and destination.

### Later milestones

The following are deliberately outside the first milestone:

- Full modern desktop UI.
- Scheduling, bandwidth policies, categories, and advanced queue rules.
- A broader browser support matrix.
- Site-specific media extraction.
- Protocols other than HTTP(S).
- Installer, signing, automatic updates, telemetry, and cloud features.

Later media support must use documented, permitted browser-visible sources. It
must not become a DRM or access-control bypass mechanism.

## 3. Recommended architecture

The system is split into independently testable components:

1. **Rust core engine**
   - Owns job lifecycle, HTTP(S) transfer, segmentation, retry policy,
     checkpoint persistence, file assembly, verification, and events.
   - Exposes a typed command/event API so the desktop host and future UI do not
     depend on implementation details.

2. **Windows host**
   - Runs as the local application/service boundary.
   - Owns Windows-specific process lifetime, local IPC, filesystem locations,
     and the Chrome Native Messaging host registration.
   - Does not duplicate transfer logic from the Rust core.

3. **Chrome extension**
   - Detects eligible browser download requests and presents a user-facing
     handoff action.
   - Sends a validated URL and narrowly scoped request metadata to the local
     Native Messaging host.
   - Receives job acceptance and status updates.
   - Does not store passwords, cookies, or long-lived authentication secrets.

4. **Future desktop UI**
   - Consumes the host command/event API.
   - Displays queue, progress, errors, and destination controls.
   - Does not access the engine's database or temporary files directly.

This separation keeps the transfer engine reusable and makes the browser
integration replaceable without changing download correctness.

## 4. Data flow

1. The user starts or approves a browser download.
2. The extension checks that the request is eligible for handoff and sends the
   URL plus safe request context to the local Native Messaging host.
3. The Windows host validates message shape, size, origin, and destination
   policy, then forwards a typed command to the Rust engine.
4. The engine creates a durable job record and returns an identifier.
5. The engine probes the resource and chooses a single stream or a bounded
   number of byte-range segments.
6. Each segment writes only to a controlled temporary location. Progress and
   checkpoints are persisted at bounded intervals.
7. Transient failures use bounded exponential backoff. Authorization errors,
   unsupported range behavior, invalid destinations, and integrity failures are
   surfaced as non-retryable or explicitly recoverable states.
8. Completed segments are validated and assembled. If expected size or a
   trusted checksum is known, it is verified.
9. The temporary file is atomically renamed to the final destination.
10. The engine emits completion or failure events; the extension and future UI
    update their state from those events.

Request metadata is minimized. Cookies or authorization headers may be
forwarded only for a user-authorized handoff, only through an in-memory
operation, and never into ordinary logs or durable job records. The initial
implementation should prefer a browser-side authenticated handoff mechanism
that avoids persisting raw credentials.

## 5. Core interfaces

The engine API should provide typed commands equivalent to:

- `CreateJob(source, destination, request_context)`
- `StartJob(job_id)`
- `PauseJob(job_id)`
- `ResumeJob(job_id)`
- `CancelJob(job_id)`
- `RetryJob(job_id)`
- `GetJob(job_id)`
- `ListJobs(filter)`
- `SubscribeEvents()`

Commands must validate input and return structured errors. Events should
include job state, bytes completed, total bytes when known, speed, retry count,
and an error classification when applicable. The wire format between the
Windows host and extension/UI should be versioned and bounded in size.

## 6. Persistence and filesystem safety

The engine needs durable records for job identity, source, destination,
selected transfer mode, segment ranges, completed bytes, retry state, and
schema version. Sensitive transient request material must not be stored in
these records.

Downloads should use an application-controlled temporary directory and a
collision-safe finalization policy. Destination paths must be normalized and
validated to prevent traversal outside the user's selected location. Existing
files must never be silently overwritten; the product should expose an
explicit conflict policy.

On startup, unfinished jobs are reconciled with temporary files and resumed
only when their checkpoint and server behavior are still valid. Corrupt or
ambiguous state must become an explicit recovery error rather than silently
starting over.

## 7. Reliability, security, and privacy

- Enforce limits for concurrent jobs, segments per job, response size metadata,
  redirects, retries, and Native Messaging message size.
- Use TLS through the platform-supported HTTP client and reject malformed
  URLs and unsupported schemes.
- Do not execute downloaded files automatically.
- Keep logs free of cookies, authorization headers, passwords, and complete
  signed URLs when they contain sensitive query parameters.
- Distinguish network interruption, server capability, authorization,
  filesystem, integrity, and policy errors.
- Never claim success when a transfer is incomplete or verification failed.
- Make cancellation and shutdown cooperative so temporary files can be
  reconciled safely.

## 8. Testing strategy

### Engine tests

Use local deterministic HTTP fixtures to test:

- Full-file downloads.
- `Content-Disposition` and unknown content length.
- Redirects and redirect limits.
- Range support, multi-range assembly, and servers that ignore ranges.
- Pause/resume and restart recovery.
- Slow responses, disconnects, retries, and cancellation.
- Corrupt chunks, mismatched sizes, checksum failures, and disk errors.
- Duplicate jobs and destination conflicts.
- Input validation and path traversal defenses.

### Host and extension tests

- Native Messaging framing, malformed messages, size limits, and versioning.
- Host registration and process lifecycle on Windows.
- Extension permission boundaries and user-consent behavior.
- Duplicate handoff prevention.
- Progress, completion, and error event handling.
- Verification that sensitive request metadata is not logged or persisted.

The first milestone should include automated unit tests and integration tests
that run without requiring a live third-party website.

## 9. Acceptance criteria for milestone one

The milestone is complete when a user can install/run the local host, approve
a Chrome download, see a durable queued job, pause and resume it, close and
reopen the host, and finish the file correctly for both a single-stream
server and a range-capable test server. A forced disconnect must produce a
recoverable job rather than a false success. Invalid URLs, unauthorized
requests, unsafe paths, and integrity failures must be visible as explicit
errors.

## 10. Open implementation choices

These choices should be made during implementation planning, not by copying
another product:

- Rust HTTP client and persistence library.
- Host IPC transport and process model.
- Database format and migration mechanism.
- Desktop UI toolkit.
- Chrome extension manifest version and permission minimum.
- Packaging and code-signing strategy.

Each choice must preserve the boundaries, privacy rules, and acceptance
criteria above.
