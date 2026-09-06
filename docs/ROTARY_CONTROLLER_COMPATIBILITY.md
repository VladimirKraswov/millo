# Rotary Controller Compatibility

Scope: protocol evidence and the opt-in `millo-mock` XYZA firmware fixture.
Extended scope: standalone virtual serial opt-in and guarded typed Zero A.
Reviewed 2026-09-06 against primary upstream source. No physical controller was
queried or moved. Mock results are not hardware qualification.

## Detection Contract

`millo_grbl::rotary_axis_evidence(inspection, status)` accepts optional references
to `DeviceInspection` and `MachineState`, returning:

| Result | Required evidence |
| --- | --- |
| `Some(ReportedAxes)` | Successful `$I` or `$I+` response with `[AXS:4:XYZA]` |
| `Some(StatusPosition)` | Finite fourth MPos or WPos coordinate, without an explicit conflicting axis declaration |
| `None` | Missing, malformed, unsupported, or contradictory evidence |

An explicit non-XYZA declaration vetoes the status fallback. A supplied
three-coordinate MPos, WPos, or WCO also vetoes four-axis evidence. Status
positions must be finite. A WCO-only frame, accessory field `A:S`, firmware name,
`[OPT:...]` axis count, `$103`, machine profile, or successful jog is not evidence.
Five/six-axis and remapped XYZU layouts are deliberately outside this contract.

The helper reads the original identity response lines, so no domain schema
extension is needed. Raw identity lines remain in `DeviceInspection.responses`.
Callers must use inspection and status from the **same current connection/reset
epoch**, discard cached evidence after disconnect/reset, and fail closed before
streaming A when evidence is absent. `GrblHAL ...` and `Grbl ...` startup banners
are both classified as resets. The helper itself does not cache or authorize
motion, and does not replace profile, readiness, or execution guards.

Four numeric coordinates establish protocol shape, not degrees, gearing, rotary
limits, axis assignment, homing, or a safe machine envelope. A fourth-vector
fallback assumes the standard XYZA order; a known remapping must veto it.
Angular configuration and feed semantics require separate controller-specific
qualification. Never try a physical A jog to discover axis support.

### Identity and Angular Gates

The execution owner should combine axis evidence with an enabled/finite rotary
profile and explicit firmware semantics. Recognize exact successful raw identity
markers, not arbitrary user-editable build-info substrings:

| Firmware | Identity | Angular proof |
| --- | --- | --- |
| grblHAL | `[FIRMWARE:grblHAL]` | Reported `$376` integer has bit 0 set: `value & 1 != 0` |
| FluidNC | `FluidNC` token in the version portion of `[VER:... FluidNC ...:build info]` | Examined upstream defines ABC as angular |
| MilloVirtual | `[FIRMWARE:MilloVirtual]` plus `[AXS:4:XYZA]` | Explicit opt-in mock contract below; G0/G1 only for rotary |

