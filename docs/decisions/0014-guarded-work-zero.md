# 0014: Guarded work-zero transaction

- Status: accepted
- Date: 2026-08-11

## Context

The first machine has no homing, limit switches, probe, or physical emergency
stop. The operator still needs to define a workpiece datum before loading a
program. Exposing raw `G10`, a console, or a selectable `P` value would bypass
the typed command boundary and could silently alter the wrong coordinate system.

The controller's displayed state can become stale between a UI render and a
button press. GRBL acknowledging `G10` also does not by itself prove that the
expected active work offset was changed.

## Decision

Millo exposes three named operations: Zero X, Zero Y, and Zero Z. A request
contains only the axis and an explicit one-attempt operator confirmation.

The command actor owns the complete transaction:

1. Reject an unconfirmed request before controller I/O.
2. Read fresh status and require stable `Connected + Idle`, with no alarm or
   reset notice.
3. Read `$G` and accept only active G54-G59.
4. Map G54-G59 to `P1..P6` in Rust and send one typed
   `G10 L20 Pn <axis>0` command.
5. Read `$#`, require the matching parameter, then read final status.
6. Verify the selected work coordinate is within `0.002 mm` of zero.

The operation invalidates any test-jog authorization. React cannot format a GRBL
line, select another WCS, or infer success from local state. Work Zero is placed
in the independent `control.coordinates` extension slot and is not granted to
plugins as a machine capability in this slice.

## Consequences

- Work zero remains usable without introducing a general command endpoint.
- A stale Idle display cannot authorize a write.
- Verification adds bounded `$G`, `$#`, and status traffic to each attempt.
- Unsupported or missing modal/coordinate data fails closed.
- The implementation is tested with Mock GRBL only. Physical use requires a
  separate operator-confirmed check.
- Probe and heightmap workflows remain blocked until a sensor is physically
  installed and its input behavior is validated.
