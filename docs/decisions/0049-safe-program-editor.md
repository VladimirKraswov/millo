# 0049. Safe Program Editor

Status: accepted

## Problem

The virtualized Program table can select a source row but cannot repair a CAM
file. Operators need ordinary text and row operations without allowing a
half-typed block, stale preview, or stale preflight result to become sender
authority.

## Decision

Millo keeps two revisions while the editor is open:

1. The draft is a mutable source string with a bounded 200-revision undo/redo
   history. Copy, cut, paste, insert-row, and delete-row operations modify only
   this draft.
2. Every settled draft revision is sent through `ProgramGateway.parse` with the
   active optional-block policy. Parse responses carry a local sequence, so a
   slow older response cannot replace a newer preview.
3. The editor's syntax colors are a viewport-bounded lexical aid. They do not
   parse geometry, approve commands, or alter source text.
4. The right-hand Three.js scene and warning list use only the latest successful
   immutable `GcodeProgram`. While the current draft is invalid, that scene is
   explicitly marked as the preceding valid revision.
5. `Apply` is enabled only when the visible draft exactly matches that parsed
   revision. Applying atomically replaces source plus DTO and clears sender,
   GRBL Check, preflight, selected-line, and safe-start evidence.
6. Closing a dirty draft requires a second destructive action. An active sender
   and a compiled safe-start remainder cannot open the editor.
7. Native save reparses the source in Rust before displaying the save result.
   It accepts the same `.nc`, `.ngc`, `.gcode`, `.tap`, and `.cnc` formats as
   loading and records a structured storage audit event.

`Обработанная копия` is deterministic parser output: comments and formatting
are removed, words use `ProgramLine.normalized`, and `/` optional blocks are
included or omitted according to the active policy. It is reparsed before
write. This is not Candle's heightmap-compensated transformed export; Millo
will add geometric/heightmap transformation only with a typed core transform
model that can prove arc and modal semantics.

## Consequences

- An editor bug or syntax highlighter cannot mint sender commands. Every run
  still reparses source and passes the normal policy, Check, preflight, and
  authorization gates.
- Large files retain a full native textarea, but only visible highlighted rows
  are mounted in the DOM.
- Processed export is useful for inspecting the exact optional-block policy and
  normalized GRBL stream, but it deliberately does not claim geometric
  compensation that the core does not yet implement.
