# Testing and definition of done

Every vertical slice must update tests and documentation in the same commit.
The standard local gate is:

```bash
npm run verify
```

It runs TypeScript type checking, all Rust workspace tests, the production Vite
build, Rust formatting checks, and Clippy with warnings denied.

`npm run test:ui` runs Vitest policy tests for TypeScript feature modules. The
jog-pad suite verifies one signed fixed-step gateway call per press, rejects a
non-preset value before IPC, and rejects a concurrent press while the first call
is unresolved.

The work-zero feature suite rejects an unconfirmed request before the gateway
and delegates only a typed X/Y/Z request. Registry tests also verify that the
core panel occupies the separate `control.coordinates` slot.

Program-loader tests reject unsupported, empty, and oversized files before IPC
and assert both the exact typed parse request and retained original source.
Dry-run read-model tests prove that Mock availability and policy eligibility are
both required, expose only state-valid controls, and clamp untrusted display
progress. Toolpath read-model tests verify rapid and cutting buffers, centering,
framing, grid placement, and exact selected-line geometry independently from
WebGL. Program-line window tests prove that a large source mounts only a small
overscanned slice and that source-line lookup remains stable.

Extension-registry tests cover deterministic slot ordering, duplicate and
self-replacement rejection, add/replace/dispose behavior, one-revision owner
unload, and restoration of `core.jog-pad` after a plugin replacement unloads.

Plugin-host tests validate manifest/API versions, identifiers and capability
lists; deterministic grant ordering; required-versus-optional permission
behavior; unsupported and mismatched API rejection before activation; scoped
guarded-jog proxy exposure; owner namespace enforcement; activation rollback;
and complete UI cleanup on unload. A linked test plugin replaces
`core.jog-pad`, then proves that unloading restores the core contribution.

Machine-state host tests prove that mutable controller DTOs are cloned and
deeply frozen, publication stops after an idempotent unsubscribe, and a state
source does not imply a grant. Loader tests cover current/future read access,
subscriber-error isolation, cleanup after activation failure, automatic
unsubscribe on unload, and rejection through retained capability proxies after
their plugin is gone.

PluginHost bootstrap tests verify that core UI and the machine source share one
composition root, no plugin is activated implicitly, and an explicitly loaded
observer receives publications from that exact store. Stream-binding tests cover
initial state, live events, stale initial-response suppression, and cleanup when
the asynchronous Tauri listener resolves after disposal.

The test phase also runs `scripts/check-brand.mjs`, which keeps npm, Cargo,
Tauri, UI, and documentation naming consistent.

Machine-profile tests cover required names, bounded positive XYZ travel,
case-insensitive duplicate rejection, stable selection, JSON reload, local-only
fact updates, corrupt selection rejection, and conservative derivation from
GRBL settings. Settings tests cover the complete catalog, unknown firmware keys,
numeric formatting, immutable connection baselines, bounded reconnect history,
fresh-observation persistence, positive travel, and stale external changes.
Actor tests prove profile binding does not dismiss a reset banner and that one
setting edit executes the exact read/write/reread sequence. TypeScript tests keep
unverified hardware flags off and cover settings search/value comparison. The
`/?fixture=profiles` and `/?fixture=settings` screens are checked at desktop and
mobile sizes.

## Slice checklist

1. Capture new protocol or compatibility behavior as a fixture where possible.
2. Add focused unit tests for state transitions and failure paths.
3. Add adapter or UI tests when behavior exists outside the Rust core.
4. Run `npm run verify` from a clean working tree.
5. Update `README.md`, `docs/ARCHITECTURE.md`, and an ADR when a boundary or
   architectural decision changes.
6. Perform visual verification for changed operator screens.
7. Commit the complete slice atomically.

## Current lifecycle coverage

- GRBL status, reset, alarm, error, and acknowledgement fixtures, including
  `Bf`, `Ov`, `Pn`, `A`, and `Ln` telemetry.
- Reset banner ordering in the mock transport.
- Persistent mock alarm and explicit alarm clearing.
- Unresponsive transport simulation.
- Transient timeout counting and threshold transition to recovery.
- Reconnect plus status synchronization before returning to connected.
- Reset acknowledgement and non-alarm status behavior.

