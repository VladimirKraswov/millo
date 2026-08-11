# 0015: G-code program boundary

- Status: accepted
- Date: 2026-08-11

## Context

Millo needs file loading, parser diagnostics, and preview before it can design a
sender. Treating G-code as strings in React would duplicate modal behavior,
make warning policy difficult to test, and invite a future UI component to pass
unchecked lines to the controller actor. Reusing controller wire parsing would
also mix two different concerns: GRBL responses and user programs.

The first physical machine has a manual spindle and no probe, limits, homing, or
physical emergency stop. Preview must therefore expose safety-relevant commands
without creating any execution path or implying that a visually plausible file
is safe to run.

## Decision

Create an independent `millo-gcode` Rust crate. It accepts a bounded source name
and source string, then returns an immutable serializable program containing:

- normalized, source-addressable lines;
- typed warnings and detected safety features;
- metric toolpath segments and sampled XY arcs;
- bounds, rapid/cutting distances, and preview completeness;
- a fail-closed `dryRunEligible` parser gate.

Parsing supports a deliberately explicit first subset. Unknown or unsafe
behavior becomes a warning with its original line; unsupported motion is not
approximated as trustworthy geometry. Spindle/coolant activation, tool changes,
probing, machine-coordinate moves, coordinate mutation, malformed geometry, and
parser limits prevent dry-run eligibility.

React reads supported files and calls a `ProgramGateway`. A thin Tauri command
runs the parser off the async runtime thread and owns no `AppState`. A pure
TypeScript read model converts only the returned geometry into rapid/cut buffers.
Three.js is lazy-loaded behind that adapter. The program workbench has no
machine gateway, actor handle, transport, or plugin grant.

## Consequences

- Parser fixtures run without Tauri, a browser, or hardware.
- File loading and preview cannot move the machine, even when connected.
- Future sender work must consume the typed program through a new capability and
  add its own authorization/state machine; it cannot reuse `ProgramGateway` as a
  write path.
- Preview can be incomplete while diagnostics remain useful and source lines
  remain available for a future program table.
- The initial parser is intentionally not a universal RS-274 interpreter.
  Compatibility grows fixture by fixture, with unsupported behavior failing
  visibly rather than being guessed.
