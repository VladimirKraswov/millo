# ADR 0042: Structured diagnostic audit log

## Status

Accepted.

## Context

Run journals retain bounded recovery evidence, but they are intentionally too
sparse to explain an operator failure. Terminal output was ephemeral and did
not correlate connection, controller, preflight, safety, and sender events. A
failed engraving therefore could not be reconstructed after closing Millo.

Logging must not delay realtime commands, controller polling, or G-code FIFO
dispatch. It must remain useful after a process or power failure, avoid storing
an entire source program, and expose enough structure for UI filtering and
support export.

## Decision

`millo-audit` owns a versioned structured event model and persistence policy.

- Every entry has a monotonic sequence, process session ID, Unix timestamp,
  level, category, stable event ID, operator-facing message, and typed JSON data.
- The Tauri adapter records lifecycle, transport, controller, setting, Jog,
  work-zero, preflight, authorization, sender, Hold, Reset, Alarm, disconnect,
  persistence, and UI event-bridge outcomes.
- Sender entries retain source name, source line, current command, state,
  acknowledgement progress, timing, and typed failure through its bounded
  snapshot. The complete G-code source is never copied into the audit log.
- `record` updates a bounded in-memory tail and uses a fixed-capacity nonblocking
  queue. A dedicated writer thread performs JSON serialization, writes, flushes,
  rotation, and export. Queue drops and write failures are observable counters.
- JSON Lines files rotate at 5 MiB and retain four preceding generations. The
  most recent 2,000 entries are restored across active and rotated files.
- Audit initialization failure falls back to an in-memory log with a Critical
  event. Diagnostics cannot prevent Millo from starting or controlling a machine.
- Text and JSONL export are serialized by the same writer after all previously
  queued entries. Tauri owns the native save dialog and the audit crate owns
  destination writing.

The React Log Viewer presents a modal workstation panel with level/category
color, search, level and category filters, expandable structured data, health
counters, periodic refresh, and native `.log`/`.jsonl` export. Debug is hidden
by default to preserve operator signal.

## Consequences

- A failed job can be reconstructed across controller, safety, and sender
  boundaries from one ordered artifact.
- Detailed motion snapshots create disk traffic, but total storage is bounded.
- Audit persistence is diagnostic, not a recovery lease or authorization source.
- A future plugin capability may expose a read-only filtered stream without
  granting filesystem or machine-control access.

## Verification

- Audit tests cover bounded memory, monotonic sequence, rotation, restore, and
  text export.
- UI read-model tests cover default Debug suppression, combined search/category
  filtering, and attention counts.
- `/?fixture=logs` verifies structured colors, filters, details, and stable modal
  geometry without requiring Tauri or a controller.
- `npm run verify` remains the repository gate.
