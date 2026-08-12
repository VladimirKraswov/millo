# ADR 0017: Immutable program-line selection

## Status

Accepted; amended by [ADR 0049](0049-safe-program-editor.md).

## Context

Operators need to relate parser output to preview geometry without turning the
line table into an editor or execution authority. Programs may contain up to
200,000 source lines and 500,000 preview points, so mounting every row or
rebuilding the complete Three.js scene on each click is not acceptable.

## Decision

- Keep selected `sourceLine` as local Program feature state. The immutable Rust
  DTO and retained original source are never modified.
- Render Program Lines through a fixed-row virtual window with overscan and a
  bounded scroll viewport on desktop and mobile.
- Derive selected geometry in the pure `toolpathReadModel` by exact
  `ToolpathSegment.sourceLine` equality and the base model's center.
- Keep a persistent Three.js line/point selection layer. Replace only its
  geometry when selection changes and dim base rapid/cut materials only when the
  selected line owns preview motion.
- Permit selection of comments, modal commands, and warnings. Such lines show
  `No preview motion`; adjacent geometry is never inferred.

## Consequences

- Table selection and 3D highlighting remain read-only and cannot reorder,
  skip, edit, or send G-code. Editing is a separate draft/parse/apply workflow.
- Large files have bounded DOM cost, while their scrollbar still represents the
  complete source.
- Warning entries can select their exact source line using the same state.
- Future sender-current-line following may publish into this selection model,
  but manual selection must never influence sender order or authorization.
