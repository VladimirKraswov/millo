# ADR 0035: Bind optional stream semantics before authorization

## Status

Accepted.

## Context

G-code files may carry host-side transport checksums, optional blocks prefixed
with `/`, and `M1` optional stops. Stripping those tokens during dispatch is
unsafe: an optional modal command can change every later coordinate, while a
corrupt checksum means the bytes being previewed are not the bytes the producer
certified. Reading UI toggles only when a line is sent would also allow a
different program interpretation from the one checked and authorized.

## Decision

- A single final decimal checksum is validated as an 8-bit XOR of the exact
  source bytes before `*`. A checksummed block must begin with `N`; malformed,
  duplicate, or mismatched values block execution.
- The checksum suffix is removed only after successful host validation because
  target GRBL 1.1 does not accept that transport framing.
- `/` is valid only as the first non-whitespace code character. The parser marks
  the block and, when Block Delete is enabled, excludes it from modal and
  geometry state as well as the sender plan.
- `M0` always creates an empty-FIFO pause. Isolated `M1` creates that pause only
  when Optional Stop is enabled; otherwise it is omitted locally.
- The UI reparses original source through Rust whenever Block Delete changes.
- Both options are immutable plan metadata and are repeated in preflight,
  confirmation, authorization, and consumed lease. Block Delete also changes
  the program fingerprint.

## Consequences

- Preview, bounds, modal contract, line table, Check mode, and physical
  execution share one interpretation.
- A sender cannot turn a skipped modal block into active movement or silently
  ignore a producer-detected corruption.
- Changing either option invalidates displayed preflight and authorization.
- Checksums provide source-integrity validation, not serial retransmission or
  sequence recovery; GRBL `ok/error` correlation remains the wire protocol.
