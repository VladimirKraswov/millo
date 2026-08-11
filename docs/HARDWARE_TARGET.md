# Initial hardware target

This file records facts supplied by the operator. Unknown values remain unknown
until they are read from the controller or measured before cutting.

## Known configuration

- Controller protocol: GRBL.
- Motion: three axes, X/Y/Z.
- Tool: milling spindle switched manually by the operator.
- Probe: touch sensor planned for Z calibration and heightmap probing.
- Homing: not installed.
- Limit switches: not installed.
- Physical emergency stop: not installed.

## Safety consequences

- Millo must never infer that the machine is homed after power-up or reconnect.
- Machine coordinates cannot be treated as a verified physical envelope without
  homing and limits.
- Approved hardware interactions are the read-only Inspector, realtime Feed
  Hold, and challenge-confirmed Soft Reset. There is still no motion or spindle
  command endpoint.
- Future motion controls must start with low-speed, short-distance, explicit jog
  actions and an operator present at the machine power control.
- Software Hold/Reset is not a replacement for a physical emergency stop.
- Automatic `M3`, `M4`, or spindle-speed control must stay disabled for this
  profile. The sender will require an explicit manual-spindle workflow.
- Probe and heightmap support must validate probe polarity and a stationary
  spindle before enabling a probing cycle.

## Values to collect before motion

Device Inspector will capture `$I`, `$$`, `$G`, and `$#`. Before the first jog,
the operator and Millo must review axis steps, direction, acceleration, maximum
rate, travel, hard/soft limits, homing flags, status-mask behavior, units, active
WCS, and probe state. Workpiece, cutter, feeds, safe Z, and touch-probe geometry
will be configured later before milling.

## Readiness interpretation

`millo-readiness` now evaluates the complete Inspector response against this
profile. A future test jog is configuration-ready only when all four read-only
queries succeed, the controller is connected and idle, XYZ tuning values are
finite and positive, `$20/$21/$22` agree with operation without sensors, `$32`
selects milling behavior, and the modal state reports millimetres plus `M5`.

The report deliberately keeps missing homing, limits, emergency stop, and manual
spindle operation visible as cautions. A green report does not establish a
physical machine envelope and does not unlock probing or arbitrary G-code.

## First hardware observation

Observed through the read-only Inspector on 2026-08-11:

- Native device: `/dev/cu.usbmodem11101` at 115200 baud.
- USB identity: `LUNYEE_4axis_Control`; this label alone does not prove an A
  axis is configured or reported.
- Firmware: `1.1f.20230316`, options `VMZHL,35,254`.
- All `$I`, `$$`, `$G`, and `$#` queries completed successfully.
- The controller reported 41 settings, 11 coordinate parameters, `G21`, `G54`,
  `G91`, and `M5` while idle.
- The visible machine position included X = -10 mm. Without homing this remains
  an unverified controller coordinate, not a safe travel boundary.

Only values visible in the operator capture are recorded here. The automated
fixture uses representative, explicitly synthetic XYZ values rather than
inventing the unseen portion of the physical controller's `$$` response.

## Test-jog preflight

Before a future test jog, the operator must confirm in the UI that the physical
spindle is off, the tool is clear of the workpiece, and the machine power control
is immediately reachable. Millo then reads a new `$I/$$/$G/$#` set and evaluates
the current controller state. A successful result creates a 15-second,
single-use backend authorization; it does not move the machine.

Feed Hold is available only while GRBL reports an active `Run`, `Jog`, or `Home`
state. Soft Reset is intentionally available in connected alarm states, but it
requires a fresh 10-second confirmation challenge. Neither control substitutes
for a physical emergency stop or verified machine travel boundaries.
