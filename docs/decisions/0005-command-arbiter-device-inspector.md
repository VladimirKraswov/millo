# ADR 0005: Single-owner command arbiter and read-only inspection

- Status: accepted
- Date: 2026-08-10

## Context

Periodic status polling and ordinary GRBL commands share one byte stream. A
mutex around the controller prevents simultaneous writes, but leaves scheduling
split between Tauri commands and a polling task. As command types grow, that
shape risks response misattribution and makes transport ownership unclear.

The first real-hardware workflow needs controller identity and configuration but
must not permit motion or spindle control.

## Decision

Create `millo-command` with one asynchronous actor that owns
`Controller<BoxedTransport>`. All lifecycle, realtime, and line requests enter a
bounded FIFO channel. The actor owns periodic polling and publishes snapshots
through a watch channel. Tauri holds only a cloneable arbiter handle and bridges
snapshot changes to frontend events.

Model realtime bytes separately from newline-terminated commands. A realtime
status request consumes its status frame in the same actor operation. Feed Hold,
Cycle Start, and Soft Reset have typed Rust representations but are not exposed
through Tauri in this slice.

Expose Device Inspector as exactly four typed queries: `$I`, `$$`, `$G`, and
`$#`. Each query consumes lines until its correlated `ok`, `error:n`, `ALARM:n`,
or reset terminal. Rust converts the responses into firmware, settings, modal
state, and coordinate-parameter domain data. There is no raw-line IPC command.

## Consequences

- Only the actor worker can touch the active transport.
- Polling cannot interleave bytes with an in-flight line response.
- UI code receives structured data and never parses GRBL wire text.
- Current IPC cannot express G-code motion or spindle commands.
- Inspector errors and alarms remain associated with the command that produced
  them, while status and reset lines still update controller state.
- Sender buffering will extend the actor protocol instead of adding another
  serial writer.
