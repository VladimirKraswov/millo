# ADR 0054: Full virtual GRBL execution target

## Status

Superseded by ADR 0055. Retained as history of the in-process approach.

## Context

The original Mock GRBL was useful for lifecycle and fault fixtures, but normal
program execution was still classified as serial-only. Program lines received
`ok` without changing coordinates, the mock had no persistent machine profile,
and the desktop exposed a separate reduced dry-run workflow. That made the mock
good for unit tests but poor for rehearsing an operator job.

## Decision

Mock GRBL is a machine-capable `ExecutionTarget`, alongside Serial. Both targets
use the same command actor, readiness policy, GRBL Check certificate, one-use
authorization, bounded sender, Hold/Resume/Reset path, tool-change barrier,
recovery journal, probing, heightmap, work-zero and UI.

On first connection the desktop creates and binds a stable virtual-machine
profile from the mock controller's own `$130/$131/$132` values. Later sessions
match it by the synthetic controller fingerprint just as physical controllers
match their profiles by hardware identity.

`millo-mock` now contains a separate virtual GRBL planner. It maintains machine
and work coordinates, G54-G59, modal motion, units, distance and feed modes,
feed/spindle/accessory state, line numbers and a time-based motion queue. It
interpolates G0/G1 and circular or helical G2/G3 motion in all GRBL planes,
including IJK, radius and full-circle forms. Status polling reports intermediate
`MPos`, `WPos`, `FS`, `Bf`, `Ov`, `Ln` and `A` values while the virtual tool is
moving. `$C` parses without motion; `!`, `~`, Jog Cancel and Soft Reset affect
the same virtual planner.

The virtual clock is deliberately accelerated and bounded so long jobs remain
observable without taking production time. This changes wall-clock duration,
not command ordering or controller state transitions. Electrical behavior,
step loss, acceleration curves and cutting forces remain outside the simulator.

Protocol tests that emulate a physical serial device may explicitly disable
virtual motion and supply scripted status frames. This is a test-only mode;
application Mock GRBL enables virtual motion by default.

## Consequences

- Switching between Mock and Serial no longer changes the Program workflow.
- Generated files, selected-line starts, depth correction and compensated
  heightmaps can be rehearsed through the production sender without hardware.
- The Three.js tool marker follows status positions from the same event stream
  used by a physical controller.
- Fault-injection controls remain under Connection diagnostics, but are not a
  separate execution path.
- A successful virtual run validates Millo's policy, protocol and state-machine
  behavior; it does not prove physical clearance, wiring or material setup.

## Verification

Unit and actor fixtures cover compact/modal input, absolute and incremental
linear motion, interpolated arcs, G93 timing, Check mode, Hold/Resume, reset,
final coordinates, profile creation and the same authorization-to-completion
flow used by Serial.
