# Initial hardware target

This file records facts supplied by the operator. Unknown values remain unknown
until they are read from the controller or measured before cutting.

## Known configuration

- Controller protocol: GRBL.
- Motion: three axes, X/Y/Z.
- Tool: milling spindle switched manually by the operator.
- Probe: no sensor is installed or connected. A touch sensor may be added later
  for Z calibration and heightmap probing.
- Homing: not installed.
- Limit switches: not installed.
- Physical emergency stop: not installed.
- Selected profile: `LUNYEE CNC` (`machine-0001`).
- Firmware-configured travel: X `500.000 mm`, Y `500.000 mm`, Z `200.000 mm`.
- GRBL `[OPT]` on 2026-08-11: flags `VMZHL`, planner `35`, RX `254` bytes;
  physical sender window `253` bytes.

## Safety consequences

- Millo must never infer that the machine is homed after power-up or reconnect.
- Machine coordinates cannot be treated as a verified physical envelope without
  homing and limits.
- Approved hardware interactions are the read-only Inspector, realtime Feed
  Hold, challenge-confirmed Soft Reset, guarded single-axis step jog, and typed
  per-axis work zero while Idle. A serial real-run preflight may additionally
  reparse a file and repeat status plus Inspector reads. The first-cut gate may
  repeat those reads and issue an in-memory 30-second lease after mode-specific
  operator confirmations. There is no arbitrary motion or raw-line endpoint.
  A policy-approved file can now start on a profile-bound serial target only
  after that lease is atomically consumed.
- The sender covers terminal responses, program pause/end, Hold/resume, reset,
  polling failure, and link loss. Physical completion requires a fresh `Idle`.
- Serial Check run may send a parser-approved program through GRBL `$C` with no
  motion. It uses one outstanding line and verifies the return to `Idle` on
  every terminal path; it is not permission for a physical Air or Cutting run.
- Step jog is deliberately limited to `0.01..1.00 mm` at `10..100 mm/min`, with
  one fresh preflight lease required for every attempt and the operator present
  at the machine power control.
- Software Hold/Reset is not a replacement for a physical emergency stop.
- The spindle is still operated manually for this profile. Cutting-file
  `M3/M4/S` words are accepted by policy only after the operator confirms that
  manual spindle workflow; they do not prove that the spindle is physically
  running. Air run rejects those words. Both modes block coolant, probing, M6,
  machine/reference-coordinate moves, and coordinate mutation before an opaque
  plan can be created.
- Probe, `$H`, and heightmap motion must remain unavailable while their physical
  hardware is absent. A future probe slice must first validate wiring, polarity,
  input transitions, and a stationary spindle before enabling a probing cycle.

## Values to collect before motion

Device Inspector captures `$I`, `$$`, `$G`, and `$#`. Before milling, the operator
and Millo must review axis steps, direction, acceleration, maximum rate, travel,
hard/soft limits, homing flags, status-mask behavior, units, and active WCS.
Workpiece, cutter, feeds, and safe Z will be configured before a dry run. Probe
geometry cannot be configured or trusted until a sensor exists.

The Program preflight now collects a second fresh status after Inspector and
combines motion-critical readiness with the strict motion-only file policy. A
clear report still does not establish physical travel bounds. Stock dimensions,
cutter, verified XYZ work zero, safe Z, cutting depth/feed, manual spindle state,
and a reachable power control remain required before a future run authorization.

The implemented launch dialog records the intent-specific confirmations and then
repeats the whole preflight in the command actor. Its single-use lease is bound
to the parsed program fingerprint, controller session, and observed positions.
It expires after 30 seconds and does not itself send G-code. Start consumes it
once inside the actor. Cutting depth and feed remain properties of the reviewed
program, not operator-checkbox overrides.

## Readiness interpretation

`millo-readiness` now evaluates the complete Inspector response against this
profile. A future test jog is configuration-ready only when all four read-only
queries succeed, the controller is connected and idle, XYZ tuning values are
finite and positive, `$20/$21/$22` agree with operation without sensors, `$32`
selects milling behavior, and the modal state reports millimetres plus `M5`.

The report deliberately keeps missing homing, limits, emergency stop, and manual
spindle operation visible as cautions. A green report does not establish a
physical machine envelope and does not unlock probing or arbitrary G-code.