## Current native serial coverage

- Boxed runtime transport preserves the common transport contract.
- Empty port names and zero baud rates are rejected before OS I/O.
- Fragmented serial input is assembled into one CR/LF-trimmed line.
- End-of-stream and I/O before connect are reported as disconnection.
- Tauri serial IDs preserve native port names, including Unix device paths.
- USB device metadata maps to a stable UI descriptor.
- Likely-GRBL discovery accepts explicit controller metadata and common USB-UART
  vendors while rejecting Bluetooth and unidentified USB fixtures.
- macOS callout/TTY alias pairs collapse to `/dev/cu.*`; unpaired and non-macOS
port names remain untouched.

## Current command arbiter and inspector coverage

- One worker serializes polling, realtime bytes, and line queries.
- Actor-owned periodic polling publishes lifecycle snapshots.
- Realtime `?` consumes its status response; `!`, `~`, `0x85`, and `Ctrl-X` use
  their exact one-byte representation.
- `$I`, `$$`, `$G`, and `$#` execute in deterministic order and stop at their
  correlated terminal response.
- `error:n` and `ALARM:n` retain both active command and numeric code.
- Recorded Inspector fixtures parse firmware/build/options, numbered settings,
  structured `[OPT]` planner/RX capacities, modal state, WCS/TLO, and probe
  parameters.
- Mock Inspector responses cover the full Rust-to-UI readiness path.
- The Tauri command surface contains no raw-line or general movement endpoint;
  only typed guarded step-jog use cases are exposed.

## Current work-coordinate coverage

- Domain and GRBL tests map X/Y/Z plus active G54-G59 to exact
  `G10 L20 P1..P6 <axis>0` lines.
- Missing operator confirmation is rejected before any controller I/O.
- A non-idle fresh status prevents `G10` from being written.
- Actor tests assert the complete transaction for every axis:
  `?`, `$G`, one `G10 L20`, `$#`, then `?`.
- Mock GRBL updates only the active work offset, leaves machine position intact,
  and returns the changed G54-G59 parameter through `$#`.
- Success requires the final work coordinate to be within `0.002 mm` of zero.
- TypeScript tests cover the platform-neutral interactor, while the Tauri build
  verifies the typed command adapter. Automated tests never send work-zero to a
  physical controller.

## Current G-code and preview coverage

- Real `.nc`, `.ngc`, and `.tap` fixtures cover compact words, comments, common
  headers/modal cancels, metric and imperial units, absolute and incremental
  distance, linear moves, all three GRBL arc planes, IJK/R arcs, helices, and
  full circles.
- Timing fixtures cover modal units-per-minute feed, inverse-time feed on every
  cutting block, dwell accumulation, per-segment duration, and the explicit
  incomplete estimate produced by rapid motion.
- Parser failures cover conflicting G/M modal groups, wrong-plane arc offsets,
  mixed R/IJK definitions, context-only words, feedless cutting, unsupported
  G95, GRBL-incompatible absolute arc-center mode, and center-only arcs without
  the explicit XYZ target required by physical GRBL.
- Safety fixtures cover `M3`, spindle speed, `M6`, `G38.2`, and `G53`; they load
  for review but fail `dryRunEligible`, and unsafe movements are not invented as
  preview segments.
- Malformed fixtures retain line-addressable warnings for comments, tokens, and
  invalid arc definitions instead of panicking or silently drawing a chord.
- Missing/oversized source name, empty source, 2 MB input, 200,000-line, and
  500,000-preview-point limits are enforced in Rust. The UI mirrors the
  file-size and extension gate before reading a file.
- The Tauri adapter test proves parsing returns a typed program without an
  `AppState`, controller, transport, or command actor dependency.
- Vitest checks the platform-neutral loader, dry-run controls, and pure toolpath
  read model. No browser test can bypass the Rust execution policy.
- Selection tests isolate one linear segment, return an empty overlay for a
  non-motion line, and keep base geometry centered identically.
- Virtual-table tests cover mid-file, invalid, and bottom-edge scroll windows
  plus binary source-line lookup. The UI therefore remains bounded at the
  parser's 200,000-line limit.
