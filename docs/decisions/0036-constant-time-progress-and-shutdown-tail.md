# ADR 0036: Require a shutdown tail and constant-time progress evidence

## Status

Accepted.

## Context

An `ok` means GRBL accepted a line for execution; it does not prove all physical
motion has finished. A file may also omit output-off commands. Large jobs cannot
afford UI or snapshot work proportional to total line count, and a percentage
alone cannot distinguish healthy progress from a stalled transport.

## Decision

- Every policy-approved plan receives typed `M5` and `M9` epilogue lines after
  source commands and before an optional program-end command.
- M2/M30 remains deferred until fresh `Idle`; physical completion still requires
  every response and a final fresh `Idle` observation.
- Sender records acknowledgement sequence, exact last accepted line/command,
  wall-clock age since the last acknowledgement, and shutdown-tail count.
- Snapshot generation must be O(1). Aggregate counts are calculated once while
  loading the immutable plan.
- The sender limit permits parser-sized programs plus bounded host-generated
  lines. In-flight response state remains bounded by GRBL RX capacity.
- A 100,000-line test is required and checks peak FIFO depth and byte capacity.

## Consequences

- Missing M5/M9 in source no longer leaves normal completion dependent on file
  hygiene.
- Operators can see whether progress is fresh and whether shutdown lines were
  accepted, while errors retain the exact failed line.
- Software cannot prove a manually switched spindle has stopped; that remains a
  physical checklist fact. Electrical-noise faults also still require correct
  shielding and grounding.
- Character-count streaming can have later lines already buffered when GRBL
  rejects one. Physical failure therefore still sends Hold then Soft Reset.
