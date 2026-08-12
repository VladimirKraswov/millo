# GRBL execution audit

## Execution surface

| Family | Parser / preview | Check mode | Air run | Cutting run |
| --- | --- | --- | --- | --- |
| G0/G1 | Yes | Yes | Yes | Yes |
| G2/G3, G17/G18/G19, IJK/R | Yes, including helices and explicit-endpoint full circles | Yes | Yes | Yes |
| G4 P | Yes, timed | Yes | Yes | Yes |
| G20/G21 | Yes, preview normalized to mm | Yes | Yes | Yes |
| G40/G49/G80 | Yes | Yes | Yes | Yes |
| G54-G59 | Recorded; local preview does not apply live offsets | Yes | Yes with warning | Yes with warning |
| G61 | Nominal geometry unchanged | Accepted by parser and physical Check fixture | Policy-capable | Policy-capable |
| G64 | Rejected before execution | Physical controller returns `error:20`; parser reports a blocking error | Blocked | Blocked |
| G90/G91/G91.1 | Yes | Yes | Yes | Yes |
| G93/G94 | Yes, with time estimates | Yes | Yes | Yes |
| M0/M1 | M0 barrier; M1 conditional on explicit Optional Stop | M0 validated; M1 sent only when Optional Stop is enabled | M0 pause; optional M1 pause | M0 pause; optional M1 pause |
| Leading `/` | Retained; modal state and geometry depend on Block Delete | Included or omitted by explicit option | Included or omitted by authorized option | Included or omitted by authorized option |
| `N...*checksum` | Decimal XOR checked before normalization; N retained, checksum removed | Yes | Yes | Yes |
| M2/M30 | Program end barrier | Host-validated; not sent | Deferred until fresh Idle | Deferred until fresh Idle |
| M3/M4/S/M5 | Parsed and marked | Yes under Cutting grammar | Start/speed blocked; M5 allowed | Yes after Cutting authorization |
| M9 | Yes | Yes | Yes | Yes |
| M6 | Host-managed barrier with bounded T0-T255 | Tn validated; M6 skipped locally | Blocked | Verified operator barrier; M6 never sent |
| M7/M8 | Detected | Blocked by current hardware policy | Blocked | Blocked |
| G10/G92 | Detected as coordinate mutation | Blocked | Blocked | Blocked |
| G28/G30/G53 | Detected as reference/machine motion | Blocked | Blocked | Blocked |
| G38.x | Detected as probing | Blocked | Blocked | Blocked |
| G43.1 | Not yet a production workflow | Blocked | Blocked | Blocked |

## Stream lifecycle

- Preconditions: selected profile, serial target, connected stable state, fresh
  Inspector, explicit modal contract, policy-approved immutable parse result.
- Authorization: intent, program SHA-256, controller session, observed position,
  execution options, 30-second expiry, atomic one-time consume.
- Cutting validation: a successful full Check run mints a 15-minute certificate
  bound to the same SHA-256, execution options, reset count, and reconnect count.
  Missing, expired, changed, reset, or reconnected evidence blocks Cutting
  preflight. Air run does not require this syntax certificate.
- Optional semantics: Block Delete is applied during parsing, not by dropping
  text at dispatch; Optional Stop and Block Delete are included in preflight,
  confirmation and the consumed lease. Preview is reparsed when Block Delete
  changes.
- Dispatch: reported RX capacity minus one byte, exact command/newline accounting,
  oldest-command FIFO, periodic realtime status.
- Shutdown: typed `M5`, `M9` epilogue follows source commands and precedes a
  deferred M2/M30; their acknowledgements are explicit snapshot evidence.
- Timing: per-line millisecond estimate, correlated completed/remaining totals,
  monotonic active elapsed, Hold exclusion, terminal freeze, explicit lower
  bound when rapid timing is unknown.
- Realtime: status, Hold, Resume, Jog Cancel, confirmed Reset, and closed-set
  feed/rapid/spindle overrides share the actor owner.
- Terminal: every `ok` correlated; physical M2/M30 waits for fresh Idle; errors,
  alarm, timeout, reset, or disconnect fail closed.
