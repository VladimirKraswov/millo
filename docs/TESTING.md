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
and assert the exact typed parse request. Toolpath read-model tests verify rapid
and cutting buffers, centering, framing, and grid placement independently from
WebGL.

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

- GRBL status, reset, alarm, error, and acknowledgement fixtures.
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
  modal state, WCS/TLO, and probe parameters.
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
  distance, linear moves, I/J arcs, and R arcs.
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
- Vitest checks the platform-neutral loader and pure toolpath read model. No
  test invokes a sender because none exists in this slice.
- Manual browser fixtures use `/?fixture=program` for desktop and
  `/tests/visual/program-mobile.html` for a 390 x 844 responsive viewport.
  Playwright screenshots verify layout and top/isometric switching. Canvas crops
  are checked for non-uniform luminance and color: the accepted desktop fixture
  measured Y `0..197`, and mobile measured Y `0..206`.

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

## Current realtime safety coverage

- Reset confirmation accepts only the active, unexpired actor challenge and
  consumes it before sending `Ctrl-X`.
- A mismatched, reused, missing, or expired challenge cannot reset the mock.
- Feed Hold writes exactly `!`; a running mock reports `Hold:0` on the next poll.
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