- Manual browser fixtures use `/?fixture=program` for desktop and
  `/tests/visual/program-mobile.html` for a 390 x 844 responsive viewport.
  Browser screenshots verify layout, internal table scrolling, warning/line
  tabs, selected-row retention, motion highlighting, no-motion selection, and
  top/isometric switching. Canvas crops are checked for non-uniform luminance
  and color: the accepted desktop fixture measured Y `0..197`, and mobile
  measured Y `0..206` before this slice.

## Current dry-run sender coverage

- Policy tests independently reject spindle activation, non-zero spindle
  speed, coolant activation, probing, M6, machine/reference-coordinate motion,
  coordinate mutation, parser safety/errors, incomplete previews, and commands
  over 255 bytes.
- Approved plans contain normalized executable lines plus only an M5/M9 safety
  preamble. Plan and line fields are private and have no deserialize path.
- Sender tests prove bounded RX-window fill, exact command-plus-newline byte
  accounting, FIFO response correlation, pause/resume/cancel transitions,
  exact failed source-line retention, terminal completion only after every
  `ok`, and line/plan bounds.
- Plan timing tests sum feed motion and dwell to exact milliseconds, mark rapid
  duration as an incomplete lower bound, and attach estimates to source lines.
  Sender tests prove estimate advancement only on `ok`, Hold wall time exclusion,
  resumed elapsed growth, and a frozen terminal elapsed value. UI read-model
  tests distinguish `ETA` from `ETA >=` and format minute/hour durations.
- Mock transport can acknowledge ordinary program lines or inject a correlated
  `error:n`/`ALARM:n` without changing serial hardware.
- Actor integration tests assert multi-line prefill without exceeding the active
  RX window, exact rejected FIFO-line reporting, terminal state publication,
  and rejection when the execution target is not Mock.
- Tauri adapter tests reparse original source and prove an unsafe request cannot
  mint a plan. Runtime start also checks the active backend transport descriptor.
- No automated or manual test in this slice sends a program line to the physical
  controller.

## Current real-run and sender coverage

- Parser fixtures accept common standalone `%` file delimiters without turning
  them into executable sender lines.
- `millo-run` tests clear a bounded program while retaining operator cautions,
  reject spindle control for Air run at the exact source line, accept it for
  Cutting, keep a
  probe-only readiness failure from blocking non-probing motion, and independently
  detect empty geometry plus a non-idle controller. A separate test rejects a
  motion file that relies on ambient units, distance, or feed modes.
- Actor tests require the serial execution target, reject Mock before any I/O,
  and assert the exact read-only sequence `?`, `$I`, `$$`, `$G`, `$#`, `?`.
- An unsafe-program actor test proves every emitted byte belongs to that
  read-only allowlist and no normalized program line is dispatched.
- TypeScript tests keep preflight and authorization controls separate. Air and
  Cutting require different physical facts; blocked, missing-gateway, and busy
  states fail closed.
- The `/?fixture=preflight` browser fixture covers Blocked status, the dedicated
  Preflight diagnostics tab, internal scrolling, desktop/mobile layout, and the
  source-line jump from a policy blocker to selected `L8`.
- `millo-run` tests cover incomplete confirmations, blocked/stale evidence,
  SHA-256 program binding, 30-second expiry, single-use consumption, and
  position/session invalidation.
- Actor fixtures model a Serial execution target with deterministic Mock GRBL.
  A successful authorization repeats exactly `?`, `$I`, `$$`, `$G`, `$#`, `?`,
  leaves the sender Idle, and emits no program line. Incomplete confirmation
  fails before controller I/O.
- `/?fixture=first-cut` provides intent selection, clear preflight, the
  mode-specific checklist, lease issuance, and Start for visual regression.
- Serial-class actor fixtures consume the lease atomically and exercise the
  production sender over deterministic Mock GRBL. They cover all-`ok` plus
  fresh-Idle completion, correlated `error`, `ALARM`, Hold/resume, reset banner,
  status failure, and transport disconnect. Sender tests additionally cover
  `M0/M1` program barriers and `M2/M30` plan termination. Physical terminal
  commands are withheld until fresh `Idle`; dedicated fixtures cover
  Hold/Resume, Reset cancellation, and terminal-command timeout while deferred.
  Physical command failures also verify automatic Hold plus Soft Reset and that
  reset flushes every queued Mock GRBL response.
