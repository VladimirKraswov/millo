# ADR 0007: Safety controls use actor-owned challenges and leases

## Status

Accepted on 2026-08-11.

## Context

The first physical motion test needs reliable Feed Hold and Soft Reset before a
jog command exists. A React confirmation dialog alone cannot be a safety gate:
it can become stale across alarm, reset, reconnect, or controller-state changes.
Likewise, a previously displayed green readiness report must not authorize later
motion without re-reading the controller.

## Decision

Create `millo-safety` as a pure Rust state machine owned by the command actor.
It issues and validates two bounded capabilities:

- A 10-second reset challenge, consumed before the actor writes `Ctrl-X`.
- A 15-second, single-use test-jog authorization issued only after all operator
  confirmations and a fresh `$I/$$/$G/$#` readiness assessment.

Expose named Tauri commands for Feed Hold, reset challenge request/confirmation,
and test-jog preparation. Do not expose arbitrary realtime bytes, raw controller
lines, spindle commands, or a jog consumer in this slice.

## Consequences

- UI state can present intent but cannot independently authorize a controller
  write.
- Reusing or guessing a stale confirmation does not repeat Reset because the
  active challenge is consumed on the first confirmation attempt.
- Alarm, reset, reconnect, disconnect, non-idle state, timeout, and other
  realtime actions invalidate a prepared test jog.
- Preflight can display a newly discovered blocker without issuing a lease.
- The actor is still the sole port owner. Realtime bytes are serialized with the
  current bounded controller transaction; priority between streamed G-code lines
  remains a sender-state-machine requirement.
