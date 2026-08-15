# ADR 0056: Keep PCB CAM in a Rust core behind a bundled plugin

Date: 2026-08-14

## Context

PCB preparation needs rich file interaction and preview, but Gerber geometry,
tool compensation and multi-tool G-code are machine-domain behavior. Putting
that logic in React would duplicate parser/sender policy and let a UI module
construct unchecked source. Calling `pcb2gcode` or `libgerbv` would add GPL and
platform packaging constraints to every Millo installation.

## Decision

Add an independent `millo-pcb` crate for bounded RS-274X/Excellon parsing,
transform, boolean/offset geometry and deterministic G-code. Add
`inspectPcb`/`generatePcb` to the existing `jobs.create` capability. The
bundled `io.millo.pcb-fabrication` plugin receives only `ui.contribute`,
`tools.read` and `jobs.create`.

Rust resolves every `toolId` from the application-owned tool store, emits
manual `M6` barriers between different tools, and reparses output through
`millo-gcode`. The plugin can inspect, generate, save or publish only
core-issued immutable DTOs; it cannot access sender, serial, filesystem paths
or arbitrary Tauri commands.

The core also reports a bounded estimate of the narrowest clearance between
independent exterior copper contours. The bundled UI combines that measurement
with the library-owned V-tip diameter and included angle to recommend a tool,
cut depth, XY/Z feed and spindle speed. A recommendation is advisory and every
value remains editable; incomplete conical geometry is rejected instead of
being guessed.

Unsupported geometry fails closed with a named format feature. Native Clipper2
offsets replace approximate canvas geometry. The UI remains a replaceable
workflow over the same host service.

## Consequences

- Other trusted plugins can reuse PCB inspection and CAM without copying the
  algorithm.
- Hole contours are excluded from clearance estimation, so an annular ring is
  not mistaken for spacing between independent copper islands.
- External script packages still cannot inject Gerber parsers or native code.
- The release has no `pcb2gcode` executable or `libgerbv` runtime dependency.
- Aperture macros, step-and-repeat, Excellon `G85` and Gerber X2 drill routes
  are enabled only behind dedicated Rust fixtures; remaining ambiguous format
  features retain named fail-closed errors.
- Every generated PCB still passes the normal Check, readiness, map
  compensation, authorization, tool-change and recovery workflow.
