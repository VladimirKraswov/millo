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

Unsupported geometry fails closed with a named format feature. Native Clipper2
offsets replace approximate canvas geometry. The UI remains a replaceable
workflow over the same host service.

## Consequences

- Other trusted plugins can reuse PCB inspection and CAM without copying the
  algorithm.
- External script packages still cannot inject Gerber parsers or native code.
- The release has no `pcb2gcode` executable or `libgerbv` runtime dependency.
- Full aperture-macro and Excellon-slot support require new Rust fixtures before
  their fail-closed checks can be removed.
- Every generated PCB still passes the normal Check, readiness, map
  compensation, authorization, tool-change and recovery workflow.

