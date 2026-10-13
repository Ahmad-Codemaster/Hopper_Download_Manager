# Open-Source Windows Download Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first usable milestone of an original Windows download manager with a durable Rust transfer engine and Chrome Native Messaging handoff.

**Architecture:** A Cargo workspace contains a transport-independent `download-core` library, a Windows host binary that exposes the core through versioned local IPC and Native Messaging, and a thin Chrome extension. The engine owns transfer correctness and persistence; host and extension only validate, translate, and display commands/events.

**Tech Stack:** Rust stable, Tokio, Reqwest, Serde, SQLx with SQLite, Chrome Manifest V3, Node.js built-in test runner, and local deterministic HTTP fixtures.

**Spec:** `docs/superpowers/specs/2026-10-13-download-manager-design.md`

## Global Constraints

- Use original code, branding, assets, internal data flows, and UI; do not copy Internet Download Manager proprietary implementation or design.
- Support only user-authorized HTTP(S) downloads in milestone one.
- Do not bypass DRM, paywalls, authentication controls, or other access restrictions.
- Never persist passwords, cookies, authorization headers, or other transient credentials in ordinary job records or logs.
- Do not execute downloaded files automatically.
- Validate URL scheme, destination paths, message shape, message size, redirect count, retry count, concurrency, and segment count.
- Surface authorization, policy, filesystem, integrity, server-capability, and network errors explicitly.
- Use controlled temporary files and atomic finalization; never silently overwrite an existing destination.
- Keep the engine API typed and versioned so host, extension, and future UI do not access persistence internals.

## Review Focus

- A server advertises ranges but ignores or changes `Content-Range`; the engine must fall back safely rather than corrupting the file. Test in Task 4.
- A request contains signed or sensitive query parameters; logs and durable records must redact them while the in-memory transfer still works. Test in Task 2.
- The process exits between checkpoint and segment write; restart must reconcile state and avoid false completion. Test in Task 5.
- Native Messaging receives malformed, oversized, or unexpected-origin messages; the host must reject them with a structured error and remain alive. Test in Task 7.
- A destination already exists or escapes the selected directory through traversal syntax; the engine must require an explicit conflict policy and reject unsafe paths. Test in Task 3.

---

## File and directory map

The project starts empty. Create these focused units:

- `Cargo.toml`: workspace members and shared dependency versions.
- `crates/download-core/Cargo.toml`: engine library dependencies.
- `crates/download-core/src/lib.rs`: public engine module exports.
- `crates/download-core/src/model.rs`: job, segment, state, command, event, and error types.
- `crates/download-core/src/validation.rs`: URL, destination, request-context, and redaction validation.
- `crates/download-core/src/store.rs`: schema, migrations, and durable job/checkpoint repository.
- `crates/download-core/src/transfer.rs`: probe, single-stream, ranged transfer, retry, pause, cancellation, and assembly logic.
- `crates/download-core/src/engine.rs`: command orchestration and event publication.
- `crates/download-core/tests/http_fixtures.rs`: deterministic local HTTP server helpers.
- `crates/download-core/tests/engine_tests.rs`: core integration tests.
- `crates/download-host/Cargo.toml`: Windows host dependencies.
- `crates/download-host/src/main.rs`: host process lifecycle and transport startup.
- `crates/download-host/src/native_messaging.rs`: framed Native Messaging input/output.
- `crates/download-host/src/ipc.rs`: versioned local command/event transport.
- `crates/download-host/tests/native_messaging_tests.rs`: host protocol tests.
- `extension/manifest.json`: Manifest V3 permissions and Native Messaging configuration.
- `extension/package.json`: extension test script using Node.js's built-in test runner.
- `extension/src/background.js`: user-approved download capture and message translation.
- `extension/src/native_host.js`: Native Messaging client and reconnect handling.
- `extension/src/protocol.js`: extension-side schema validation and redaction-safe status mapping.
- `extension/test/protocol.test.js`: extension protocol tests.
- `extension/test/background.test.js`: duplicate handoff and user-consent tests.
- `README.md`: build, test, local Native Messaging setup, and scope boundaries.

### Task 1: Bootstrap the workspace and protocol vocabulary

**Files:**
- Create: `Cargo.toml`
- Create: `crates/download-core/Cargo.toml`
- Create: `crates/download-core/src/lib.rs`
- Create: `crates/download-core/src/model.rs`
- Create: `crates/download-core/src/validation.rs`
- Test: `crates/download-core/src/model.rs` and `crates/download-core/src/validation.rs`

