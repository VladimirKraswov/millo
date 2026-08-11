# ADR 0025: Stream programs through a bounded GRBL receive window

## Status

Accepted.

## Context

ADR 0023 intentionally began with one outstanding line. That preserved exact
response correlation but left GRBL's planner underfed on serial links. GRBL 1.1
accepts a stream while the sender keeps the total unacknowledged character count
within the controller receive buffer. A rejection can arrive after later lines
have already entered that buffer, so faster dispatch also needs an explicit
queue-flush policy.

## Decision

- `millo-sender` owns a FIFO of dispatched, unacknowledged lines and their
  source locations. The default usable window is 127 bytes.
- Device Inspector parses `[OPT:flags,planner,rx]`. The one-use physical-run
  lease carries that observation to Start, where sender capacity becomes
  `rx - 1`. Missing or invalid values use the default; reported values are
  bounded to 4095 usable bytes.
- Capacity accounting includes every UTF-8 command byte and its trailing
  newline. A command larger than the configured window is rejected before run.
- The single-owner command actor fills the available window and consumes
  terminal responses in FIFO order. Each `ok` frees exactly the oldest line's
  bytes; `error` and `ALARM` remain attached to that line.
- `M0/M1` are fill barriers. No later line is sent until the pause is
  acknowledged and explicitly resumed.
- Physical `M2/M30` remains deferred until prior commands are acknowledged and
  GRBL reports fresh `Idle`. Its own acknowledgement is still required.
- On a physical command rejection, alarm, timeout, or response/write failure,
  the actor marks the correlated line failed and sends realtime Feed Hold then
  Soft Reset. This best-effort abort flushes commands already accepted after the
  failed line. Mock GRBL implements the same reset flush.
- Snapshot metrics expose dispatched and in-flight line counts, occupied RX
  bytes, and configured capacity without exposing a raw-line command surface.

## Consequences

- Serial throughput no longer depends on one command round trip at a time.
- Response order is deterministic and bounded by the configured byte window.
- A failed physical stream intentionally resets modal/controller execution
  state; resuming it requires a new preflight and one-use authorization.
- Controllers with larger reported RX buffers are fed accordingly without
  changing the sender state machine or trusting stale UI state.