- Delayed-response fixtures prove Feed Hold is serviced within one 10 ms read
  slice instead of waiting for the command timeout. A separate fixture injects
  realtime status ahead of a delayed `ok`, verifies live `Bf/Ov` telemetry, and
  keeps FIFO acknowledgement counts unchanged.
- Typed override tests cover every GRBL byte family, Mock `Ov:` mutation, and
  feed/rapid/spindle requests while an acknowledgement is delayed. An explicit
  status refresh during that delay cannot consume the sender's response or
  increment its acknowledgement count.
- A non-default Mock `[OPT:V,15,256]` fixture proves the inspected capacity is
  carried by the one-use lease and becomes a 255-byte sender window at Start.
- Reusing a consumed authorization fails after only the fresh status read. No
  second program line can be started from the same lease.
- Tauri and React expose production Start only for a profile-bound serial target
  and matching one-use authorization. Plugins still have no run capability.
- Automated tests never send a program line to physical hardware. The first
  manually confirmed attempt on 2026-08-11 sent the 20 mm fixture, detected the
  original premature-`M30` timeout at 9/10 acknowledged lines, and performed the
  emergency Reset path. It is recorded as an interrupted attempt, not a pass.
  A separately confirmed retry after the terminal-barrier fix completed all
  10/10 lines and returned to fresh `Idle` at WPos XYZ zero.

## Current GRBL Check-run coverage

- The typed controller transition permits only verified `Idle -> Check` and
  `Check -> Idle`; an active Run state is rejected before `$C`.
- Mock GRBL models `$C`, reports `Check`, rejects the transition from Run, and
  returns to `Idle` after the second toggle.
- Check sender mode keeps one line in flight, preserves exact source-line errors,
  acknowledges `M30` without physical draining, and never emits Hold or Reset.
- Check plans use Cutting grammar: M3/M4/S are accepted for firmware validation,
  while the same source remains forbidden by Air policy. M6, coolant, probing,
  coordinate mutation, and reference/machine movement remain blocked.
- Actor tests reject non-serial targets before I/O, run the complete multi-plane
  fixture, verify every approved line exactly once, cover correlated error, and
  require automatic cleanup to `Idle`.
- The physical command below first rejected a center-only full circle with
  `error:26` at source line 10. The parser now rejects that form locally and the
  fixture names the unchanged endpoint explicitly. A repeat on 2026-08-11
  accepted all 25 sender lines and returned the controller to `Idle`:

```bash
cargo run -p millo-desktop --example hardware_check_run -- \
  /dev/cu.usbmodem11101 fixtures/programs/grbl-complex-check.nc
```

- The same physical fixture was repeated after incremental response polling was
  introduced. All 25 lines were again correlated and GRBL returned to `Idle`,
  validating the real serial demultiplexer rather than only Mock ordering.
- `grbl-cutting-check.nc` adds metadata-only O headers, N words, M3/M4/S, all
  three arc planes, distance/feed mode transitions, dwell, M0/M1, and an
  explicit-endpoint full circle. Check validates M0/M1 without entering the
  physical-run operator pause state. The physical controller accepted 26/26 lines on
  2026-08-12 and returned to `Idle`:

```bash
cargo run -p millo-desktop --example hardware_check_run -- \
  /dev/cu.usbmodem11101 fixtures/programs/grbl-cutting-check.nc
```

- The React check-run read model requires a loaded program, typed gateway, and
  serial target and refuses to replace an active sender. Program workspace
  exposes the action only through `start_check_run`; Rust reparses the source.
- Browser inspection at 1440 x 900 and 390 x 844 verifies the additional action
  in the preflight panel. The mobile page remains exactly 390 px wide; the
  340 px Check button stays inside its 25..365 px content bounds.

### Hardware Air-run fixture

Read-only preflight:

