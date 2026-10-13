# Tauri Download Manager MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a runnable Windows MVP with a Tauri desktop UI, asynchronous Rust download engine, durable job controls, and consent-gated Chrome handoff.

**Architecture:** The Rust core becomes an asynchronous Tokio engine with a single owned SQLite store and worker manager. The Windows host owns that engine and exposes one typed command/event protocol to Native Messaging and Tauri; the Tauri frontend renders state and dispatches commands without touching persistence. The Chrome extension remains a thin, consent-gated client.

**Tech Stack:** Rust stable, Tokio, Reqwest, Serde/Serde JSON, rusqlite with bundled SQLite, Tauri 2, Vite, TypeScript, React, Chrome Manifest V3, Node.js built-in test runner, and local deterministic HTTP fixtures.

**Spec:** `docs/superpowers/specs/2026-10-13-tauri-engine-mvp-design.md`

## Global Constraints

- “This is an original implementation. It must not copy Internet Download Manager source code, branding, proprietary assets, exact internal data flows, or pixel-identical design.”
- “Accept only HTTP and HTTPS URLs.”
- “The product is for user-authorized downloads. It must not implement DRM, paywall, login, credential, or other access-control circumvention.”
- “Operational signed URLs and transient request headers remain in memory only.”
- “An interrupted active job is recovered as `Paused` or an explicit recovery error, never `Completed`.”
- “Closing the window disconnects the UI only; it does not cancel jobs.”
- “Never execute the downloaded file automatically.”
- “Errors must remain visible and typed; no broad catch may turn a failure into a success-shaped result.”
- “Multi-segment transfer implementation” is out of scope for this milestone.

## Review Focus

- A pause/cancel arrives while a response is streaming; the worker must stop cooperatively, persist a non-terminal checkpoint, and never emit `Completed`. Test in Task 2.
- The UI disconnects or reconnects while a job is active; the host must retain the worker and replay authoritative state without duplicate workers. Test in Task 4.
- A signed URL is needed for an active job but must not be persisted; the engine must use the in-memory operational URL and require reauthorization after restart. Test in Task 2.
- Two clients issue conflicting commands concurrently; the engine must serialize state transitions and return typed conflict errors. Test in Task 3.
- A Native Messaging or Tauri payload is malformed, oversized, or from an unsupported protocol version; the host must return a structured error and remain available. Test in Task 4.

---

## File and directory map

- `crates/download-core/src/model.rs`: async job commands, summaries, events, errors, and state transitions.
- `crates/download-core/src/store.rs`: SQLite schema, migrations, job queries, checkpoints, and durable destination resolution.
- `crates/download-core/src/engine.rs`: Tokio worker manager, command serialization, lifecycle, progress, pause/cancel, retry, and recovery.
- `crates/download-core/src/transfer.rs`: streamed HTTP transfer and response/error classification.
- `crates/download-core/tests/async_engine_tests.rs`: local-fixture engine behavior and restart tests.
- `crates/download-host/src/ipc.rs`: shared versioned command/event envelopes and validation.
- `crates/download-host/src/native_messaging.rs`: bounded Chrome Native Messaging framing and dispatch.
- `crates/download-host/src/main.rs`: long-lived host startup and engine routing.
- `crates/download-host/tests/host_routing_tests.rs`: protocol, routing, and reconnect tests.
- `src-tauri/Cargo.toml`: Tauri application dependencies.
- `src-tauri/src/main.rs`: Tauri startup, shared engine state, command registration, and event bridge.
- `src-tauri/src/commands.rs`: typed frontend command adapters.
- `src-tauri/src/events.rs`: engine-event to Tauri-event mapping.
- `src-tauri/tauri.conf.json`: Windows window and build configuration.
- `src-tauri/capabilities/default.json`: minimum Tauri permissions.
- `ui/package.json`: Vite/React scripts and Node test script.
- `ui/src/App.tsx`: application shell and filter state.
- `ui/src/components/DownloadList.tsx`: download rows and action buttons.
- `ui/src/components/AddDownloadDialog.tsx`: URL/destination form and validation display.
- `ui/src/lib/api.ts`: typed Tauri invoke/listener client.
- `ui/src/lib/types.ts`: frontend protocol types.
- `ui/test/*.test.tsx`: UI rendering, filters, and command dispatch tests.
- `extension/src/background.js`: consent-gated handoff and duplicate suppression.
- `extension/src/protocol.js`: extension payload validation and redaction.
- `extension/test/*.test.js`: extension behavior tests.
- `README.md`: MVP build, run, Native Messaging, and Tauri setup.

