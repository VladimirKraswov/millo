# ADR 0033: Typed sender failures

## Status

Accepted.

## Context

The sender already correlated a terminal response with the oldest FIFO line,
but exposed the result mainly as a formatted string. String details such as
`Some(33)` are Rust formatting artifacts, not a stable API. A React view or
plugin would have to parse that text to distinguish a GRBL error from an alarm,
timeout, reset, or disconnect.

## Decision

- Terminal sender snapshots carry a closed `SenderFailureKind`.
- The failure owns its optional GRBL code, source line, exact command, and
  display message before in-flight state is cleared.
- Controller errors are classified at the Rust command boundary.
- `lastError` remains temporarily as compatibility display text; consumers use
  the structured failure for behavior and concise labels.

## Consequences

- Operator UI and future plugins can route alarms, timeouts, and disconnects
  without matching localized or implementation-specific strings.
- Diagnostics retain the exact failed block after abort/reset cleanup.
- Adding a new terminal category requires an explicit enum and regression test.