```bash
cargo run -p millo-desktop --example hardware_air_run -- \
  /dev/cu.usbmodem11101 fixtures/programs/air-square-20mm.nc --inspect-only
```

The command parses and validates the exact `20 x 20 x 0 mm` fixture before it
opens serial, then performs only status and Inspector reads. Confirmed execution
requires all seven flags; omission of any one fails before serial connection:

```bash
cargo run -p millo-desktop --example hardware_air_run -- \
  /dev/cu.usbmodem11101 fixtures/programs/air-square-20mm.nc \
  --confirm-unlock --confirm-tool-removed --confirm-spindle-off \
  --confirm-set-current-xyz-zero \
  --confirm-safe-z --confirm-path-clear --confirm-power-control
```

The harness uses the typed `G10 L20` workflow to set the confirmed current
position as XYZ work zero, rereads `$#`, and requires observed WPos within
`+/-0.02 mm`. It monitors the sender for two minutes, verifies final `Idle` and
return to work zero, and handles `Ctrl-C` or timeout by requesting Feed Hold
followed by challenge-confirmed Soft Reset.

The successful hardware run on 2026-08-11 first verified typed `$X` from
`Alarm` to fresh `Idle`, set and reread the new G54 XYZ0, acknowledged all 10
sender lines, and returned to WPos XYZ `0.000 mm`. The earlier interrupted
attempt remains recorded because it exposed the now-fixed and regression-tested
`M30` planner barrier. Every future invocation still requires all seven
confirmations and a
new one-use authorization.

## Current hardware readiness coverage

- A representative unhomed XYZ fixture passes the guarded test-jog
  configuration while retaining cautions for G91, manual spindle, missing
  homing/limits, untested probe input, and missing physical emergency stop.
- Missing axis tuning blocks readiness.
- Enabled homing or hard limits block a profile that declares no sensors.
- Laser mode blocks the milling profile.
- Alarm or non-idle controller state blocks readiness even when static settings
  are valid.
- Mock GRBL exposes all required XYZ values and exercises the ready report across
  the command actor and typed Tauri response.
- The Tauri mock smoke test confirms a ready report is invalidated after an
  injected alarm rather than leaving stale green readiness on screen.

## Current machine-profile and settings coverage

- An empty store permits read-only serial connection and inspection, but blocks
  movement until controller-derived onboarding creates a validated profile.
- A known exact fingerprint is selected automatically. One legacy exact-port
  profile can be migrated; duplicate strong or legacy matches fail closed.
- The selected profile loads into the command actor at startup; manual selection
  remains available only while disconnected. Connected onboarding uses a
  separate actor binding operation and cannot replace an already bound profile.
- Schema version 1 uses stable IDs, a bounded 64-profile list, and temporary-file
  replacement persistence.
- Detection uses only `?`, `$I`, `$$`, `$G`, and `$#`, and rejects invalid
  `$130/$131/$132` before a draft reaches React.
- The physical import helper exposes no movement, setting-write, spindle, or
  coolant operation. It stored the first real profile on 2026-08-11.
- `millo-settings` stores one JSON file per profile. Multiple verified edits keep
  the first observed connection value as rollback baseline; reconnect promotes
  the observed machine state to a new baseline and archives the old one.
- Tauri requires both the source revision and source value. The actor compares
  the source value with a new `$$`, sends only one validated setting command,
  and verifies the result through another complete Inspector read.

## Current realtime safety coverage

- Reset confirmation accepts only the active, unexpired actor challenge and
  consumes it before sending `Ctrl-X`.
- A mismatched, reused, missing, or expired challenge cannot reset the mock.
- Feed Hold writes exactly `!`; a running mock reports `Hold:0` on the next poll.
- Feed override maps reset/`+10`/`-10`/`+1`/`-1` to
  `0x90..0x94`; rapid `100/50/25` maps to `0x95..0x97`; spindle
  reset/`+10`/`-10`/`+1`/`-1` maps to `0x99..0x9d`. Mock status reports the
  bounded result through `Ov:` and reset restores `100/100/100`.
