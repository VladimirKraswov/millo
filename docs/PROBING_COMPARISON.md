# Probing and Heightmap Comparison

This audit compares Millo with Candle, CNCjs, bCNC, UGS, and the GRBL 1.1
protocol. It records design evidence, not a compatibility target: Millo keeps a
typed Rust operation actor instead of streaming a generated probing program.

## Kept in Millo

- Candle and bCNC derive the map perimeter from loaded G-code, expose the probe
  grid, use serpentine travel, raise to safe Z between points, and interpolate a
  rectangular surface. Millo keeps these operator-visible behaviors.
- CNCjs separates physical probe spacing from visual interpolation, offers a
  one-point test before a full scan, updates progress and surface statistics,
  and can recover probe data after its UI reconnects. Millo already separates
  physical and render grids, establishes Z0 before scanning, checkpoints every
  sample, and restores a disarmed session after restart.
- UGS validates a rectangular mesh and applies bilinear compensation. Millo
  uses the same interpolation class and rejects incomplete or invalid maps
  before compensation can be enabled.
- GRBL `PRB` is stored in machine coordinates and includes a success flag.
  Millo parses both, converts through the active WCS, G92, and TLO, and rejects
  failed or malformed contacts.

## Deliberate differences

- Candle and bCNC normally use `G38.2`; a missed contact raises `ALARM:5` and
  interrupts the generated probe job. Millo uses bounded `G38.3`, verifies the
  `PRB:...:1` success flag itself, records the exact failed point, and attempts
  safe-Z and modal cleanup without requiring an avoidable alarm unlock.
- Millo has a durable prepare/persist/commit barrier. No probe motion begins
  until the pending session is on disk. A completed map replaces the previous
  workpiece map atomically; partial data remains diagnostic only.
- A stale UI snapshot cannot authorize probing. The actor obtains fresh status.
  A short trailing `Run` or `Jog` is allowed to settle to `Idle` for at most
  three seconds with status queries only; alarms, reset notices, connection
  loss, an active sender, and all other modes remain blockers. The wait runs as
  individual actor requests, so realtime Hold and Reset are not starved.

## Follow-up improvements

UGS contributes two useful ideas for a later explicit settings change:

1. Clamp probe travel against the remaining soft-limit distance when homing and
   soft limits provide trustworthy machine coordinates.
2. Offer optional two-stage touch-off: fast approach, retract, then a slower
   precision touch. It must remain opt-in because it adds travel, time, and new
   feed/retract settings.

Additional probe modes from CNCjs and bCNC (`G38.4`/`G38.5`, edge finding, hole
centering, and tool setters) belong in separate typed workflows. They should not
complicate the first-party Z-zero and heightmap controls.

## Sources

- GRBL 1.1 commands and interface documentation:
  <https://github.com/gnea/grbl/tree/master/doc/markdown>
- Candle heightmap workflow: `src/candle/frmmain.cpp` in the Denvi/Candle source.
- CNCjs Autolevel and Probe widgets: <https://github.com/cncjs/cncjs>
- bCNC probe/autolevel implementation: <https://github.com/vlachoudis/bCNC>
- UGS ProbeService, SurfaceScanner, and MeshLeveler:
  <https://github.com/winder/Universal-G-Code-Sender>
