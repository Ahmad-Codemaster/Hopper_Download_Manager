# Hopper Desktop UI Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the current single-screen React dashboard with a native-style Hopper desktop shell containing searchable categorized downloads, real event-driven progress, and a dedicated download detail view.

**Architecture:** Keep the current Tauri command/event architecture and make the frontend responsible for presentation, navigation, filtering, and derived display values only. Split the current monolithic `main.tsx` into focused components and add only backend commands that are required to make a requested control truthful; unsupported file-management actions remain unavailable rather than simulated.

**Tech Stack:** React 18, TypeScript, Tauri v2 invoke/events, CSS, `lucide-react`, Node test runner, TypeScript compiler, Vite.

**Spec:** The approved Phase 4 UI requirements in the user request; implementation decisions are captured in this plan because no separate design spec exists.

## Global Constraints

- Preserve the existing Tauri commands, extension/native-host integration, queue behavior, persistence, and download event semantics.
- Display only values derived from `Job` and `JobEvent`; unknown values render as `Unknown` or are omitted.
- Do not fabricate segment activity, speed, ETA, completion, file operations, or backend support.
- Do not label cancellation as deletion or remove a database record while a transfer continues.
- Keep the UI desktop-oriented: compact shell, persistent sidebar, restrained surfaces, keyboard focus, and minimum window dimensions.
- Do not add FFmpeg, HLS/DASH assembly, or unrelated backend refactors.
- Do not commit, push, or release changes.

## Review Focus

- Unknown-size and zero-byte jobs must show an honest indeterminate/zero state and no fabricated ETA; pin in `DownloadRow` tests.
- Failed commands must leave the displayed job state unchanged and show an actionable error; pin in `App` action tests.
- Back navigation must preserve the prior category/search selection; pin in navigation tests.
- Unsupported Delete/Open/Show-folder controls must be absent or explicitly disabled, never invoke nonexistent commands; pin in action-visibility tests.
- Progress events for jobs not currently in the list must not create fabricated records; pin in event-reducer tests.

---

### Task 1: Add UI dependency and typed presentation helpers

**Files:**
- Modify: `ui/package.json`
- Modify: `ui/package-lock.json`
- Create: `ui/src/lib/format.ts`
- Create: `ui/src/lib/jobView.ts`
- Test: `ui/test/jobView.test.js`

**Interfaces:**
- `formatBytes(value: number | null | undefined): string`
- `formatSpeed(value: number | undefined): string`
- `formatEta(completed: number, total: number | null, speed: number | undefined): string | null`
- `getFileKind(destination: string): "archive" | "video" | "audio" | "image" | "document" | "file"`
- `matchesCategory(job: Job, category: Category): boolean`
- `type Category = "all" | "downloading" | "queued" | "completed" | "failed" | "paused"`

- [ ] Add `lucide-react` as the icon dependency using the existing package manager lockfile.
- [ ] Write tests for byte/speed/ETA formatting, unknown totals, file-kind mapping, and all category predicates.
- [ ] Run `npm --prefix ui run build` and the focused test command to verify the tests initially fail for missing helpers.
- [ ] Implement the helpers without locale-dependent or fabricated values.
- [ ] Run the focused tests and TypeScript build; expected result is green.

### Task 2: Create event-driven job state utilities

**Files:**
- Create: `ui/src/lib/jobs.ts`
- Test: `ui/test/jobs.test.js`

**Interfaces:**
- `applyJobEvent(jobs: Job[], event: JobEvent): Job[]`
- `filterJobs(jobs: Job[], query: string, category: Category): Job[]`
- `getJobCounts(jobs: Job[]): Record<Category, number>`

- [ ] Write tests proving existing jobs update from real events, unknown event IDs are ignored, failed events retain byte counts, and filtering combines search/category correctly.
- [ ] Run the focused test command and observe the expected failure before implementation.
- [ ] Implement immutable reducers and filters using the existing `Job`/`JobEvent` shapes.
- [ ] Run focused tests and `npm --prefix ui run build`.

### Task 3: Extend the API boundary only for truthful UI behavior

**Files:**
- Modify: `ui/src/lib/api.ts`
- Modify: `src-tauri/src/main.rs` only if a command is added
- Test: `ui/test/api-contract.test.js` only for pure request-shape helpers if needed

**Interfaces:**
- Preserve existing `listJobs`, `createAndStart`, `controlJob`, `refreshSource`, `onProgress`, settings, and archive-preview signatures.
- Add no delete/open/show-folder command unless implementation and platform behavior are verified.

- [ ] Audit each requested action against the existing command list.
- [ ] Keep Delete, Open file, and Show in folder unavailable in the UI because no supported backend command exists.
- [ ] Ensure command failures remain rejected promises so callers can show errors; do not swallow errors in new API wrappers.
- [ ] Run TypeScript build and existing Rust tests if `main.rs` is touched.

### Task 4: Build the desktop shell and navigation components

**Files:**
- Create: `ui/src/components/AppShell.tsx`
- Create: `ui/src/components/Sidebar.tsx`
- Create: `ui/src/components/TitleBar.tsx`
- Create: `ui/src/components/EmptyState.tsx`
- Create: `ui/src/components/StatusNotice.tsx`
- Modify: `ui/src/main.tsx`
- Modify: `ui/src/styles.css`

