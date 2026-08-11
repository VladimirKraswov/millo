# ADR 0016: Mock-only bounded dry-run sender

## Status

Accepted.

## Context

The preview parser can describe a program but must not itself grant machine
execution. The first machine has no homing, limits, probe, or physical emergency
stop, and its spindle is controlled manually. A sender is needed for protocol
and workflow development without widening the serial hardware surface.

## Decision

Create two independent Rust crates:

- `millo-dry-run` reparses program semantics into an opaque `DryRunPlan`. It is
  fail-closed for parser blockers and explicitly forbids spindle/coolant
  activation, non-zero spindle speed, probing, tool change, machine/reference
  coordinates, and coordinate mutation. It prepends M5/M9 and exposes no public
  constructor or deserialize path for approved lines.
- `millo-sender` owns a bounded state machine with one line in flight. A line is
  advanced only by its correlated `ok`; `error`, `ALARM`, timeout, or invalid
  controller state terminates the run.

The sender is driven by the existing `millo-command` actor one transaction at a
time. Tauri reparses the original source at start. Tauri and the actor both gate
execution to Mock GRBL; serial transport replacement disables and cancels it.
React receives typed sender snapshots but no raw-line command.

## Consequences

- Parser preview and UI eligibility remain advisory, never authorization.
- Realtime and lifecycle requests use the same transport owner and are
  considered between program lines.
- Mock tests can cover exact response correlation and operator progress without
  physical motion.
- Hardware dry run remains intentionally unavailable until a separate readiness,
  manual-spindle, travel-envelope, and operator-confirmation decision is made.
