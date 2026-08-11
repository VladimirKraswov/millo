# ADR 0031: Keep runtime timing inside the sender plan and state machine

## Status

Accepted.

## Context

Parser summaries estimated feed and dwell time, but the execution snapshot knew
only line-count progress. Computing elapsed or ETA in React would create a
second lifecycle clock, count Hold time, lose terminal values on rerender, and
hide whether rapid duration was unknown.

## Decision

- `DryRunPlan` stores per-line and total estimates as integer milliseconds.
  Motion durations come from parser geometry; G4 dwell is assigned to its source
  line. Unknown rapid duration is represented as absent, not zero-known.
- `Sender` owns a monotonic active timer. Pause records an instant; Resume adds
  that interval to excluded duration. Completed, Failed, and Cancelled freeze the
  final elapsed value; Draining remains active until fresh Idle.
- Estimated completed time advances only when the matching FIFO line receives
  `ok`. Remaining time is the saturating difference from the plan lower bound.
- Snapshots expose elapsed, completed, remaining, total, and an explicit
  completeness flag. Actor polling republishes snapshots while a run is active.
- React only formats the typed values. It labels incomplete estimates `ETA >=`.

## Consequences

- Timing remains correct across UI remounts and future plugin-provided run views.
- Hold and terminal states have testable semantics independent of Tauri.
- Current estimates do not model machine acceleration or rapid limits. Candle's
  planner-oriented estimator remains a reference for a future dedicated Rust
  estimator; the completeness flag must remain even after that work.
