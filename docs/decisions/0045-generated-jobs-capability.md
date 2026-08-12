# ADR 0045: Generated jobs belong to the core

## Status

Accepted.

## Context

Multiple plugins may create G-code from images or other domain inputs. Putting
conversion inside each plugin would duplicate CAM policy, permit unreviewed raw
G-code injection, and make preview/sender behavior depend on plugin quality.

## Decision

- Add independent Rust crate `millo-cam` for bounded SVG/PNG engraving jobs.
- Use VTracer as the replaceable PNG-to-SVG implementation and `usvg` as the
  typed SVG parser. Pin versions behind Millo-owned DTOs.
- Reparse every generated source with `millo-gcode` before returning it.
- Implement `jobs.create` as generate/save/open for host-issued immutable jobs.
- Keep job publication separate from execution. A generated job enters the
  existing Program workflow and gains no sender authority.
- Ship `io.millo.image-to-gcode` as an explicitly linked, granted, default-on
  module in the global `workspace.tools` slot.

## Consequences

Future plugins can reuse and extend generation without depending on Tauri or
GRBL internals. The vectorizer can be forked or replaced behind the same core
contract. The first implementation intentionally generates contour engraving;
filled-area pocketing, centerline tracing, multi-depth passes, and tool-radius
compensation require separate typed strategies rather than hidden plugin flags.
