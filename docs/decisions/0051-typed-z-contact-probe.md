# 0051. Typed Z Contact Probe

Status: accepted

## Problem

A touch plate needs more than a raw `G38.2` console shortcut. Contact can take
longer than the normal command timeout, the active WCS and plate thickness must
be applied correctly, probing changes modal state, and an already closed input
must never start downward motion.

## Decision

Millo models one Z contact operation as a single command-arbiter request. The
frontend submits bounded numeric settings and one setup confirmation. The Rust
actor owns status validation, `$G`, `G38.2`, `PRB` verification, `G10 L20`, `$#`
verification, modal restoration, retract, and the final `Idle` check. The long
probe response is consumed in short actor steps: status, Hold, and confirmed
Soft Reset remain preemptive while all ordinary requests stay queued.

Probe geometry is profile-local. `useForWorkZero` is a workflow preference, not
an electrical enable: it removes Z from ordinary zeroing, enables the typed
contact action, and preserves live input telemetry. It can be disabled and
saved without a measured plate; ordinary manual Z zeroing then becomes
available again. The Rust actor independently rejects contact requests while
the preference is disabled, so a plugin cannot bypass the UI lockout.

## Consequences

- No UI or plugin can interleave serial commands inside the contact sequence.
- Search time has a movement-derived timeout without weakening global command
  timeouts.
- Contact evidence and the written offset are independently verified.
- G38.2 motion mode cannot leak into the next command after a successful cycle.
- Soft Reset during a delayed probe cancels the operation before any offset
  write; a regression test proves no `G10` is dispatched afterward.
- Heightmap probing remains a separate future plan, not an implicit loop around
  this UI command.
