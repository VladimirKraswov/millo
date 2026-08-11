# ADR 0029: Expose runtime overrides as typed realtime operations

## Status

Accepted.

## Context

GRBL 1.1 changes feed, rapid, and spindle scaling with non-printable realtime
bytes. They must remain responsive during buffered execution, but a raw-byte or
raw-line API would let UI and future plugins bypass the command actor's safety
boundary. Status also has to remain correlated with an outstanding program
response.

## Decision

- Domain enums represent the finite GRBL operations: feed and spindle reset,
  `+/-10`, `+/-1`, plus rapid `100/50/25`.
- Only `millo-controller` maps those enums to protocol bytes. The command actor
  serializes them through the single transport owner and Tauri exposes the same
  typed values.
- An override request may run between 10 ms response slices. It neither pauses
  the sender nor consumes the outstanding line response.
- Realtime `?` status remains the evidence of the applied value. `millo-grbl`
  parses `Ov:` into a typed snapshot and Mock GRBL applies the same bounded
  percentage behavior.
- This API does not expose spindle start, coolant, arbitrary realtime bytes, or
  arbitrary G-code.

## Consequences

- Core UI and future capability proxies can adjust known runtime scaling
  without acquiring protocol-level authority.
- Tests can verify byte mapping, sender preemption, and status telemetry on Mock
  GRBL. A reproducible hardware example verifies the complete serial round trip
  and restores every override to 100% before disconnecting.
