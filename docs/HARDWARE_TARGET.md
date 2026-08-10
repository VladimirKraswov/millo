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
- The current read-only Inspector is the only approved hardware interaction in
  this slice; it cannot send motion or spindle commands.
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