**Interfaces:**
- Produces `JobId`, `JobSpec`, `RequestContext`, `JobState`, `JobCommand`, `JobEvent`, `EngineError`, `ConflictPolicy`, and `ProtocolVersion`.
- Produces `validate_job_spec(&JobSpec) -> Result<(), EngineError>` and `redact_url(&Url) -> String`.

- [ ] **Step 1: Write failing model and validation tests**

  Add tests for accepted `http`/`https` URLs, rejected non-HTTP schemes, absolute destination validation, traversal rejection, explicit overwrite policy, bounded request-context fields, and URL redaction that preserves host/path but removes sensitive query values.

- [ ] **Step 2: Run the focused tests to verify they fail**

  Run `cargo test -p download-core model validation`.
  Expected: compilation or test failures because the workspace and types do not exist.

- [ ] **Step 3: Implement the workspace and typed protocol**

  Define serde-serializable types with stable field names and explicit error variants. Keep request context limited to in-memory headers needed for the user-authorized handoff; do not derive persistence serialization for secrets.

- [ ] **Step 4: Run the focused tests to verify they pass**

  Run `cargo test -p download-core`.
  Expected: PASS with validation and redaction tests.

- [ ] **Step 5: Commit**

  ```text
  git add Cargo.toml crates/download-core
  git commit -m "feat: bootstrap download engine protocol"
  ```

### Task 2: Add durable job storage and filesystem safety

**Files:**
- Modify: `crates/download-core/Cargo.toml`
- Create: `crates/download-core/src/store.rs`
- Modify: `crates/download-core/src/lib.rs`
- Test: `crates/download-core/tests/engine_tests.rs`

**Interfaces:**
- Consumes `JobSpec`, `JobState`, `Segment`, and `EngineError` from Task 1.
- Produces `JobStore::open(path)`, `create_job(&JobSpec)`, `get_job(JobId)`, `list_jobs()`, `update_checkpoint(JobId, Checkpoint)`, and `reconcile_unfinished_jobs()`.
- Produces application-controlled temporary-path creation and atomic-finalization helpers.

- [ ] **Step 1: Write failing persistence and path-safety tests**

  Test schema creation, job round-trip, checkpoint round-trip, restart visibility, unfinished-job reconciliation, redacted durable records, destination traversal rejection, existing-file conflict behavior, and atomic rename from a temporary file.

- [ ] **Step 2: Run tests to verify they fail**

  Run `cargo test -p download-core --test engine_tests`.
  Expected: FAIL because `JobStore` and checkpoint persistence are not implemented.

- [ ] **Step 3: Implement the store and migrations**

  Use SQLx with SQLite and a versioned schema containing job identity, source, destination, transfer mode, segment ranges, completed bytes, retry state, and schema version. Store no cookies, passwords, authorization headers, or raw sensitive request context. Use parameterized queries and application-controlled paths.

- [ ] **Step 4: Run tests to verify they pass**

  Run `cargo test -p download-core --test engine_tests`.
  Expected: PASS, including restart and filesystem safety tests.

- [ ] **Step 5: Commit**

  ```text
  git add crates/download-core
  git commit -m "feat: persist download jobs and checkpoints"
  ```

### Task 3: Implement metadata probing and single-stream transfers

**Files:**
- Modify: `crates/download-core/Cargo.toml`
- Create: `crates/download-core/src/transfer.rs`
- Create: `crates/download-core/src/engine.rs`
- Modify: `crates/download-core/src/lib.rs`
- Modify: `crates/download-core/tests/http_fixtures.rs`
- Modify: `crates/download-core/tests/engine_tests.rs`

**Interfaces:**
- Consumes `JobStore` and protocol types from Tasks 1-2.
- Produces `TransferEngine::new(config, store)`, `create_job`, `start_job`, `pause_job`, `resume_job`, `cancel_job`, `retry_job`, `get_job`, `list_jobs`, and `subscribe_events`.
- Produces `probe(url, request_context) -> ResourceMetadata` and `download_single(job_id)`.

- [ ] **Step 1: Write failing transfer tests**

  Add local fixture tests for full-file download, `Content-Disposition`, unknown content length, redirects within the limit, redirect-limit failure, pause/resume, cancellation, transient disconnect retry, and explicit authorization/server/filesystem errors.

- [ ] **Step 2: Run tests to verify they fail**

  Run `cargo test -p download-core --test engine_tests`.
  Expected: FAIL because the transfer engine and fixture server are not implemented.

