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
| G61/G64 | Nominal geometry unchanged | Fixture pending physical support proof | Policy-capable | Policy-capable |
| G90/G91/G91.1 | Yes | Yes | Yes | Yes |
| G93/G94 | Yes, with time estimates | Yes | Yes | Yes |
| M0/M1 | Program barrier | Validated without operator pause | Sender pause state | Sender pause state |
| M2/M30 | Program end barrier | Yes | Deferred until fresh Idle | Deferred until fresh Idle |
| M3/M4/S/M5 | Parsed and marked | Yes under Cutting grammar | Start/speed blocked; M5 allowed | Yes after Cutting authorization |
| M9 | Yes | Yes | Yes | Yes |
| M6, M7/M8 | Detected | Blocked by current hardware policy | Blocked | Blocked |
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
- Check: serial-only typed `$C`, one command in flight, automatic verified exit.

## Physical evidence

On `/dev/cu.usbmodem11101`, GRBL `1.1f.20230316`:

- `grbl-complex-check.nc`: 25/25 accepted after parser correction for full-circle
  endpoint requirements; repeated after response demultiplexing changes.
- `grbl-cutting-check.nc`: 26/26 accepted on 2026-08-12, including `N` words,
  metadata-only `O2026` omission, M3/M4/S syntax, all three arc planes,
  G90/G91, G93/G94, dwell, M0/M1, full circle, and M30; returned to Idle.
- `air-square-20mm.nc`: 10/10 physical Air run, planner drained, WPos returned
  to XYZ zero.
- Realtime override smoke: observed `Ov:110,50,99`, then verified restore to
  `Ov:100,100,100`.

This proves the currently enabled execution surface. It does not authorize the
blocked hardware workflows listed above.