- Failure data: terminal snapshots retain a closed failure kind, optional GRBL
  code, source line, and exact command after the FIFO is cleared. Text remains
  display-only compatibility data.
- Progress data: monotonic acknowledgement sequence, last acknowledged source
  line/command, acknowledgement age, and shutdown-tail completion. Snapshot
  generation is constant-time regardless of program length.
- Crash diagnostics: a monotonic run sequence identifies each sender load.
  `millo-journal` records start/state transitions, throttled progress checkpoints,
  and every terminal result in a bounded 100-entry JSON file with a backup.
  Failed/cancelled entries are explicitly non-executable recovery evidence.
- Check: serial-only typed `$C`, one command in flight, host-validated M2/M30,
  automatic verified exit, and an exact program/session certificate.
- Tool change: isolated `M6` drains the response FIFO into a host-only
  `ToolChange` state; continuation is line/tool-bound and repeats fresh
  `Idle -> Inspector -> Idle` verification. Runtime excludes operator time.

## Physical evidence

On `/dev/cu.usbmodem11101`, GRBL `1.1f.20230316`:

- `grbl-complex-check.nc`: 25/25 accepted after parser correction for full-circle
  endpoint requirements; repeated after response demultiplexing changes.
- `grbl-cutting-check.nc`: 27/27 sender steps completed on 2026-08-12, including `N` words,
  metadata-only `O2026` omission, M3/M4/S syntax, all three arc planes,
  G90/G91, G93/G94, dwell, M0/M1, full circle, and host-validated M30. The
  controller returned to Idle and an immediate Cutting preflight accepted the
  newly issued certificate. This firmware emits a reset banner while disabling
  `$C`; only the one banner created inside that verified transition is cleared.
- `grbl-path-control-check.nc`: 7/7 accepted on 2026-08-12 with `G61` exact
  path mode. The same board rejected `G64` with `error:20`, so Millo blocks
  that LinuxCNC-style command during parsing instead of failing mid-program.
- `grbl-tool-change-check.nc`: 16/16 sender steps completed on 2026-08-12.
  Physical GRBL validated `T2` and mixed geometry, Millo acknowledged the M6
  barrier locally, and the controller returned to Idle.
- `grbl-stream-semantics-check.nc`: 10/10 sender steps completed on 2026-08-12
  with Optional Stop and Block Delete enabled. Host checksum validation passed,
  the optional `N30` block was absent from the wire, `N50 M1` and the injected
  M5/M9 shutdown tail were accepted, and the controller returned to Idle.
- `air-square-20mm.nc`: 10/10 physical Air run, planner drained, WPos returned
  to XYZ zero.
- `millo-solar-guilloche.nc`: 1045/1045 physical Cutting run on 2026-08-12,
  after a same-session 1045/1045 `$C` certificate. The 1034-motion, 1018.317 mm
  dense engraving completed in 226.5 s through a 253-byte RX window and parked
  in `Idle` at G54 WPos X30 Y30 Z3. This run verifies the status-refreshed
  silence watchdog that replaced the earlier false absolute timeout at line 44.
- Realtime override smoke: observed `Ov:110,50,99`, then verified restore to
  `Ov:100,100,100`.

This proves the currently enabled execution surface. It does not authorize the
blocked hardware workflows listed above.

## Completion boundary

The sender core is complete for the declared first-machine profile: GRBL 1.1,
XYZ motion, manual spindle, no homing/limits/probe, whole-file certified Check,
Air and Cut plans, and files using only the enabled command surface above. Completion
means immutable parse-to-plan input, bounded/correlated streaming, realtime
safety and overrides, terminal drain, typed failures, runtime timing, Mock
fault coverage, and physical Check/Air evidence all pass together.

This statement deliberately excludes arbitrary partial-file start, probing,
heightmaps, coolant, machine/reference-coordinate movement, coordinate
mutation, and tool-length offsets. Guided restart of a Millo-started interrupted
job is the narrower ADR 0038 workflow and requires durable physical-line
evidence plus a new whole-program safety pipeline. The other features can change
physical meaning or depend on absent hardware, so each remains a separate typed
workflow rather than an unguarded sender option.
