# ADR 0028: Keep realtime safety responsive during sender I/O

## Status

Accepted.

## Context

The single-owner actor prevents concurrent serial writers, but awaiting one
complete program response inside an actor turn could delay Feed Hold or Soft
Reset until the command timeout. Disabling status polling whenever a line was
in flight also hid position, buffer, and override telemetry during long streams.
A second reader task would introduce response races and violate actor ownership.

## Decision

- The controller retains one pending oldest-command response, accumulated
  message lines, and a silence deadline based on the last valid controller
  activity.
- Each sender turn waits for at most 10 ms of serial input. A pending read
  returns control to the actor without resetting the silence deadline.
- The actor prioritizes queued requests, then status ticks, then sender response
  and dispatch work. Feed Hold and Reset therefore wait at most one read slice
  behind serial input processing.
- During an in-flight response, a tick writes realtime `?` but does not run a
  second blocking status transaction. The program-response parser classifies
  interleaved status frames separately from terminal `ok/error/ALARM/reset`.
- Planner reconciliation runs only after `StatusObserved`. Pending reads and
  command acknowledgements cannot reuse an old `Idle` snapshot to release a
  deferred program end.
- A valid interleaved status or message refreshes the silence deadline without
  acknowledging the command. This permits planner backpressure, long motion,
  and dwell while GRBL remains observable. A silent controller still reaches
  the configured timeout, preserving power-loss and link-loss detection.
- Timeout, transport failure, response mismatch, and protocol rejection retain
  their existing correlated failure and physical abort behavior.

## Consequences

- Realtime safety remains responsive even if GRBL delays a normal response.
- Live status telemetry continues during bounded RX streaming without a second
  transport owner.
- Dense paths cannot fail merely because their planner queue keeps the oldest
  `ok` pending for longer than the ordinary command timeout.
- The Mock transport models realtime status preceding a delayed acknowledgement.
  Actor tests cover preemption and telemetry, and the complex physical Check
  fixture still completes 25/25 with verified return to `Idle`.
