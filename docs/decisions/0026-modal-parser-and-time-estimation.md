# ADR 0026: Parse GRBL modal geometry and estimate commanded time

## Status

Accepted.

## Context

The initial preview parser understood linear motion and XY arcs. A production
sender cannot safely validate real CAM output if G18/G19 arcs, helical axes,
inverse-time feed, or conflicting modal words are either ignored or drawn as a
plausible chord. Operators also need a useful duration before a run, but a
portable G-code file does not contain every controller acceleration and rapid
rate parameter.

## Decision

- `millo-gcode` owns a GRBL-oriented modal interpreter for G0-G3, G17-G19,
  G20/G21, G90/G91, G91.1, G93/G94, G4, and G80.
- Circular and helical IJK/R arcs are sampled in all three right-handed GRBL
  planes. Full-circle IJK arcs retain their complete sweep.
- The parser rejects conflicting commands in one modal group, mixed R/IJK arc
  definitions, wrong-plane offsets, and context words used by the wrong command.
- GRBL 1.1 does not support absolute arc centers. G90.1 geometry may be shown
  for diagnosis, but its error prevents sender policy from approving the file.
- G94 cutting time is distance divided by modal feed. G93 cutting time is
  `60 / F` for that block and therefore requires a positive F on every cutting
  motion. G4 contributes its P duration directly.
- Rapid geometry is retained, but any non-zero G0 segment marks the total as
  incomplete. The displayed value is then a known lower bound, not a promise.
- A physical run still requires explicit millimetres, absolute distance, feed
  mode, and arc plane declarations before the relevant motion. Parser defaults
  never satisfy that execution contract.

## Consequences

- Preview, policy, sender source locations, and duration now derive from one
  immutable modal interpretation.
- Common multi-plane CAM output can be validated without flattening its arcs.
- Programs with ambiguous feed or modal meaning stop before controller I/O.
- Exact elapsed-time prediction remains future machine-aware work involving
  `$110-$122`, planner behavior, overrides, and measured execution telemetry.
