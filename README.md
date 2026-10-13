# Open Download Manager

An original, open-source Windows download manager foundation with a Rust
transfer core and Chrome Manifest V3 handoff. This project is not affiliated
with or derived from Internet Download Manager.

## Current milestone

The current milestone provides:

- HTTP(S) job validation and redaction-safe persistence.
- A local SQLite job store with checkpoints and duplicate prevention.
- Single-stream downloads with temporary-file finalization.
- Content-Range validation primitives for future segmented transfers.
- A versioned host message envelope and Chrome Native Messaging framing.
- A minimal Chrome extension protocol layer.

Segment scheduling, restart reconciliation, polished desktop UI, scheduling,
and broader browser/media support remain subsequent milestones.

## Build and test

Install the stable Rust toolchain and Node.js 20 or newer, then run:

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
Push-Location extension
node --test
Pop-Location
```

## Local Chrome setup

1. Build the `download-host` binary with `cargo build -p download-host`.
2. Create a per-user Native Messaging host manifest that points to the
   resulting executable and lists the Chrome extension ID.
3. Register that manifest under the Windows per-user Native Messaging host
   registry key.
4. Load `extension/` as an unpacked Manifest V3 extension in Chrome.

The host registration is intentionally not automated yet because the final
installer and signed release identity are not defined.

## Safety and authorization

The manager is intended for downloads the user is authorized to access. It
supports HTTP(S) sources exposed by the browser or supplied by the user. It
does not bypass DRM, paywalls, authentication controls, or other access
restrictions, and it never executes downloaded files automatically. Do not
place passwords, cookies, or authorization headers in logs or durable job
records.
