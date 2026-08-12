# 0052. Workpiece Heightmap Session

Status: accepted

## Problem

A heightmap describes one mounted workpiece and its current work zero, not the
machine itself. Reusing a profile-level map after replacing stock or losing
position can drive the cutter into the material. A multi-point probe also needs
preemptible Hold/Reset behavior, durable progress, and useful inspection beyond
an unlabelled table of values.

## Decision

Millo keeps probe input visibility, one-point Z zero, and heightmap probing in
one core workflow with three mutually exclusive modes: Off, Work Zero, and
Heightmap. The Rust command actor owns the complete serpentine operation. It
validates a bounded XY perimeter and grid, raises to absolute work clearance Z,
moves one XY point, executes `G38.2`, reads `PRB` through `$#`, records surface Z
without writing `G10`, and raises before the next point. Hold, Resume, and Soft
Reset remain preemptive. A failure after contact attempts safe-Z and modal-state
cleanup before publishing a failed snapshot.

Map data lives in `surface-session.json`, separate from machine profiles. A
pending map is checkpointed atomically while the preceding active map remains
intact. Only a complete all-contact operation replaces the active map. Restart
restores both records but never silently re-enables compensation; the operator
must confirm that the workpiece and active work zero have not moved.

Starting uses a three-phase durable boundary. The command actor first prepares
the bounded plan and reads fresh Idle/probe/WCS/modal evidence without emitting
motion. Tauri atomically persists that exact operation sequence as pending, then
commits it back to the actor. Only commit publishes Running and makes the first
safe-Z move dispatchable. Persistence failure discards the preparation; Reset is
allowed to preempt both phases, while unrelated machine commands are deferred.

The operator UI derives a perimeter from loaded program XY bounds plus a
configurable margin, supports physical density presets or explicit rows and
columns, and blocks probing when any program motion lies outside that area.
Physical probe density is independent from the interpolated display grid. The
same map is shown as a coordinate-labelled numeric matrix and as an exaggerated
Three.js mesh with low-to-high color contrast, probe points, perimeter, program
outline, top/isometric views, and optional interpolation wireframe.

The ordinary 19.1 mm touch plate setting is never inherited by a heightmap.
Heightmap contact is either direct conductive stock with zero offset or an
explicit fixed plate covering every point.

## Consequences

- A failed or cancelled replacement cannot destroy the last completed map.
- A storage failure cannot leave an unjournaled probing operation moving.
- A map cannot follow a different workpiece merely because the same machine
  profile remains selected.
- An unhomed machine cannot automatically reconstruct XY after power loss; map
  data survives, but applying it requires manual work-zero restoration.
- Numeric values never use `NaN`; unmeasured cells are visibly empty.
- Display interpolation can become smoother without increasing probe motion.
- Actual G-code compensation is a separate typed transformation boundary. Until
  that transform is connected to sender planning, storage application state is
  recovery intent, not permission to alter outgoing coordinates.
