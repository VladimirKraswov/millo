# ADR 0032: Physical GRBL dialect evidence

## Status

Accepted.

## Context

Nominal preview geometry does not change for path-control commands, so a
permissive parser could treat both `G61` and `G64` as harmless modal words.
The target GRBL 1.1 controller accepts `G61` but rejects `G64` with `error:20`.
Deferring that discovery to a live sender can stop a job after earlier motion
has already completed.

## Decision

- Keep `G61` in the supported GRBL parser dialect.
- Reject `G64` with a blocking parser error before plan construction.
- Record the accepted command in a physical Check fixture and the rejected
  command in hardware-independent parser regression coverage.
- Expand the dialect only after both parser fixtures and physical Check evidence
  exist for the target controller family.

## Consequences

- Preview, Check and production plans share one evidence-backed command set.
- Unsupported dialect extensions fail before machine motion.
- Compatibility is intentionally narrower than generic RS-274/LinuxCNC input.
