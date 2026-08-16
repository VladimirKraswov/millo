# 0052. Workpiece Heightmap Session

Status: accepted

## Coordinate and Z-datum Amendment

Each new map stores active G54-G59 and effective WCO. The first successful map
contact establishes and verifies surface Z0. Compensation uses relative height
(`sample - first sample`), never raw work-coordinate Z. A G10 mutation, WCS/WCO
mismatch, or legacy map without a binding makes the map display-only.

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
validates a bounded XY perimeter and grid, computes bounded relative Z/XY
deltas, executes `G38.3`, waits for fresh `Idle` after terminal
acknowledgement, reads `PRB` through `$#`, records surface Z without writing
`G10`, and raises before the next point. Hold, Resume, and Soft Reset remain
preemptive. Each internal jog must finish at its expected work target within
0.05 mm. Failure sends Feed Hold and Soft Reset, publishes a quarantined failed
snapshot, and issues no automatic recovery motion from an untrusted coordinate.
An ordinary `G38.3` no-contact result is the narrow exception: when fresh status
still proves the same connected Idle controller and work coordinates, the actor
raises to its measured-safe transit plane and restores modal state without a
Soft Reset. Any failure during that recovery falls back to quarantine.

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

The UI does not classify stock as `PCB` or `relief`. That distinction did not
select a different probing algorithm and therefore looked like a processing
mode without changing the operation. Instead, the primary surface setup exposes
the two physical bounds that do change movement: the first Z0 contact distance
and the maximum downward variation searched at each grid point. Contact offset,
feeds, clearance, and display layers remain on-demand settings.

The ordinary 19.1 mm touch plate setting is never inherited by a heightmap.
Heightmap contact is either direct conductive stock with zero offset or an
explicit fixed plate covering every point. In fixed-plate mode the same plate
thickness establishes the first Z0 contact and offsets every grid contact; the
UI cannot combine it with a second removable calibration-plate offset.

Direct probing begins with an explicit surface calibration at the highest point
of the intended perimeter. That contact establishes Z0; the operator does not
measure arbitrary stock thickness. Safe travel is above that Z0. A separate
bounded surface-variation value controls how far below the highest point each
grid contact may search, which supports relief while preserving a finite miss
condition. Per-machine UI drafts survive dialog closure, but successful physical
calibration never does: any close, reset, disconnect, or relevant setting change
requires a new touch. In particular, changing the perimeter or contact setup
invalidates the highest-point calibration; changing only density or rendering
does not.

`maximum probe depth` is defined as reserve below calibrated work Z0. It is not
the total move from safe clearance: the actual bounded `G38.3` travel is
`clearance + reserve`. On a miss or operator cancellation, the pending map keeps
its contiguous measured prefix beside the previous complete map. Explicit
resume creates a new operation sequence, revalidates the selected profile,
fresh Idle/A5/WCS/modal state, and continues at the first empty sample. The grid,
contact mode and measured values are immutable; only the lower search reserve
may increase. A persisted Running draft after process or power loss is never
continued automatically and requires the same explicit resume action.

The settings column owns its own scroll area and a stable action dock, so Start,
Pause, Stop, progress, and failures remain visible without moving the surrounding
layout. Below 900 px, settings precede the preview because completing the setup
is the next operator action. Saved map geometry always renders from its own
stored perimeter; editing the next plan cannot stretch old measurements into a
false surface.

Applying a map is deliberately separate from selecting an A5 probe workflow.
Off, Work Zero, and Heightmap control which new measurement the operator can
start. A saved map is enabled for one mounted workpiece from the Program
readiness card with `Компенсировать по карте`. The switch is visible only for
the selected machine profile and includes the map identity, physical grid,
perimeter, and measured Z range. It is disabled when the loaded XY toolpath is
outside the measured perimeter. Restart disarms it. A successful one-point Z
zero also disarms it because that operation changed the coordinate datum in
which the map was measured; the map remains available for inspection.

Map application is a typed Rust sender transformation, not a UI offset. The
map ID is part of `ProgramExecutionOptions`, the GRBL Check certificate, the
one-use run authorization, and the recovery seed. Tauri resolves that ID to the
active immutable map for preflight, `$C`, and physical dispatch. A replacement,
profile mismatch, disabled session, missing sample, unsupported modal state, or
motion outside the map fails closed before sender start.

The transformer requires metric absolute G94 motion in the XY plane. It
linearizes previewed arcs and subdivides long moves to at most half the physical
probe spacing, bounded to 0.25..1 mm, then bilinearly interpolates measured
surface Z. Every sample is already expressed in the active work coordinate
system after the probe-established Z0, so compensation adds the interpolated
absolute surface Z. It does not subtract the first grid sample: the calibrated
highest point and the first serpentine point can be in different places.
Nominal cutting Z at or below zero receives the full correction.
Between Z0 and the configured clearance the correction fades linearly to avoid
an abrupt Z step; at and above clearance it is zero. Safe rapid motion may be
outside the map, but every point that receives non-zero compensation must be
inside it. The transformed stream is bounded to 200,000 lines and retains the
original source-line identity for diagnostics and recovery.

Completed maps are checked for neighboring discontinuities. Millo compares the
largest horizontal or vertical neighbor delta with the median and marks a jump
of at least 0.5 mm and eight times the median as suspicious. The data is never
silently smoothed because a real stepped surface is possible. Preflight emits a
caution, and the final start dialog requires confirmation that probe contact was
valid and that neither the cutter nor its stick-out changed after probing.

## Consequences

- A failed or cancelled replacement cannot destroy the last completed map.
- A storage failure cannot leave an unjournaled probing operation moving.
- A map cannot follow a different workpiece merely because the same machine
  profile remains selected.
- An unhomed machine cannot automatically reconstruct XY after power loss; map
  data survives, but applying it requires manual work-zero restoration.
- Numeric values never use `NaN`; unmeasured cells are visibly empty.
- Display interpolation can become smoother without increasing probe motion.
- GRBL Check and physical execution consume the same compensated sender plan.
- Changing the map selection invalidates preflight and the prior `$C`
  certificate, preventing a checked path from differing from the dispatched
  path.
