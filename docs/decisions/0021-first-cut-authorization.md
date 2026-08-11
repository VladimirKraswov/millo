# ADR 0021: First-cut authorization is short-lived and single-use

## Status

Accepted.

## Context

The serial real-run preflight can prove that a parsed motion-only program and a
fresh GRBL inspection satisfy Millo's software policy. It cannot observe stock
clamping, cutter installation, work zero, fixture clearance, a manually powered
spindle, or whether the operator can reach machine power. A green report must
not silently become execution authority.

The first physical machine has no homing, limit switches, probe, or physical
emergency stop. Its spindle is controlled manually. These facts make an explicit
operator boundary necessary before a serial sender can be designed.

## Decision

- `millo-run` owns a six-field `FirstCutConfirmation`: secured stock, secured
  cutter, verified XYZ work zero, verified safe Z, running manual spindle, and
  immediately reachable power control.
- React displays the checklist but cannot mint a lease or resubmit a displayed
  report as evidence.
- Tauri reparses the retained original source. The single-owner command actor
  repeats `?`, `$I`, `$$`, `$G`, `$#`, `?` and evaluates a new preflight before
  authorizing.
- A successful gate issues one opaque 30-second authorization. It is bound to a
  SHA-256 fingerprint of the program name and source lines, the controller reset
  and reconnect counters, the fresh poll sequence, and observed machine/work
  positions.
- Consumption is fail-closed and destructive: wrong ID, changed program,
  expiration, unsafe state, position change, or controller-session change uses
  up the attempt. A new authorization requires the whole workflow again.
- Polling invalidates the lease on non-Idle state, position/session change, or
  expiry. Controller-changing actor requests explicitly invalidate it.
- This slice exposes no consumer through Tauri. It has no serial sender and no
  Start action. Authorization performs read-only controller traffic only.

## Consequences

- A future serial sender must accept the original source and authorization ID,
  rebuild an opaque approved plan, refresh state as required, and consume the
  matching lease atomically before its first write.
- Mock transport fixtures can exercise the serial-class actor transaction
  without granting Mock dry-run plans access to this lease.
- Manual spindle state remains an operator assertion because GRBL cannot observe
  a separately powered spindle.
- The lease is deliberately not persisted. Reconnect, app restart, or controller
  reset always requires a new physical checklist.