**Interfaces:**
- `AppShell` accepts `children`, current page title, optional back callback, and sidebar category/count data.
- `Sidebar` emits `onSelect(category)` and displays counts from real jobs.
- `TitleBar` emits `onBack` only when a detail view is active.

- [ ] Add component tests or DOM-level assertions for category navigation, title/back behavior, focusable controls, and empty/error states using the project’s available test setup.
- [ ] Implement the compact dark desktop shell with persistent sidebar, title bar, Add URL button, and responsive minimum-safe layout.
- [ ] Preserve search/category state in `App` while switching between list and detail views.
- [ ] Add visible focus styles, `aria-label`s, and tooltips for icon buttons.
- [ ] Run UI type-check and bundle.

### Task 5: Implement download rows and contextual actions

**Files:**
- Create: `ui/src/components/DownloadRow.tsx`
- Create: `ui/src/components/DownloadList.tsx`
- Modify: `ui/src/main.tsx`
- Modify: `ui/src/styles.css`
- Test: `ui/test/downloadRow.test.js`

**Interfaces:**
- `DownloadRow` receives `job: Job`, `onSelect`, and `onAction(command, jobId)`.
- `DownloadList` receives filtered jobs, loading/error state, and action callbacks.

- [ ] Write tests for all job states, visible progress/unknown totals, speed/ETA derivation, failed error copy, and contextual action visibility.
- [ ] Run focused tests to verify the missing component behavior fails.
- [ ] Implement file/type icons, source host extraction without exposing query strings, status badges, progress bars, byte totals, speed, ETA, and compact hover actions.
- [ ] Show only real supported actions: pause for Active, resume for Paused/Queued where applicable, retry for Failed, cancel for Active/Queued, and archive preview only for eligible ZIP jobs.
- [ ] Do not render or invoke Delete/Open/Show-folder actions until backend support exists.
- [ ] Ensure action promise failures are reported through the parent error notice and do not optimistically mutate job state.
- [ ] Run focused tests, UI build, and bundle.

### Task 6: Add URL flow and preserve existing archive preview

**Files:**
- Create: `ui/src/components/AddDownloadDialog.tsx`
- Create: `ui/src/components/ArchivePreviewModal.tsx`
- Modify: `ui/src/main.tsx`
- Modify: `ui/src/styles.css`

**Interfaces:**
- `AddDownloadDialog` receives `open`, `onClose`, and `onSubmit(url)`.
- `ArchivePreviewModal` receives archive name, entries, loading state, and `onClose`.

- [ ] Test invalid schemes, empty input, submit failure, Escape/backdrop close behavior, and archive loading/error states.
- [ ] Implement the existing `createAndStart` flow with clear error reporting and no optimistic Active record until the command succeeds.
- [ ] Keep archive preview wired to `previewZipArchive` and preserve its current behavior.
- [ ] Run UI build and bundle.

### Task 7: Implement the dedicated download detail view

**Files:**
- Create: `ui/src/components/DownloadDetails.tsx`
- Modify: `ui/src/main.tsx`
- Modify: `ui/src/styles.css`
- Test: `ui/test/downloadDetails.test.js`

**Interfaces:**
- `DownloadDetails` receives `job`, `onBack`, and `onAction(command, jobId)`.

- [ ] Test filename/source/status, large progress indicator, bytes/speed/ETA, unknown totals, supported actions, and back callback.
- [ ] Implement destination, sanitized source host/path, current status, progress, real byte counts, speed, ETA, and a collapsed technical section using only available fields.
- [ ] Display start time/elapsed as `Unknown` or omit because the current backend model does not expose them.
- [ ] Display HTTP status/diagnostics only when a future backend event supplies them; do not infer them from UI state.
- [ ] Show an honest “connection activity unavailable” panel rather than fabricated segments.
- [ ] Run focused tests and UI build.

### Task 8: Integrate state, error handling, and keyboard behavior

**Files:**
- Modify: `ui/src/main.tsx`
- Modify: `ui/src/lib/jobs.ts`
- Modify: `ui/src/styles.css`
- Test: `ui/test/appState.test.js`

**Interfaces:**
- `App` owns `jobs`, `selectedJobId`, `category`, `query`, `loading`, `error`, and archive-preview state.
- `onProgress` updates jobs through `applyJobEvent`.

- [ ] Test event updates, command rejection behavior, retry/refresh flow, selection removal after cancellation, and list-state restoration after Back.
- [ ] Integrate initial loading, event subscription cleanup, command error notices, and reload-after-command behavior.
- [ ] Add keyboard shortcuts for Escape/back and Enter submission where appropriate.
- [ ] Ensure canceled jobs remain visible with Canceled status until the user changes category; do not delete records.
- [ ] Run all frontend tests, TypeScript build, and production bundle.

### Task 9: Final verification

**Files:**
- Modify: none unless verification exposes a directly related defect.

- [ ] Run `npm --prefix ui run build`.
- [ ] Run `npm --prefix ui run bundle`.
- [ ] Run the UI test command and report exact results.
- [ ] Run `cargo test --workspace` because the UI depends on the Rust command/event contract.
- [ ] Inspect the final diff for fabricated controls, unsupported commands, query-string leakage, and regressions.
- [ ] Do not commit or push.
