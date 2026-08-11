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

## Safety consequences

- Millo must never infer that the machine is homed after power-up or reconnect.
- Machine coordinates cannot be treated as a verified physical envelope without
  homing and limits.
- Approved hardware interactions are the read-only Inspector, realtime Feed
  Hold, challenge-confirmed Soft Reset, guarded single-axis step jog, and typed
  per-axis work zero while Idle. A serial real-run preflight may additionally
  reparse a file and repeat status plus Inspector reads. There is no arbitrary
  motion or spindle command endpoint. The program sender is hard-disabled for
  serial targets and runs only against the deterministic Mock GRBL transport.
- Step jog is deliberately limited to `0.01..1.00 mm` at `10..100 mm/min`, with
  one fresh preflight lease required for every attempt and the operator present
  at the machine power control.
- Software Hold/Reset is not a replacement for a physical emergency stop.
- Automatic `M3`, `M4`, or spindle-speed control stays disabled for this
  profile. The current dry-run policy also blocks coolant, probing, M6,
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
connection preset. That path is convenience metadata, not cryptographic device
identity. The automated Mock fixture continues to use representative synthetic
XYZ values.

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
