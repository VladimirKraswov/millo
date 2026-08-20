# ADR 0008: First motion is a lease-guarded step jog

## Status

Accepted on 2026-08-11. The initial deferral of continuous jog, keyboard jog,
A-axis, and spindle commands is superseded by ADR 0057; the step-jog lease and
encoder rules remain active.

## Context

The first machine has no homing, limit switches, or physical emergency stop.
Millo therefore cannot infer a verified travel envelope or safely expose a raw
G-code box. The existing readiness preflight issues a short-lived single-use
authorization, but previously nothing consumed it.

GRBL v1.1 defines `$J=` as a cancellable motion that does not alter parser state.
It accepts per-command `G91`, `G21`, one or more axis words, and a required feed.
Millo intentionally supports a smaller subset than the protocol.

## Decision

Expose one typed `step_jog` operation with an authorization ID, an X/Y/Z enum, a
signed distance, and a feed. The command actor consumes the authorization before
calling the controller, while its current snapshot must still be connected,
stable `Idle`, free of Alarm, and in the same reset/reconnect session.

The Rust GRBL encoder is the final syntax and finite-value boundary:

- exactly one axis selected by enum;
- incremental metric mode injected as `G91 G21` on every command;
- absolute distance from `0.01` through `100000 mm`;
- feed from `10` through `100000 mm/min`;
- finite, non-zero numeric values only.

Those are technical bounds, not operator permission. The command actor also
enforces selected-machine profile distance, selected-axis travel, and inspected
GRBL maximum rate. ADR 0040 defines the scalable operator policy.

The lease is not restored after validation rejection, controller rejection,
timeout, or transport failure. This fail-closed rule prevents a retry from
creating ambiguous duplicate movement after an uncertain I/O result. The UI
also clears its displayed lease before awaiting IPC.

Expose Jog Cancel as a separate named operation. It sends GRBL realtime `0x85`
only when the actor snapshot reports `Jog`. Do not expose arbitrary realtime
bytes, continuous jog, keyboard jog, multi-axis jog, raw lines, or spindle
commands.

GRBL protocol reference:

- <https://github.com/gnea/grbl/blob/master/doc/markdown/jogging.md>
- <https://github.com/gnea/grbl/blob/master/doc/markdown/interface.md>

## Consequences

- A single authorization can cause at most one port write.
- The operator arms jog with one explicit readiness decision. The UI maps it to
  the unchanged spindle-off, tool-clear, and reachable-power backend facts;
  every movement click still creates a fresh authorization and inspection.
- Backend limits remain effective if IPC is called without the React UI.
- `ok` records command acceptance, not physical completion. Status polling is
  authoritative for machine mode and final coordinates.
- Without homing, even a valid short jog can collide near an unknown travel end;
  operator clearance and access to machine power remain mandatory.
- Realtime requests still share the actor FIFO with bounded line transactions.
  Priority handling is deferred until the sender state machine exists.
