# ADR 0057: Actor-owned homing and continuous jog

## Status

Accepted on 2026-08-20.

## Context

ADR 0008 deliberately limited initial hardware motion to one lease-guarded step
jog. A production operator workflow also needs session-scoped homing,
press-and-hold motion, optional rotary control, WCS selection, and declared
machine outputs. Implementing Candle's synchronous event-pumping jog loop would
couple motion lifetime to the WebView and could delay Hold, Reset, or link-loss
handling.

## Decision

The Rust command actor owns both homing and continuous-jog lifecycles. `$H` is
an extended correlated command with a travel-derived timeout and a mandatory
fresh-Idle settle phase. Its reference is valid only for the current electrical
session and is invalidated by reset, transport recovery, or relevant profile and
controller-setting changes.

Continuous jog consumes the same fresh one-use readiness authorization as step
jog. It emits one bounded single-axis `$J=` and retains an actor watchdog until
fresh Idle, Alarm, Door, disconnect, or `0x85` cancellation. React handles input
gesture lifetime, but it cannot own or extend the controller command.

Homed XYZ uses a captured MPos envelope. Unhomed XYZ uses profile distance and
travel bounds. Optional A uses degree-specific profile limits. WCS and machine
outputs are typed, Idle-only, and modal-verified; spindle/coolant writes require
explicit profile capabilities.

## Consequences

- UI event loss cannot create an unlimited jog command.
- Hold, Reset, Status, Jog Cancel, and Disconnect stay serviceable while homing
  or continuous motion is active.
- A reset/recovery cannot leave a misleading "homed" indicator or stale machine
  envelope.
- Firmware acceptance and physical completion remain separate states.
- External plugins do not receive the new motion/output authority implicitly.
- ADR 0008 still defines step-jog authorization and encoder bounds; its decision
  to defer continuous, keyboard, A-axis, and spindle commands is superseded by
  this ADR.