## Work-zero procedure

Zero X/Y/Z is an offset update, not a motion command. The operator places the
tool at the intended datum, explicitly confirms that fact, and selects one axis.
Millo requires a fresh Idle state, reads the active G54-G59 system, sends one
typed `G10 L20` command for that system, reads `$#`, and verifies the resulting
work coordinate. The UI cannot select another coordinate system or submit a raw
line. This path is automated against Mock GRBL; no physical work-zero command was
executed as part of the implementation slice.

## First hardware observation

Observed through the read-only Inspector on 2026-08-11:

- Native device: `/dev/cu.usbmodem11101` at 115200 baud.
- USB identity: `LUNYEE_4axis_Control`; this label alone does not prove an A
  axis is configured or reported.
- Firmware: `1.1f.20230316`, options `VMZHL,35,254`.
- A dedicated profile import later read `$130=500.000`, `$131=500.000`, and
  `$132=200.000` and stored those values as the selected machine travel.
- All `$I`, `$$`, `$G`, and `$#` queries completed successfully.
- The controller reported 41 settings, 11 coordinate parameters, `G21`, `G54`,
  `G91`, and `M5` while idle.
- A fresh guarded smoke inspection read `$20=0`, `$21=1`, and `$22=1`. Hard
  limits and homing are therefore enabled in firmware despite the recorded lack
  of physical switches.
- The visible machine position included X = -10 mm. Without homing this remains
  an unverified controller coordinate, not a safe travel boundary.
- The 2026-08-11 hardware smoke stopped at readiness because of the `$21/$22`
  conflict. No `$J=` command and no physical movement occurred during that first
  attempt.
- After separate operator confirmation, Millo wrote `$21=0` and `$22=0`, received
  `ok` for both, and verified both values through a new complete Inspector read.
- A subsequent fresh preflight authorized exactly
  `$J=G91 G21 X0.100 F10.000`. The controller returned to `Idle` and reported
  deltas X `+0.100 mm`, Y `+0.000 mm`, Z `+0.000 mm`.
- Separate Y and Z processes each repeated connection, settings verification,
  Inspector, readiness, and one-use authorization. Y reported deltas X `+0.000`,
  Y `+0.100`, Z `+0.000 mm`; only after its successful `Idle` return, Z reported
  X `+0.000`, Y `+0.000`, Z `+0.100 mm`. Both used `10 mm/min`.

The persistent profile also stores `/dev/cu.usbmodem11101` and 115200 baud as a
connection preset. The USB device reports no usable unique serial number, so its
identity confidence is `portBound`: VID/PID, product, and the `/dev/cu.*` path
are combined. Firmware remains visible observation metadata but is not part of
the stable key. This distinguishes the current setup but cannot
reliably distinguish two identical controllers swapped onto the same port;
Millo must ask the operator when matches are ambiguous. No ID is written into a
GRBL startup block. The automated Mock fixture uses a synthetic fingerprint.

## First file-based Air run

The reviewed fixture is `fixtures/programs/air-square-20mm.nc`. It contains four
linear moves at `100 mm/min`, covers X `0..20 mm` and Y `0..20 mm`, does not move
Z, begins with `M5 M9`, contains no spindle-speed or spindle-start word, and
returns to X0 Y0 before `M30`.

Parser, policy, sender, and top/isometric preview checks report four motions,
`80.0 mm` total path, `20.0 x 20.0 x 0.0 mm` bounds, and zero warnings. The
production sender completed the same file against deterministic Mock GRBL and
waited for a fresh final `Idle`.

A read-only hardware preflight on 2026-08-11 passed with zero blockers and the
three expected cautions for unverified envelope, manual spindle, and physical
setup. It sent no program line. GRBL reported work position X `-9.400 mm`, Y
`-0.500 mm`, Z `+0.500 mm`, so the confirmed-run harness refused to start. The
operator must position the empty spindle at a safe origin and confirm that the
harness may set it as XYZ work zero, with at least 20 mm of positive X/Y
clearance, before the physical Air run can consume an authorization.

A later read-only capability check on the same date parsed `[OPT:VMZHL,35,254]`
into typed firmware capabilities and selected a 253-byte usable RX window. It
again passed with zero blockers, sent no program line, and observed WPos XYZ
`0.000 mm`.