**For grblHAL `$376`, A is 1, not 8.** Its public ABC-relative mask is shifted
left by three internally and right by three for reports. Thus internal A mask 8
is reported as `$376=1`. Do not write settings as part of detection.
[Setter/getter](https://github.com/grblHAL/core/blob/d67031a2b8adacd780581cf01c2d35f0cf945a2d/settings.c#L1269),
[setting number](https://github.com/grblHAL/core/blob/d67031a2b8adacd780581cf01c2d35f0cf945a2d/settings.h#L231).

Angular-axis proof does not also prove rotary-arc support or mixed G94 feed
semantics. In particular, the mock's firmware marker must not authorize rotary
arcs, which it rejects. Actual execution gating is outside this evidence helper.

## Primary Firmware Evidence

### Stock GRBL

Upstream gnea/grbl defines `N_AXIS` as 3 with X/Y/Z indices. Its parser has no
active A/B/C word cases, so A is an unsupported command. It supports G93 and G94
for its supported axes; G93 support does not imply rotary capability.
Sources: [axis definitions](https://github.com/gnea/grbl/blob/bfb67f0c7963fe3ce4aaf8a97f9009ea5a8db36e/grbl/nuts_bolts.h#L26),
[parser](https://github.com/gnea/grbl/blob/bfb67f0c7963fe3ce4aaf8a97f9009ea5a8db36e/grbl/gcode.c#L279).

Its `$I` reports version and build options; status coordinates are emitted over
the compiled axes. An ordinary stock identity or three-coordinate status must
not enable A. [Reporting source](https://github.com/gnea/grbl/blob/bfb67f0c7963fe3ce4aaf8a97f9009ea5a8db36e/grbl/report.c).

### grblHAL

The extended build report emits `[AXS:<count>:<axis letters>]` from the active
axis count and configured letters. Depending on compatibility configuration,
extended information may require `$I+`; absence from `$I` alone is inconclusive.
Its status/parameter coordinate formatter iterates active axes, and skips inch
conversion for axes configured as rotary. The extended report also exposes
`RF` when rotary feed correction is enabled.
[Reporting source](https://github.com/grblHAL/core/blob/d67031a2b8adacd780581cf01c2d35f0cf945a2d/report.c#L926).

The parser's G20 coordinate conversion excludes configured rotary axes. A is
therefore angular only with appropriate rotary configuration, not merely
because the build contains A. G93 requires a new F on each feed-motion block;
switching feed modes invalidates the prior feed. Inverse-time F is not converted
to millimeters. [Parser source](https://github.com/grblHAL/core/blob/d67031a2b8adacd780581cf01c2d35f0cf945a2d/gcode.c#L2171).

With rotary correction enabled, mixed G94 linear/angular moves derive time from
the linear distance and coordinate angular motion to that time. Without that
option this is not guaranteed. A separate option reverses inch feed conversion
for pure-angular motion. The planner still applies individual axis rate and
acceleration limits. [Planner source](https://github.com/grblHAL/core/blob/d67031a2b8adacd780581cf01c2d35f0cf945a2d/planner.c#L439),
[rotary settings implementation](https://github.com/grblHAL/core/blob/d67031a2b8adacd780581cf01c2d35f0cf945a2d/settings.c#L1155).

### FluidNC

FluidNC emits MPos/WPos using `Axes::_numberAxis`; its coordinate reporting
distinguishes linear units from angular degrees. The examined `$I` reporter does
not emit grblHAL's `[AXS:...]` declaration. A standard XYZA four-coordinate
status is therefore the conservative fallback, not the FluidNC brand string.
[Reporting source](https://github.com/bdring/FluidNC/blob/e769f1a7dd24d4edd2657e1927431b57dc410617/FluidNC/src/Report.cpp#L63).

The version payload contains the `FluidNC` token. The axis-type predicate
classifies A/B/C as non-linear; X/Y/Z and U/V/W are linear.
[Identity source](https://github.com/bdring/FluidNC/blob/e769f1a7dd24d4edd2657e1927431b57dc410617/FluidNC/src/Report.cpp#L343),
[axis-type source](https://github.com/bdring/FluidNC/blob/e769f1a7dd24d4edd2657e1927431b57dc410617/FluidNC/src/Types.h#L54).

G20 converts coordinates only where `is_linear(axis)` is true. G93 checks for a
per-block F and avoids ordinary inch-feed conversion. G94 follows different
conversion rules. [Parser source](https://github.com/bdring/FluidNC/blob/e769f1a7dd24d4edd2657e1927431b57dc410617/FluidNC/src/GCode.cpp#L975).

The examined planner normalizes the configured-axis delta vector and multiplies
inverse-time feed by that path magnitude. It does not contain grblHAL's explicit
mixed-rotary G94 correction branch. Consequently the mock's linear-distance G94
timing must not be advertised as generic FluidNC behavior; verify the actual
version, configuration, and kinematics. G93 provides a block-time request, not
an exemption from physical axis limits.
[Planner source](https://github.com/bdring/FluidNC/blob/e769f1a7dd24d4edd2657e1927431b57dc410617/FluidNC/src/Planner.cpp#L323).

## Mock Firmware Model

`MockTransport::default()` remains XYZ. It rejects real A words with `error:20`,
including program, check, work-offset, and jog commands, even when virtual motion
is disabled. Comments containing A do not enable or command an axis.

`MockTransport::rotary()` explicitly selects a synthetic four-axis firmware.
It emits `[FIRMWARE:MilloVirtual]`, `[AXS:4:XYZA]`, and four-coordinate status
before any motion. It does not
pretend to be a complete grblHAL or FluidNC emulator. All tests use the existing
in-memory serial `Transport` interface; no port is opened.

| Behavior | Fixture contract |
| --- | --- |
| Program motion | G0/G1, A-only or coordinated XYZ+A, including queued and compact blocks |
| Coordinates | G90 absolute, G91 incremental; A always degrees, no modulo-360 wrapping |
| G20/G21 | XYZ scales inches/mm; A does not scale |
| G93 | Positive F required on every feed-motion block; nominal duration `60/F` seconds |
| G94 mixed | Nominal duration `60 * XYZ_distance_mm / feed_mm_per_min`; A shares the progress parameter |
| G94 A-only | Nominal duration `60 * abs(delta_A_degrees) / F_degrees_per_min`, including G20 |
| Limits | XYZ rate/acceleration plus `$113` degrees/min and `$123` degrees/s^2 constrain the whole move |
| Work offsets | G54-G59 selection; `G10 L20 P1..6 Avalue`; G53 absolute machine-coordinate A |
| Status | MPos/WPos fourth values are machine/work A, including nonzero offsets |
| Queries | `$G` returns `[GC:...]`; `$#` reports four-coordinate G54-G59, G92, PRB |
| Hold/resume | `!` decelerates all moving axes to Hold:0; `~` continues the retained path and queue |
| Reset | Ctrl-X clears queued motion and pending responses; preserves stopped XYZ/A and WCS offsets; resets modal state and selects G54 |
| Jog | Opt-in A jog uses timed motion; 0x85 cancels jog, not an ordinary program |
| Check | Validates axis/feed words and updates modal state without moving |

The mock accepts `$GC` as a convenience alias; the standard GRBL query is `$G`,
whose **response tag** is GC. Neither query reports a live position: use status
for that, and `$#` for coordinate parameters. Status/parameters are metric XYZ
plus angular A; `$13` report-unit emulation is not implemented. `$G` retains the
canonical linear feed (mm/min in G94), or inverse minutes in G93.

Timing uses the existing acceleration integrator, so acceleration, rate limits,
and Hold can extend the nominal block time. Blocks changing A use exact stops;
the fixture does not claim production look-ahead, step quantization, collision
checking, rotary surface-speed compensation, cable limits, or physical position
trust after reset. Rotary arcs, G92 A changes, and other unimplemented rotary
G-code are rejected, not silently executed as XYZ. Existing XYZ-only simulation
retains its prior scope and is not a complete G-code validator.

## Focused Verification

Run `cargo test -p millo-mock -p millo-grbl -p millo-virtual-controller`.

- `crates/millo-mock/tests/rotary_firmware.rs`: serial identity, default rejection,
  real queued XYZ+A execution, inch/degree separation, coordinated timing,
  feed validation, axis limits, offsets, Hold/resume, Reset, check mode and jog.
- `crates/millo-grbl/tests/rotary_evidence.rs`: explicit/fallback evidence,
  failed identity, non-evidence branding/settings, contradictory layouts,
  unsupported vectors, and non-finite coordinates.
- `crates/millo-virtual-controller/tests/rotary_cli.rs`: launches the executable
  with `--rotary`, connects only to its freshly-created PTY, sends fragmented
  XYZ+A program input, and verifies timing, Hold/resume and Reset. The child is
  terminated and reaped at test end, including assertion failure.

## Standalone Virtual Serial Controller

On a Unix PTY host:

```sh
cargo run -p millo-virtual-controller -- --rotary
```

The executable prints its newly-created virtual serial path. Its discoverable
product is `Millo VMC-4 XYZA Controller`. This command never opens a physical
device. Omitting `--rotary` retains the existing XYZ firmware, product and
identity. Unknown options are rejected. `--help` opens no endpoint.

Library callers can use `VirtualController::start_rotary()`; existing
`VirtualController::start()` callers remain XYZ. The PTY receiver clears partial
line input on Ctrl-X, so a pre-reset A target cannot be combined with a new line.

## Typed Zero A

`WorkAxis::A` serializes as `"a"`. The Zero A command uses only
`G10 L20 P<n> A0`, without motion or implicit XYZ zeroing. The command arbiter
requires explicit operator confirmation, stable Idle, an enabled finite rotary
profile, fresh identity/settings inspection, current XYZA evidence, and a
recognized angular firmware contract. The reusable non-motion helper is
`validate_rotary_capability(profile, inspection, snapshot)` in the command crate.

Verification requires finite four-coordinate active-WCS and G92 parameters,
finite machine/work/offset A, agreement between `$#` and status, and work A
within 0.01 degree of zero. A missing or non-finite A is an error, never an
implicit zero. Reset/reconnect epoch changes during the operation invalidate
the verification.

The UI presents A only with finite reported machine/work/offset A, uses a
separate angle confirmation, and clears confirmations on reset/reconnect or
lost A evidence. The backend remains authoritative for profile and firmware
permission. The combined XYZ/XY action never includes A. Return A is not
available: both the interactor and typed encoder reject it until an explicit
rotary-clearance request contract exists. Positive Z alone is insufficient.

Focused tests: `cargo test -p millo-command rotary_zero_tests`,
`cargo test -p millo-command coordinates`, and the two WorkZero UI test files.

The tests establish host-side protocol/model behavior only. Native UI,
streaming policy, preview, dry-run, restart and recovery integration are owned
outside these crates and must preserve the detection and unit contracts above.