- Soft Reset makes the mock emit a GRBL reset banner and return to `Idle`.
- Incomplete operator confirmation performs no controller I/O.
- Every preflight executes a new status plus `$I/$$/$G/$#` sequence and receives
  a distinct short-lived authorization.
- Readiness blockers and alarm state return the fresh inspection report without
  authorization.
- Test-jog authorization is single-use and is invalidated by expiry, alarm,
  reset-count change, reconnect-count change, disconnect, or non-idle state.
- The GRBL encoder accepts only X/Y/Z, one signed `0.01..1.00 mm` axis word, and
  `10..100 mm/min`; it always injects `G91 G21` and never trusts UI formatting.
- Actor tests prove a lease produces at most one `$J=` write and remains consumed
  after validation failure. Missing, stale, or reused leases produce no motion.
- Mock step jog changes exactly one coordinate, reports `Jog`, completes to
  `Idle`, models bounded motion duration from distance/feed, and responds to
  realtime Jog Cancel without Reset.
- Tauri mock smoke covers Run to Hold, two-stage Reset, reset-banner
  acknowledgement, preflight, one step-jog write, lease relock, and an exact
  single-axis position update. Rust actor/mock tests cover Jog Cancel gating.
- Jog-pad actor tests prove that every press begins with a fresh status and full
  Inspector, always uses `10 mm/min`, accepts only `0.01` and `0.10 mm`, and does
  not issue another `$J` while the refreshed controller state is `Jog`.

CI does not require a physical controller. For a hardware smoke test, launch
`npm run tauri dev`, refresh the device list, connect at the controller's baud
rate, verify that machine coordinates update, unplug the device, and confirm the
state moves through `Recovering`. Reconnect the device and confirm polling
returns to `Connected`.

The no-motion override smoke uses the real actor and serial parser, observes
`Ov:110,50,99`, and restores all channels to `100/100/100` before disconnecting:

```bash
cargo run -p millo-desktop --example hardware_overrides -- \
  /dev/cu.usbmodem11101
```

Run it only while the controller is `Idle`. Cleanup is attempted even when an
intermediate assertion fails; a cleanup failure is reported explicitly instead
of silently leaving altered overrides.

The guarded first-motion smoke example uses the same serial transport, command
actor, readiness policy, and authorization path as Tauri. It requires an exact
axis plus an exact motion confirmation flag. One process performs only one
`+0.10 mm` step at `10 mm/min`:

```bash
cargo run -p millo-desktop --example hardware_step_jog -- \
  /dev/cu.usbmodem11101 Y \
  --confirm-disable-limits-and-homing --confirm-motion
```

It requires both flags because `$21=0` and `$22=0` are persistent controller
changes and the subsequent jog is a separate physical action. The actor reads
settings before and after, writes only non-zero values, and fails unless both are
verified as zero. It then requires a fresh `Idle` status and jog authorization.
After acceptance it waits at most five seconds for `Idle`, sends Jog Cancel if a
jog remains active, verifies that only the selected axis changed, and disconnects.
Never run it unattended; the machine has no verified travel envelope or physical
emergency stop. Normal automated verification compiles this example but never
executes it.
Run different axes as separate processes so every step reconnects, repeats the
Inspector/readiness checks, and consumes a fresh authorization. For example,
after a successful Y run has returned to `Idle`, replace `Y` with `Z` for the
next run. The example rejects the result unless only the selected axis changes.

The confirmed 2026-08-11 run on `/dev/cu.usbmodem11101` changed `$21` and `$22`
from `1` to `0`, verified both through a second Inspector read, accepted
`$J=G91 G21 X0.100 F10.000`, returned to `Idle`, and measured deltas X `+0.100`,
Y `+0.000`, Z `+0.000 mm`.

Two subsequent, separately launched runs re-inspected `$21=0` and `$22=0`
without additional writes. The Y run accepted `$J=G91 G21 Y0.100 F10.000`,
returned to `Idle`, and measured X `+0.000`, Y `+0.100`, Z `+0.000 mm`. Only
after that succeeded, the Z run accepted `$J=G91 G21 Z0.100 F10.000`, returned
to `Idle`, and measured X `+0.000`, Y `+0.000`, Z `+0.100 mm`.
