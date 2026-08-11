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
| M0/M1 | Program barrier | Validated without operator pause | Sender pause state | Sender pause state |
| M2/M30 | Program end barrier | Yes | Deferred until fresh Idle | Deferred until fresh Idle |
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
  30-second expiry, atomic one-time consume.
- Dispatch: reported RX capacity minus one byte, exact command/newline accounting,
  oldest-command FIFO, periodic realtime status.
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
- Check: serial-only typed `$C`, one command in flight, automatic verified exit.
- Tool change: isolated `M6` drains the response FIFO into a host-only
  `ToolChange` state; continuation is line/tool-bound and repeats fresh
  `Idle -> Inspector -> Idle` verification. Runtime excludes operator time.

## Physical evidence

On `/dev/cu.usbmodem11101`, GRBL `1.1f.20230316`:

- `grbl-complex-check.nc`: 25/25 accepted after parser correction for full-circle
  endpoint requirements; repeated after response demultiplexing changes.
- `grbl-cutting-check.nc`: 26/26 accepted on 2026-08-12, including `N` words,
  metadata-only `O2026` omission, M3/M4/S syntax, all three arc planes,
  G90/G91, G93/G94, dwell, M0/M1, full circle, and M30; returned to Idle.
- `grbl-path-control-check.nc`: 7/7 accepted on 2026-08-12 with `G61` exact
  path mode. The same board rejected `G64` with `error:20`, so Millo blocks
  that LinuxCNC-style command during parsing instead of failing mid-program.
- `grbl-tool-change-check.nc`: 16/16 sender steps completed on 2026-08-12.
  Physical GRBL validated `T2` and mixed geometry, Millo acknowledged the M6
  barrier locally, and the controller returned to Idle.
- `air-square-20mm.nc`: 10/10 physical Air run, planner drained, WPos returned
  to XYZ zero.
- Realtime override smoke: observed `Ov:110,50,99`, then verified restore to
  `Ov:100,100,100`.

This proves the currently enabled execution surface. It does not authorize the
blocked hardware workflows listed above.

## Completion boundary

The sender core is complete for the declared first-machine profile: GRBL 1.1,
XYZ motion, manual spindle, no homing/limits/probe, whole-file Check, Air and
Cut plans, and files using only the enabled command surface above. Completion
means immutable parse-to-plan input, bounded/correlated streaming, realtime
safety and overrides, terminal drain, typed failures, runtime timing, Mock
fault coverage, and physical Check/Air evidence all pass together.

This statement deliberately excludes partial-file restart, probing,
heightmaps, coolant, machine/reference-coordinate movement, coordinate
mutation, and tool-length offsets. Those features can change physical meaning
or depend on absent hardware, so each remains a separate typed workflow rather
than an unguarded sender option.