The first confirmed attempt on 2026-08-11 set the current position as G54 XYZ0
and verified every axis at `0.000 mm`. GRBL acknowledged the safety preamble,
modal setup, all four motion lines, and the final `M5 M9`, but the original
sender dispatched `M30` while motion was still queued. GRBL correctly delayed
that synchronizing acknowledgement; the generic two-second command timeout
failed the run at line 11. The harness requested Hold and challenge-confirmed
Soft Reset, and the next read-only connection reported `Alarm`. No final work
position was accepted and the attempt is not recorded as a completed Air run.

The physical sender now holds `M2/M30` as a terminal barrier without writing it,
continues status polling and accepting realtime safety requests, and dispatches
the terminal command only after a fresh `Idle`. Mock regression tests cover
Idle gating, Hold/Resume, Reset cancellation, and a correlated terminal-command
timeout. Clearing that Alarm and attempting another motion required a new
operator confirmation.

A second, separately confirmed attempt on 2026-08-11 used the typed Unlock
workflow. It observed `Alarm`, sent exactly `$X`, reread `Idle`, and then found
the interrupted position at work X `+3.386 mm`, Y `+0.000 mm`, Z `+0.000 mm`.
The harness explicitly set that current position as the new G54 XYZ0 and
verified all three axes at `0.000 mm` before issuing a new one-use lease. The
physical sender acknowledged all 10 lines, remained responsive in `Draining`
while the four sides completed, dispatched `M30` only after fresh `Idle`, and
finished at WPos X `+0.000`, Y `+0.000`, Z `+0.000 mm`. This is the first
successful physical file-based Air run for Millo.

## Physical GRBL Check run

The complex regression fixture is
`fixtures/programs/grbl-complex-check.nc`. It contains 12 preview motions and
`163.190 mm` of cutting geometry across G17, G18, and G19, including helices,
an explicit-endpoint full circle, absolute/incremental distance, G93/G94, and
G4. The sender adds `M5/M9`, producing 25 checked commands.

The first board run on 2026-08-11 stopped at source line 10 when firmware
returned `error:26` for `G2 I-5 J0`. This established that the installed GRBL
requires an explicit XYZ target word even when a full circle ends at its start.
Millo's parser now blocks center-only arcs and the fixture uses
`G2 X30 Y0 I-5 J0`.

The immediate repeat accepted all 25 commands, reported the physical
`[OPT]` RX value of 254 bytes, and automatically toggled from `Check` back to
fresh `Idle`. No Hold, Soft Reset, spindle activation, or coordinate-changing
motion was executed.

At each connection Millo treats the controller's complete `$$` response as the
truth and keeps a duplicate in
`~/Library/Application Support/io.millo.desktop/machines/machine-0001.settings.json`.
The session baseline supports rollback without pushing stale local values back
to the machine. Reconnect creates a new baseline and retains the preceding one
as a bounded revision.

## Test-jog preflight

Before each test jog, the operator must confirm in the UI that the physical
spindle is off, the tool is clear of the workpiece, and the machine power control
is immediately reachable. Millo then reads a new `$I/$$/$G/$#` set and evaluates
the current controller state. A successful result creates a 15-second,
single-use backend authorization. Selecting X, Y, or Z and pressing one direction
button consumes it and sends one incremental metric `$J=` command. Regardless of
success or failure, another movement requires another preflight.

Feed Hold is available only while GRBL reports an active `Run`, `Jog`, or `Home`
state. Soft Reset is intentionally available in connected alarm states, but it
requires a fresh 10-second confirmation challenge. Neither control substitutes
for a physical emergency stop or verified machine travel boundaries.
Jog Cancel is available while GRBL reports `Jog`; it sends realtime `0x85` and
does not grant permission for another movement.

The reproducible step-motion procedure is documented in `docs/TESTING.md`. It
accepts exactly one XYZ axis per process, uses `+0.10 mm` at `10 mm/min`, verifies
that only the selected coordinate changed, and requires separate command-line
confirmations for persistent configuration and physical motion. Testing another
axis starts a new process and therefore repeats connection, Inspector, readiness,
and one-use authorization checks.