- [ ] **Step 3: Implement probing and single-stream transfer**

  Use a bounded Reqwest/Tokio client. Prefer `HEAD` when useful and fall back to a controlled `GET`; classify status codes and transport errors; stream into the controlled temporary file; persist checkpoints at bounded intervals; publish progress events; and atomically finalize only after the expected transfer is complete.

- [ ] **Step 4: Run tests to verify they pass**

  Run `cargo test -p download-core --test engine_tests`.
  Expected: PASS for all single-stream and error-classification scenarios.

- [ ] **Step 5: Commit**

  ```text
  git add crates/download-core
  git commit -m "feat: add resumable single-stream transfers"
  ```

### Task 4: Add ranged segmented downloads and safe fallback

**Files:**
- Modify: `crates/download-core/src/transfer.rs`
- Modify: `crates/download-core/src/engine.rs`
- Modify: `crates/download-core/tests/http_fixtures.rs`
- Modify: `crates/download-core/tests/engine_tests.rs`

**Interfaces:**
- Consumes `ResourceMetadata`, `TransferEngine`, and checkpoint APIs from Tasks 2-3.
- Produces bounded segment scheduling, segment checkpoint updates, and assembly using the existing job/event API; no new host-facing protocol is required.

- [ ] **Step 1: Write failing range-transfer tests**

  Test a range-capable server with multiple segments, segment retry, out-of-order completion, server range-ignore fallback to single stream, malformed `Content-Range`, mismatched total size, and a server that advertises ranges but returns inconsistent bytes.

- [ ] **Step 2: Run tests to verify they fail**

  Run `cargo test -p download-core --test engine_tests range`.
  Expected: FAIL because segmented scheduling and response validation are absent.

- [ ] **Step 3: Implement bounded segmentation**

  Select a configured maximum segment count only when metadata supports ranges and total size is known. Validate every `206` response and `Content-Range`; treat invalid or ignored ranges as an explicit safe fallback to single-stream transfer; assemble only verified ranges; preserve pause/cancel/retry behavior.

- [ ] **Step 4: Run tests to verify they pass**

  Run `cargo test -p download-core --test engine_tests range`.
  Expected: PASS with no corrupted output for any fixture.

- [ ] **Step 5: Commit**

  ```text
  git add crates/download-core
  git commit -m "feat: add safe ranged downloads"
  ```

### Task 5: Add restart recovery, duplicate prevention, and event semantics

**Files:**
- Modify: `crates/download-core/src/store.rs`
- Modify: `crates/download-core/src/engine.rs`
- Modify: `crates/download-core/tests/engine_tests.rs`

**Interfaces:**
- Consumes all engine and store APIs from Tasks 1-4.
- Produces deterministic duplicate identity for source/destination, explicit recovery states, and event ordering guarantees for queued, active, paused, failed, completed, and canceled jobs.

- [ ] **Step 1: Write failing recovery and duplicate tests**

  Force process-like interruption after checkpointing and before finalization; reopen the store; assert the job is recoverable and never marked complete early. Add duplicate source/destination tests and event-order assertions.

- [ ] **Step 2: Run tests to verify they fail**

  Run `cargo test -p download-core --test engine_tests recovery duplicate events`.
  Expected: FAIL because reconciliation and duplicate identity are incomplete.

- [ ] **Step 3: Implement reconciliation and event rules**

  Reconcile temporary files against persisted checkpoints, invalidate ambiguous segments, resume only when safe, reject or explicitly surface duplicates according to `ConflictPolicy`, and ensure terminal events are emitted only after finalization or an explicit terminal failure.

- [ ] **Step 4: Run tests to verify they pass**

  Run `cargo test -p download-core --test engine_tests recovery duplicate events`.
  Expected: PASS.

- [ ] **Step 5: Commit**

  ```text
  git add crates/download-core
  git commit -m "feat: recover jobs and prevent duplicate downloads"
  ```

### Task 6: Define the versioned host protocol and Windows host shell

**Files:**
- Create: `crates/download-host/Cargo.toml`
- Create: `crates/download-host/src/main.rs`
- Create: `crates/download-host/src/ipc.rs`
- Modify: `Cargo.toml`
- Create: `crates/download-host/tests/ipc_tests.rs`

**Interfaces:**
- Consumes the typed engine API from Tasks 1-5.
- Produces a versioned local command/event envelope with `ProtocolVersion`, bounded payload sizes, structured errors, and host lifecycle handling.

- [ ] **Step 1: Write failing host protocol tests**

  Test valid command decoding, unknown protocol version, malformed JSON, oversized payload, structured engine-error mapping, event serialization, and clean shutdown/cancellation.