### Task 1: Define the asynchronous engine protocol and migration

**Files:**
- Modify: `crates/download-core/src/model.rs`
- Modify: `crates/download-core/src/store.rs`
- Modify: `crates/download-core/src/lib.rs`
- Create: `crates/download-core/tests/async_engine_tests.rs`

**Interfaces:**
- Produces `JobCommand`, `JobSummary`, `ProgressEvent`, `JobError`, `JobFilter`, and explicit transition validation.
- Produces store methods `list_jobs(filter)`, `load_checkpoint(job_id)`, `save_checkpoint(job_id, checkpoint)`, `resolve_destination(spec)`, and `reconcile_jobs()`.
- Preserves redacted durable source identity while accepting an in-memory operational URL separately.

- [ ] **Step 1: Write failing model/store tests**

  Test durable state transitions, `Active -> Paused` recovery, expected-size and retry persistence, destination resolution before transfer, schema migration, and rejection of a completed row whose final file is missing.

- [ ] **Step 2: Run tests to verify they fail**

  Run: `cargo test -p download-core --test async_engine_tests`
  Expected: FAIL because the async protocol and recovery APIs are not implemented.

- [ ] **Step 3: Implement the model and store APIs**

  Add typed transition errors, migration versioning, durable destination selection, checkpoint records, and startup reconciliation. Never persist signed operational URLs, cookies, passwords, or authorization headers.

- [ ] **Step 4: Run tests to verify they pass**

  Run: `cargo test -p download-core --test async_engine_tests`
  Expected: PASS.

- [ ] **Step 5: Commit**

  ```text
  git add crates/download-core
  git commit -m "feat: define async engine state and recovery model"
  ```

### Task 2: Implement async streamed transfer workers

**Files:**
- Modify: `crates/download-core/Cargo.toml`
- Modify: `crates/download-core/src/transfer.rs`
- Modify: `crates/download-core/src/engine.rs`
- Modify: `crates/download-core/tests/async_engine_tests.rs`

**Interfaces:**
- Consumes the model/store APIs from Task 1.
- Produces `Engine::new(config)`, `create_job(spec, operational_source)`, `start_job(id)`, `pause_job(id)`, `resume_job(id)`, `cancel_job(id)`, `retry_job(id)`, `list_jobs(filter)`, and `subscribe_events()`.
- Produces ordered events containing job ID, state, completed bytes, optional total, speed, retry count, and typed error.

- [ ] **Step 1: Write failing worker tests**

  Use deterministic local HTTP fixtures to test streamed progress, pause, resume, cancel, transient retry, redirect limits, HTTP failures, signed URL use without durable leakage, restart reauthorization, and no false completion after interruption.

- [ ] **Step 2: Run tests to verify they fail**

  Run: `cargo test -p download-core --test async_engine_tests`
  Expected: FAIL because the current blocking engine has no worker manager or lifecycle commands.

- [ ] **Step 3: Implement the Tokio worker manager**

  Replace blocking transfer execution with bounded Tokio tasks and per-job control channels. Stream response chunks into controlled `.part` files, checkpoint at bounded intervals, calculate recent speed, classify retryable errors, and emit terminal events only after verification and atomic finalization.

- [ ] **Step 4: Run tests to verify they pass**

  Run: `cargo test -p download-core --test async_engine_tests`
  Expected: PASS with no warnings.

- [ ] **Step 5: Commit**

  ```text
  git add crates/download-core
  git commit -m "feat: add asynchronous controllable download workers"
  ```

### Task 3: Build the shared host command/event protocol

**Files:**
- Modify: `crates/download-host/src/ipc.rs`
- Modify: `crates/download-host/src/lib.rs`
- Create: `crates/download-host/tests/host_routing_tests.rs`

**Interfaces:**
- Consumes the engine API from Tasks 1-2.
- Produces versioned `CommandEnvelope`, `EventEnvelope`, `HostError`, and `dispatch_command(engine, envelope)`.
- Supports `create`, `start`, `pause`, `resume`, `cancel`, `retry`, `list`, and event subscription commands.

