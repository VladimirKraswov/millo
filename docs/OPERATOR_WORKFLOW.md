# Operator Workflow

This is the behavior contract shared by the Rust core and React UI. It defines
observable states and allowed actions, not component layout.

## Sources of Truth

| Fact | Owner | Rule |
| --- | --- | --- |
| Connection, mode, position, WCO, reset/reconnect | GRBL snapshot | Never inferred from a click |
| Geometry and warnings | `millo-gcode` | Any edit creates a new program identity |
| GRBL Check | `millo-run::ProgramCheckGate` | Bound to program, options and controller session |
| Surface compensation | `millo-heightmap::SurfaceSession` | Bound to profile, G54-G59 and WCO |
| Streaming progress | `millo-sender` | ACK-driven; UI never advances optimistically |
| Interrupted work | `millo-recovery` | Affects the next run, not positioning or diagnostics |

## Job States

| State | Primary action | Allowed | Blocked |
| --- | --- | --- | --- |
| No program | Load G-code | Jog, probe, diagnostics | Check and run |
| Preparing | Check with GRBL | Preview, edit, run options, datum | Run |
| Checking | Stop check | Preview, logs, realtime Stop | Editing and motion |
| Ready | Start | Preview and final confirmation | Program mutation |
| Final confirmation | Start / Cancel | Physical confirmations | Background start |
| Running | Hold / Stop | Logs, preview, table, realtime controls | Jog, G10, settings writes |
| Held | Resume / Stop | Logs, preview, table | Jog and G10 |
| Stopping | Wait | Realtime Stop and logs | New motion |
| Interrupted | Return to zero / resolve recovery | Positioning, preview, logs | New run until recovery is resolved |
| Completed | Return to zero / run again | Positioning, edit, new file | Nothing remains implicitly active |

`Stop` is immediate and never requires a second click. Recovery records do not
disable manual positioning after the sender becomes terminal.

## Safe Return to Work Origin

The operator sees one **Return to work zero** command. It does not write G10 or
change G54-G59. The Rust actor owns this transaction:

1. Read fresh stable `Idle` and active G54-G59.
2. Raise Z to at least the requested clearance; never lower it before XY travel.
3. Move X and Y together to work `0,0`.
4. Move Z to the already stored work `0`.
5. Wait for `Idle` and verify X/Y/Z after every stage.

Any failed stage aborts the remainder. Separate X0/Y0/Z0 buttons are not part
of the primary operator workflow.

## Z Datum

There are three valid Z states:

- **Unknown**: Z may be set manually or found with the probe.
- **Probe established**: successful contact wrote and verified G10 L20. The
  normal zero dialog protects Z and offers XY only.
- **Heightmap established**: surface search/first map contact establishes Z0;
  all map values are deviations from it.

Selecting a probe mode does not establish Z0. Only successful verified contact
does. Reset, reconnect or coordinate-binding mismatch invalidates the UI proof.
An active map with at least one verified contact and a binding that still
matches the controller's live G54-G59 and WCO is also persistent proof of Z0;
reopening the surface dialog must not ask the operator to zero Z again.
Within the same controller session the sequence `probe Z -> set X/Y -> measure
map` retains that verified Z datum. The first grid contact records surface
deviation and does not issue a second G10 Z write.
Returning to work zero moves to the stored Z0 and never creates a new one.

## Heightmap States

| State | Execution behavior |
| --- | --- |
| No map | Nominal program Z |
| Draft | Resume only with the same grid and coordinate binding |
| Complete, disabled | Display only |
| Complete, enabled | Map ID is part of Check and run authorization |
| Stale | Display only; a new measurement is required |

The first successful point is the reference (`delta Z = 0`). Every other point
uses `measured Z - reference Z`. Changing G54-G59 or WCO makes the map stale.
Backend checks live WCS/WCO before Check and start, so UI state cannot bypass it.

## Run Variants

Without a map: parse -> GRBL Check -> final physical confirmation -> nominal run.

With a map: establish Z0 -> measure -> enable current map -> Check compensated
plan -> confirm stock/tool, removed probe wires and running manual spindle -> run.

For a cutting run, the final confirmation always shows the current workpiece
map when one exists. A usable but disabled map is a red warning with its measured
Z range and an explicit **Compensate using heightmap** switch. The operator can
still deliberately start without compensation, but the primary action says
**Start processing without map** instead of silently behaving like a nominal
run. Enabling or disabling the map from this dialog never starts motion: Millo
invalidates the previous certificate, repeats GRBL Check with the exact new
execution options, and only then reopens final confirmation.

The final modal always offers **Start** and **Cancel** and contains only facts
that software cannot observe.

## Processing Depth Correction

**Processing** is the real G-code execution mode for engraving, milling,
contouring, and drilling. **Motion check** physically follows the program with
the cutting setup disabled; GRBL Check remains the separate firmware-only
validation without motion.

For a program with cutting motion below work Z0, the launch panel can enable a
signed Z offset. It is disabled by default and starts at `0.000 mm`, so no
depth is inferred from an arbitrary program point. `-0.100 mm` makes every
negative non-rapid Z point exactly 0.100 mm deeper; `+0.100 mm` makes it exactly
0.100 mm shallower. Rapid moves, safe Z, and points originally at or above Z0
are unchanged.

The offset is stored as exact micrometres and bounded to +/-10 mm. Heightmap
compensation is applied after the nominal Z offset. Changing the correction
invalidates the previous preflight and GRBL Check certificate because the
execution options and physical trajectory have changed.

The correction belongs to the loaded job. Selecting another G-code file disables
it and restores `0.000 mm`; recovery of the same interrupted job keeps its bound
execution options.

## Errors

Command errors persist until dismissed. Successful periodic polling must never
erase them. Every error notification offers a direct path to the diagnostic log.