- [ ] **Step 2: Run tests to verify they fail**

  Run `cargo test -p download-host`.
  Expected: FAIL because the host crate and protocol transport are absent.

- [ ] **Step 3: Implement the host shell**

  Add a Windows-oriented binary that starts the core store/engine, exposes the versioned local transport, validates every message before dispatch, and keeps host-specific lifecycle code out of the transfer engine. Keep the first implementation buildable on non-Windows for protocol tests where practical.

- [ ] **Step 4: Run tests and a compile check**

  Run `cargo test -p download-host` and `cargo check --workspace`.
  Expected: PASS with a clean workspace check.

- [ ] **Step 5: Commit**

  ```text
  git add Cargo.toml crates/download-host
  git commit -m "feat: add versioned download host protocol"
  ```

### Task 7: Implement Native Messaging and the Chrome extension

**Files:**
- Create: `crates/download-host/src/native_messaging.rs`
- Create: `crates/download-host/tests/native_messaging_tests.rs`
- Create: `extension/manifest.json`
- Create: `extension/package.json`
- Create: `extension/src/background.js`
- Create: `extension/src/native_host.js`
- Create: `extension/src/protocol.js`
- Create: `extension/test/protocol.test.js`
- Create: `extension/test/background.test.js`

**Interfaces:**
- Consumes the host protocol from Task 6 and the user-authorized handoff boundary from the spec.
- Produces framed Native Messaging read/write, user-consent-gated handoff, duplicate prevention, progress mapping, reconnect handling, and no sensitive-data logging.

- [ ] **Step 1: Write failing Native Messaging and extension tests**

  Test 4-byte little-endian framing, malformed/oversized messages, valid handoff payloads, unsupported schemes, duplicate request suppression, user-consent requirement, event-to-progress mapping, reconnect behavior, and redaction of sensitive URL query values in logs.

- [ ] **Step 2: Run tests to verify they fail**

  Run `cargo test -p download-host native_messaging`; then run the extension test command defined in `extension/package.json`.
  Expected: FAIL because the protocol and extension files are absent.

- [ ] **Step 3: Implement the Native Messaging bridge**

  Read and write bounded framed messages, validate the protocol envelope, reject malformed data without terminating the host, and dispatch only the safe subset of request metadata. Do not persist credentials or write raw sensitive headers to logs.

- [ ] **Step 4: Implement the Manifest V3 extension**

  Request only the minimum permissions needed for user-approved download handoff. Keep download capture and consent in the background service worker; keep the Native Messaging client reconnectable; translate host events to extension state without implementing transfer logic in JavaScript.

- [ ] **Step 5: Run both test suites**

  Run `cargo test -p download-host native_messaging` and the extension test command.
  Expected: PASS.

- [ ] **Step 6: Commit**

  ```text
  git add crates/download-host extension
  git commit -m "feat: connect Chrome download handoff"
  ```

### Task 8: Document local setup and milestone verification

**Files:**
- Create: `README.md`
- Modify: `crates/download-core/tests/engine_tests.rs`
- Modify: `crates/download-host/tests/native_messaging_tests.rs`
- Modify: `extension/test/background.test.js`

**Interfaces:**
- Consumes the complete first-milestone engine, host, and extension.
- Produces reproducible developer setup, local Native Messaging registration instructions, and an end-to-end acceptance test checklist.

- [ ] **Step 1: Add the end-to-end acceptance test**

  Exercise a local single-stream fixture and a range-capable fixture through host protocol commands; assert durable queueing, pause/resume, restart recovery, correct final bytes, and explicit forced-disconnect recovery.

- [ ] **Step 2: Run the complete validation suite**

  Run `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and the extension test command.
  Expected: PASS with no warnings promoted to errors.

- [ ] **Step 3: Write README setup instructions**

  Document Rust/toolchain prerequisites, test commands, host build/run steps, Chrome extension loading, Native Messaging registration, data directories, limitations, and the explicit authorization/anti-circumvention boundary.

- [ ] **Step 4: Re-run the documented commands**

  Run every command copied into `README.md`.
  Expected: PASS or a clearly documented Windows-only step that cannot run in the current environment.

- [ ] **Step 5: Commit**

  ```text
  git add README.md crates/download-core/tests crates/download-host/tests extension/test
  git commit -m "docs: document first milestone setup and verification"
  ```

## Execution notes

The current folder is not a Git repository, so the commit steps above cannot
run until repository history is initialized or the project is moved into a
repository. Do not silently skip those commits: either initialize the
repository as an explicit user-approved setup action or retain the changes
uncommitted for review.