- [ ] **Step 1: Write failing routing tests**

  Test valid command dispatch, event serialization, unknown command, unknown protocol version, malformed JSON, oversized payload, concurrent command ordering, and structured mapping of engine errors.

- [ ] **Step 2: Run tests to verify they fail**

  Run: `cargo test -p download-host --test host_routing_tests`
  Expected: FAIL because the host protocol only round-trips an opaque payload today.

- [ ] **Step 3: Implement typed envelopes and dispatch**

  Define bounded serde payloads, validate protocol version and command fields before engine calls, serialize typed events, and preserve errors without exposing secrets or database internals.

- [ ] **Step 4: Run tests to verify they pass**

  Run: `cargo test -p download-host --test host_routing_tests`
  Expected: PASS.

- [ ] **Step 5: Commit**

  ```text
  git add crates/download-host
  git commit -m "feat: add typed host command and event routing"
  ```

### Task 4: Connect the long-lived Windows host and Native Messaging

**Files:**
- Modify: `crates/download-host/src/native_messaging.rs`
- Modify: `crates/download-host/src/main.rs`
- Modify: `crates/download-host/Cargo.toml`
- Modify: `crates/download-host/tests/host_routing_tests.rs`

**Interfaces:**
- Consumes `dispatch_command` and event envelopes from Task 3.
- Produces a long-lived stdin/stdout host loop, recoverable protocol errors, engine event fan-out, and host lifecycle handling.

- [ ] **Step 1: Write failing host-loop tests**

  Test valid commands, malformed messages followed by valid messages, oversized frames, EOF, unsupported protocol version, event delivery, and host survival after recoverable errors.

- [ ] **Step 2: Run tests to verify they fail**

  Run: `cargo test -p download-host --test host_routing_tests`
  Expected: FAIL because the current loop only echoes envelopes and does not own an engine.

- [ ] **Step 3: Implement host startup and dispatch loop**

  Initialize one engine instance, read bounded Native Messaging frames, route typed commands, write structured responses/events, continue after recoverable message errors, and stop only on EOF or unrecoverable transport failure. Keep workers alive after UI disconnect.

- [ ] **Step 4: Run tests and compile check**

  Run: `cargo test -p download-host` and `cargo clippy -p download-host --all-targets -- -D warnings`
  Expected: PASS.

- [ ] **Step 5: Commit**

  ```text
  git add crates/download-host
  git commit -m "feat: connect long-lived Windows download host"
  ```

### Task 5: Add the Tauri desktop shell and typed commands

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/commands.rs`
- Create: `src-tauri/src/events.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/default.json`
- Modify: `Cargo.toml`
- Create: `ui/package.json`
- Create: `ui/tsconfig.json`
- Create: `ui/vite.config.ts`
- Create: `ui/src/lib/types.ts`
- Create: `ui/src/lib/api.ts`
- Create: `ui/test/api.test.ts`

**Interfaces:**
- Consumes the shared host protocol from Tasks 3-4.
- Produces Tauri commands `list_jobs`, `create_job`, `start_job`, `pause_job`, `resume_job`, `cancel_job`, and `retry_job`.
- Produces frontend listeners for `download://progress`, `download://state`, and `download://error`.

- [ ] **Step 1: Write failing adapter tests**

  Test frontend command payload shapes, response decoding, event decoding, invalid event rejection, and that the Tauri adapter does not expose SQLite or filesystem internals.

- [ ] **Step 2: Run tests to verify they fail**

  Run: `Push-Location ui; npm test -- --run; Pop-Location`
  Expected: FAIL because the Tauri/UI workspace does not exist.

- [ ] **Step 3: Implement Tauri startup and adapters**

  Register typed commands, share one engine state, forward engine events to the frontend, configure a Windows window and minimum capabilities, and ensure closing the window does not cancel jobs.

- [ ] **Step 4: Run tests and build checks**

  Run: `Push-Location ui; npm test -- --run; Pop-Location` and `cargo check --workspace`
  Expected: PASS.

- [ ] **Step 5: Commit**

  ```text
  git add Cargo.toml src-tauri ui
  git commit -m "feat: add Tauri command and event shell"
  ```

### Task 6: Build the modern download-list UI

**Files:**
- Create: `ui/index.html`
- Create: `ui/src/main.tsx`
- Create: `ui/src/App.tsx`
- Create: `ui/src/components/DownloadList.tsx`
- Create: `ui/src/components/AddDownloadDialog.tsx`
- Create: `ui/src/components/StatusSidebar.tsx`
- Create: `ui/src/styles.css`
- Create: `ui/test/App.test.tsx`
- Modify: `ui/src/lib/api.ts`

**Interfaces:**
- Consumes Tauri command/event adapters from Task 5.
- Produces the approved UI: All/Active/Paused/Completed/Failed filters, add form, progress/speed display, per-job controls, and redacted inline errors.

- [ ] **Step 1: Write failing UI tests**

  Test rendering empty/loading/error states, filter counts, progress updates, add URL validation, start/pause/resume/cancel/retry dispatch, and redacted error display.

- [ ] **Step 2: Run tests to verify they fail**

  Run: `Push-Location ui; npm test -- --run; Pop-Location`
  Expected: FAIL because the React components do not exist.

- [ ] **Step 3: Implement the UI**

  Use accessible semantic controls and a focused layout: sidebar filters, download table, add dialog, and per-row actions. Keep transfer logic out of React and render authoritative host events into local state.

- [ ] **Step 4: Run UI tests and production build**

  Run: `Push-Location ui; npm test -- --run; npm run build; Pop-Location`
  Expected: PASS and a production bundle is generated.

- [ ] **Step 5: Commit**

  ```text
  git add ui
  git commit -m "feat: add modern download manager UI"
  ```

### Task 7: Finish consent-gated Chrome handoff

**Files:**
- Modify: `extension/src/background.js`
- Modify: `extension/src/protocol.js`
- Create: `extension/src/consent.js`
- Modify: `extension/test/protocol.test.js`
- Create: `extension/test/consent.test.js`
- Modify: `extension/manifest.json`

**Interfaces:**
- Consumes the host command envelope from Task 3 and existing Native Messaging client.
- Produces explicit consent action/state, validated HTTP(S) handoff, duplicate suppression, and progress/status mapping.

- [ ] **Step 1: Write failing extension tests**

  Test that downloads are not automatically handed off, explicit consent creates exactly one command, unsupported schemes are rejected, duplicates are suppressed, and sensitive query values are never logged.

- [ ] **Step 2: Run tests to verify they fail**

  Run: `Push-Location extension; npm test; Pop-Location`
  Expected: FAIL because the consent action and typed command payload are incomplete.

- [ ] **Step 3: Implement consent flow**

  Add an explicit extension action or confirmation state, send only validated handoff commands, preserve no credentials, and map host responses/events to extension-visible status.

- [ ] **Step 4: Run extension tests**

  Run: `Push-Location extension; npm test; Pop-Location`
  Expected: PASS.

- [ ] **Step 5: Commit**

  ```text
  git add extension
  git commit -m "feat: complete consent-gated Chrome handoff"
  ```

### Task 8: Add end-to-end verification and documentation

**Files:**
- Modify: `crates/download-core/tests/async_engine_tests.rs`
- Modify: `crates/download-host/tests/host_routing_tests.rs`
- Create: `tests/e2e_mvp.rs`
- Modify: `README.md`
- Modify: `.gitignore`

**Interfaces:**
- Consumes the complete engine, host, Tauri, and extension surfaces from Tasks 1-7.
- Produces reproducible local-fixture acceptance verification and Windows setup documentation.

- [ ] **Step 1: Write the failing end-to-end acceptance test**

  Exercise UI-equivalent create/start commands through the host, assert progress and completion events, pause/resume/cancel behavior, forced interruption recovery, Chrome-style consent handoff, and correct final bytes without third-party sites.

- [ ] **Step 2: Run the end-to-end test to verify it fails**

  Run: `cargo test --test e2e_mvp`
  Expected: FAIL until all command, event, and worker surfaces are connected.

- [ ] **Step 3: Implement the test harness and documentation**

  Add deterministic fixture setup, document Rust/Node/Tauri prerequisites, Native Messaging registration, development commands, data directories, limitations, and the explicit authorization boundary.

- [ ] **Step 4: Run the complete verification suite**

  Run: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `Push-Location ui; npm test -- --run; npm run build; Pop-Location`, and `Push-Location extension; npm test; Pop-Location`.
  Expected: PASS with no warnings promoted to errors.

- [ ] **Step 5: Commit**

  ```text
  git add README.md .gitignore crates tests ui extension
  git commit -m "test: verify and document Tauri download manager MVP"
  ```
