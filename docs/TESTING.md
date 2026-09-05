# Testing and definition of done

## Product gate

```bash
npm ci
npx playwright install chromium
npm run verify:product
```

`verify:product` runs the existing `verify` gate and then the executable
Playwright suite in `tests/workflow`. Desktop (1440x960), compact native-size
(860x600), and narrow responsive (390x844) projects exercise preparation,
confirmation, pause/resume/stop, completed Check, rerun, M6 and searchable help.
Scene tests decode canvas screenshots and require nonblank color variation,
changed pixels after changing view, and no document horizontal overflow.
WebGL fault injection must leave the navigation and realtime controls mounted.

These tests use development gateways, not physical serial. They do not prove
native WebView behavior or mechanical braking time. Narrow viewport tests do
not imply a supported mobile distribution. Existing Rust actor/PTY fixtures
remain the protocol boundary. `vitest.config.ts` keeps Playwright specs out of
Vitest, avoiding duplicated execution and false test discovery.

CI is defined in `.github/workflows/verify.yml` for macOS and Linux. It runs
with no CNC hardware or signing credentials, stores failure traces/screenshots,
and does not retry failed tests to hide instability. Inspect an actual CI run
before claiming cross-platform verification. `playwright-report/` and
`test-results/` are local generated artifacts, not source files.

Browser projects run with one worker to avoid competing software WebGL contexts
and cold transforms on development machines. Vite ignores Rust build outputs
and browser reports, preventing unrelated file-watcher traffic during tests.

The longer sections below document the accumulated domain coverage, including
historical manually exercised fixtures. Only tests present in the current
source and executed in the current run count as automated release evidence.

Every vertical slice must update tests and documentation in the same commit.
The standard local gate is:

```bash
npm run verify
```

`npm run test:architecture` enforces frontend dependency direction: Tauri stays
in API/gateway adapters, and production plugin code reaches host services only
through `src/plugin-sdk`. `npm run test:dependencies` is the explicit npm
supply-chain gate; `cargo audit` is run for release review because RustSec
reports target-specific and maintenance warnings requiring human triage.
Production build runs `test:bundle`: the initial application chunk is limited to
500 KiB and every lazy chunk to 600 KiB. Toolpath and Heightmap Three.js scenes
must stay lazy; importing either scene eagerly breaks this budget instead of
quietly slowing every startup.

It runs TypeScript type checking, all Rust workspace tests, the production Vite
build, Rust formatting checks, and Clippy with warnings denied.

The standalone serial simulator has an additional end-to-end boundary:

```bash
cargo test -p millo-virtual-controller
```

The fixture creates a raw PTY, discovers it through `millo-serial`, opens it as
an ordinary `SerialTransport`, verifies the VMC-3 `$I` identity, sends `$J`, and
asserts a non-instant accelerating position followed by the exact endpoint and
fresh `Idle`. Firmware unit tests separately cover braking, Hold/Resume, Jog
Cancel, arcs, inverse-time feed, overrides, and collinear junction continuity.

The maintainability contract is tested at the same boundaries as behavior:

- controller readiness is a table-tested shared predicate in both TS and Rust;
- sender-state classification enumerates every state instead of repeating
  Boolean expressions in individual components;
- extracted connection, Inspector, readout, preview, diagnostics, and run-card
  components have focused render tests;
- presentation tests pass gateway availability explicitly and assert that
  host-owned controls disappear when the capability is absent;
- test builders may remove repeated setup, but separate lifecycle, race, error,
  and hardware-policy cases must not be merged merely to reduce line count.

`npm run test:ui` runs Vitest policy tests for TypeScript feature modules. The
jog-pad suite verifies one signed bounded gateway call per press, rejects values
outside the technical envelope before IPC, scales presets to the machine, and
rejects a concurrent press while the first call is unresolved. Continuous-jog
tests cover release after acceptance, release-before-start, repeated idempotent
stop, actor watchdog cancellation, and machine-coordinate versus profile
boundaries. Homing tests require fresh Idle after `$H`, invalidate reference on
Ctrl-X and automatic transport recovery, and keep Hold/Reset available while
the lifecycle owns the port. Optional A tests prove that degree limits are not
clamped by linear `maxJogDistanceMm`. Output tests reject undeclared M7/M8 before
transport I/O and verify accepted spindle/coolant state through `$G`.

Operator-layout tests keep transient controller state from moving primary
controls. Safety controls render fixed Hold, Reset, and Cancel slots in both
Idle and Jog. Sender tests map every state to one primary slot and one cancel
slot. Browser fixtures compare element bounding boxes across Idle, Jog, Alarm,
and reset; the accepted delta for the controller heading, coordinate readout,
workbench tabs, safety actions, jog pad, and coordinate disclosure is exactly
zero pixels.

`/?fixture=machine-control` renders a connected, homed profile with optional A,
controller spindle, and declared flood/mist outputs. Use it for desktop and
narrow-viewport screenshots of the complete Motion Deck and WCS/output
disclosure without requiring CNC hardware.

`/?fixture=console` opens the safe operator console against deterministic
status, firmware, settings, modal, and coordinate responses. Browser checks
require a stable modal rectangle, independent transcript scrolling, no page or
dialog horizontal overflow, a successful `$I`, a visible blocked `G0 X1`, and
no React console errors. Component/model tests separately render expert mode and
preserve case for one bounded line. Rust tests prove safe rejections write zero
bytes, expert lines are actor-serialized, and queries fail while Run is active.
Preference-store tests cover safe-by-default persistence and backup recovery.

Program file-picker tests require a visible `Open G-code` primary action in the
empty workspace, the complete supported extension allowlist, and a stable
`Replace file` toolbar action after loading.

The work-zero feature suite rejects an unconfirmed request before the gateway
and delegates only a typed X/Y/Z request. Registry tests also verify that the
core panel occupies the separate `control.coordinates` slot.
Heightmap datum tests accept Z0 only when a map contains verified contact and
its stored G54-G59/WCO binding matches the live controller. Stale bindings,
changed WCS/WCO and maps without contact fail closed. The operator fixture must
show `Z0 найден` for a current saved map and must not request redundant Z
zeroing after the surface dialog is reopened.
The command-actor regression `heightmap_reuses_probe_established_z_zero_after_xy_zeroing`
executes the real operator sequence and asserts that the entire probe plus map
flow writes exactly one `G10 ... Z` command.

Depth-correction tests keep the option disabled at a zero offset by default,
preserve rapid and safe-Z moves, add the signed delta exactly to every negative
non-rapid Z point, enforce the +/-10 mm bound, update both preview bounds, bind
the exact micrometre delta into Check and run evidence, and prove that heightmap
compensation is applied after the nominal depth offset.

Program-loader tests reject unsupported, empty, and oversized files before IPC
and assert both the exact typed parse request and retained original source.
Program-editor tests cover bounded undo/redo history, whole-row insert/delete
and clipboard spans, caret/source-line mapping, G/M/axis/arc/feed/comment syntax
tokens, deterministic processed export, and the complete editor command
surface. Type checking keeps Apply bound to a parsed `LoadedProgram`; Rust tests
keep native export names leaf-only across every supported G-code extension.
Sender read-model tests cover typed failures, acknowledgement heartbeat and
bounded time presentation. Toolpath read-model tests verify rapid and cutting buffers, centering,
framing, grid placement, exact selected-line geometry, live tool-coordinate
mapping, grid projection, and inside/outside job classification independently
from WebGL. Program-line window tests prove that a large source mounts only a small
overscanned slice and that source-line lookup remains stable.

Live-tool tests bind generated-job `T` numbers to exact library IDs, switch to
the requested tool at an `M6` barrier, hide the cutter during Motion check, and
allow rotation only in active physical sender states. Geometry tests cover all
six tool kinds, preserve V-bit included-angle/tip geometry, and keep the neck of
a surfacing cutter connected to its shank. The `/?fixture=tool-motion` browser
route renders a selected 20-degree engraver at reported XYZ. Desktop and narrow
viewport checks require a nonblank canvas, no HUD overflow, and changed frame
checksums while the tool is rotating; the fixture cannot dispatch machine I/O.

Extension-registry tests cover deterministic slot ordering, duplicate and
self-replacement rejection, add/replace/dispose behavior, one-revision owner
unload, and restoration of `core.jog-pad` after a plugin replacement unloads.

Plugin-host tests validate manifest/API versions, identifiers and capability
lists; deterministic grant ordering; required-versus-optional permission
behavior; unsupported and mismatched API rejection before activation; scoped
guarded-jog proxy exposure; owner namespace enforcement; activation rollback;
and complete UI cleanup on unload. A linked test plugin replaces
`core.jog-pad`, then proves that unloading restores the core contribution.

Image-job tests cover both sides of the plugin boundary. `millo-cam` fixtures
verify transformed/cubic SVG paths, PNG-to-SVG tracing through VTracer, physical
scaling, curve flattening, blank-image rejection, bounded input/geometry, and a
spindle-free result that reparses as dry-run-eligible G-code. Vitest verifies
that `JobCreationService` freezes core results and rejects fabricated jobs,
`jobs.create` proxies close on unload, and the bundled plugin registers only in
`workspace.tools` after both required grants. The browser fixture
`/?fixture=image-job` opens the modal for desktop/mobile layout checks; upload a
local PNG to inspect its progressive vectorization controls without dispatching
machine I/O.

Tooling tests cover preset validation, editable factory records, persistent
custom CRUD, duplicate rejection, atomic reload, and restoration of deleted
presets without overwriting edits. Surfacing CAM fixtures reject incompatible
tools and oversized edge overrun, exercise multi-depth X/Y rasters and line
limits, require spindle-free parser-clean output, and verify complete final
cross-axis coverage. Vitest covers the frozen `tools.read` capability, bundled
plugin registration, and cleanup on unload. Every tool-consuming plugin uses the
same stable React subscription adapter; browser checks for `/?fixture=pcb` and
`/?fixture=surfacing` must remain free of React console errors. Use
`/?fixture=tools` and `/?fixture=surfacing` for visual checks; neither fixture
dispatches machine I/O.

PCB fixtures cover Gerber dark/clear geometry, standard and macro apertures,
step-and-repeat, outline paths, Excellon modal tool groups and `G85` slots,
Gerber X2 drill flashes/routes, ignored production layers, transform
normalization, isolation offsets, multi-tool `M6`, tabbed multi-depth outline
cutting, spindle-free output and final `millo-gcode` parse. Vitest checks X2
and legacy role inference, slot-safe tool selection, immutable inspection and
core-issued job identity. Use `/?fixture=pcb` for the modal layout fixture.
Dedicated recommendation tests verify the V-bit depth/diameter formula,
selection of the 20-degree 0.1 mm tip for fine gaps, rejection of an unknown
kit-tool angle, and the warning produced for a 90-degree cutter that cannot fit.
Rust tests cover bounded copper-gap analysis, explicit tip-geometry migration
and effective-diameter use in emitted isolation offsets and G-code metadata.
That browser-only route uses an isolated preview gateway, so file rows, drill
groups, slot rendering and generated-state layout can be inspected without a
Tauri runtime. Production builds always use the native Rust gateway.

Program diagnostics tests additionally prove that a host-managed `M6` is shown
as a localized expected tool-change event, does not mark a successfully parsed
file as defective, and still leaves malformed or incomplete previews blocked.
The execution policy remains authoritative: Air run rejects `M6`, while Cutting
accepts only the isolated barrier form.

Machine-state host tests prove that mutable controller DTOs are cloned and
deeply frozen, publication stops after an idempotent unsubscribe, and a state
source does not imply a grant. Loader tests cover current/future read access,
subscriber-error isolation, cleanup after activation failure, automatic
unsubscribe on unload, and rejection through retained capability proxies after
their plugin is gone. A delayed-activation fixture unloads the plugin before its
promise resolves and verifies immediate UI cleanup, late deactivation, and an
empty active registry.

PluginHost bootstrap tests verify that core UI and the machine source share one
composition root, no plugin is activated implicitly, and an explicitly loaded
observer receives publications from that exact store. Stream-binding tests cover
initial state, live events, stale initial-response suppression, and cleanup when
the asynchronous Tauri listener resolves after disposal.

External-plugin tests compile the bundled package in the same bounded Rhai
engine used for imported code, reject dynamic `eval`, terminate an infinite
loop at the operation budget, validate command inputs before evaluation, reparse
generated boundary-check G-code, bind grants to a package digest, and clear old
trust after an update. UI tests mount workspace buttons and one grouped machine
panel from declarations, deny commands without their capability, and prove that
disposing registrations removes all contributed UI.
They also prove transactional disk failure leaves in-memory state unchanged,
backup recovery repairs a corrupt primary, action capability cannot exceed the
command declaration, field metadata/text and generated file names are bounded,
and one execution fence prevents grant/digest mutation during an invocation.
Trusted SDK tests cover definition-time manifests, work-coordinate proxies, and
the architecture import boundary. Bootstrap tests also require idempotent host
shutdown to deactivate every loaded plugin exactly once.
Use `/?fixture=plugins` for deterministic desktop/mobile manager screenshots;
the fixture has no Tauri gateway and cannot execute or persist a command.

The test phase also runs `scripts/check-brand.mjs`, which keeps npm, Cargo,
Tauri, UI, and documentation naming consistent.
`scripts/check-security.mjs` requires explicit production/development CSP maps,
the complete critical directive set, production IPC-only `connect-src`, and no
wildcard or `unsafe-eval` source.

Machine-profile tests cover required names, bounded positive XYZ travel,
case-insensitive duplicate rejection, stable selection, JSON reload, local-only
fact updates, corrupt selection rejection, and conservative derivation from
GRBL settings. Settings tests cover the complete catalog, unknown firmware keys,
numeric formatting, immutable connection baselines, bounded reconnect history,
fresh-observation persistence, positive travel, and stale external changes.
Actor tests prove profile binding does not dismiss a reset banner and that one
setting edit executes the exact read/write/reread sequence. TypeScript tests keep
unverified hardware flags off and cover settings search/value comparison plus
write fencing by controller fingerprint, profile, dialog lifecycle, and open
state. The
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

## Job-centered operator workflow

- `jobReadinessModel` tests the complete primary-action priority, including
  disconnected, Alarm, pending/outstanding recovery, missing work zero,
  required GRBL Check, and ready Start.
- `workPositionModel` verifies direct WPos and conservative MPos derivation from
  active G5x, G92, and TLO evidence.
- React server-render tests preserve four readiness facts, exactly one primary
  action, and prominent XYZ plus individual-axis zero controls.
- Visual regression is performed with `/?fixture=first-cut`: inspect unchecked
  readiness, run preflight, confirm that Start replaces the previous action,
  open Work zero, and open the final confirmation. The fixture must show a bound
  connected machine so its states do not contradict the job surface.
- `/?fixture=check-complete` starts from a recurring terminal Check snapshot and
  must render ready validation plus the intent-specific `Начать гравировку`
  action, never the old Completed card.
- `/?fixture=check-running` renders a live Check with one stable
  `Отменить проверку` action. Cancelling must expose
  `Вернуться к подготовке`, and that action must restore the ordinary readiness
  panel without a recovery prompt.
- The same fixture exercises `Start -> Pause -> Finish job -> Prepare new run`.
  The physical sender action model keeps two stable action slots, and the actor
  regression proves typed stop writes `!` then `Ctrl-X`, becomes `Cancelled`,
  and refuses a second stop.
- The typed Unlock boundary remains covered by the actor test
  `alarm_unlock_requires_confirmation_and_verifies_idle_in_the_actor`; the Tauri
  adapter only forwards explicit UI intent and records the result in audit.

## Current lifecycle coverage

- GRBL status, reset, alarm, error, and acknowledgement fixtures, including
  `Bf`, `Ov`, `Pn`, `A`, and `Ln` telemetry.
- Reset banner ordering in the mock transport.
- Persistent mock alarm and explicit alarm clearing.
- Unresponsive transport simulation.
- Transient timeout counting and threshold transition to recovery.
- Reconnect plus status synchronization before returning to connected.
- Reset acknowledgement and non-alarm status behavior.
- Sparse GRBL status reconciliation retains periodic WCO/override evidence,
  derives a continuously updated WPos from each MPos, accepts a fresh offset
  derived from MPos+WPos, and clears the cache across reset.

## Current native serial coverage

- Boxed runtime transport preserves the common transport contract.
- Empty port names and zero baud rates are rejected before OS I/O.
- Fragmented serial input is assembled into one CR/LF-trimmed line.
- End-of-stream, an incomplete frame at EOF, and I/O before connect are reported
  as disconnection. A line beyond the 4 KiB native framing bound returns typed
  `LineTooLong` before allocation can grow with an untrusted USB stream.
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
- The Tauri command surface contains no raw transport or arbitrary-byte
  endpoint. Its console command accepts a policy-validated line only through the
  actor; safe mode defaults to the diagnostic allowlist and expert mode remains
  Idle/Alarm-only, operation-fenced, audited, and evidence-invalidating.

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
- Absolute return fixtures encode one-axis `$J=G90 ... <axis>0`, preserve the
  active G54-G59 offset, and verify that no `G10` is written.
- Actor tests reject X/Y return without positive work-Z clearance and bound
  feed/distance against live controller settings and the machine profile.
- TypeScript tests cover the platform-neutral interactor, while the Tauri build
  verifies the typed command adapter. Automated tests never send work-zero to a
  physical controller.
- The completed-run UI fixture requires both `Вернуть фрезу к Z0` and
  `Подготовить повторный запуск`, plus the visible repeat-pass sequence.

## Current G-code and preview coverage

- `fixtures/programs/millo-solar-guilloche.nc` is a 60 x 60 mm surface-
  engraving sample with four dense parametric contours. Its regression requires
  more than 1,000 preview motions, zero parser warnings, a complete preview,
  `Z -0.20..3.00 mm`, bounded XY geometry, and no spindle or probe command. A
  copy may be opened from the desktop for operator UI checks, but automated
  tests never dispatch it to hardware.
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
  speed, coolant activation, probing, Air-run M6, machine/reference-coordinate motion,
  coordinate mutation, parser safety/errors, incomplete previews, and commands
  over 255 bytes.
- Approved plans contain normalized executable lines plus an M5/M9 safety
  preamble and a distinct M5/M9 safety epilogue. Plan and line fields are
  private and have no deserialize path.
- Sender tests prove bounded RX-window fill, exact command-plus-newline byte
  accounting, FIFO response correlation, pause/resume/cancel transitions,
  exact failed source-line retention, terminal completion only after every
  `ok`, and line/plan bounds.
- A 100,000-line sender regression finishes in bounded FIFO state, asserts every
  snapshot remains within RX capacity, and prevents accidental O(n) heartbeat
  work. The test exposed and fixed an initial quadratic shutdown-counter scan.
- Sender snapshots carry a monotonic `runSequence` for journal correlation in
  addition to the acknowledgement sequence used by the live heartbeat.
- Wire-protocol tests prove source lines replace arbitrary file `N` words,
  wire prefixes participate in command/RX limits, parser modal checkpoints are
  native-only, and only in-range GRBL `Ln:` values become executing-line
  evidence. Journal tests keep that value distinct from accepted-line counts.
- Journal tests cover checkpoint throttling, mandatory terminal persistence,
  bounded history, backup recovery, and explicit failure when both primary and
  backup are corrupt. A Tauri adapter test proves the dedicated worker consumes
  snapshots without performing persistence on the async event task.
- Controller regression coverage calls the terminal-response boundary without
  a pending command and requires a typed `ProgramResponseState` error rather
  than a panic.
- Actor safety tests keep a physical-class sender active while submitting an
  invalid Reset confirmation and while attempting reconnect/transport replace.
  Both operations must fail without changing sender state or writing `Ctrl-X`.
- Heartbeat tests verify that each `ok` resets acknowledgement age, updates the
  exact line/command and sequence, and freezes evidence on terminal state.
- Sender and actor tests assert structured failure kind, GRBL code, source line,
  and command for rejected responses and disconnects. UI tests format this
  contract without parsing backend error text.
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
- An actor regression keeps a serial-class sender Running and proves Mock
  Pause/Resume both fail without changing its state.
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
- `ProgramCheckGate` tests cover exact program and execution-option binding,
  15-minute expiry, reset/reconnect invalidation, disconnect observation, and
  stable-Idle-only issuance.
- Actor fixtures model a Serial execution target with deterministic Mock GRBL
  in explicit scripted-telemetry mode. Application Mock sessions instead keep
  the virtual planner enabled by default.
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
- Virtual-machine fixtures run the same GRBL Check and authorized production
  sender used by Serial, then assert `Run -> Idle`, acknowledged source lines,
  and final XYZ. Planner unit tests cover compact words and inline comments,
  absolute/incremental motion, interpolated arcs, G93 block timing,
  Hold/Resume, and Check-without-motion. Desktop tests verify that the stable
  mock fingerprint produces one persistent profile with travel derived from
  `$130/$131/$132` and an enabled probe workflow.
- Cutting `M6` fixtures prove that a known initial tool before the first cutting
  motion is confirmed as startup setup without a redundant barrier. A later
  `Tn` is acknowledged before an empty-FIFO host barrier, no `M6` bytes reach
  Mock/serial transport, ordinary Resume is rejected, stale line/tool
  confirmation is rejected before I/O, and valid completion repeats fresh
  status, Inspector, G54-G59, and final status checks. An ambiguous startup
  `M6` without a selected tool remains a barrier.
- Sender JSON contract tests pin the Rust state spelling to `toolChange`, which
  is the value consumed by the TypeScript read model, while accepting legacy
  persisted `toolchange` values during deserialization.
- PCB workflow-model tests distinguish a missing drill source from a missing
  mapping. They prove that a valid per-diameter mapping clears validation,
  mask/paste layers are ignored, and a routed slot cannot select an oversized
  tool or a side-loaded drill.
- TypeScript tests bind all six operator facts to the exact line/tool dialog;
  sender read models keep `toolChange` active and non-restartable.
- Parser fixtures validate exact XOR checksums before normalization, reject
  corrupt or ambiguous checksums, preserve leading `/`, and prove that deleting
  an optional modal block changes all following geometry. Policy and sender
  tests independently cover M1 enabled/disabled and Block Delete enabled/disabled.
- Lease tests reject an execution-option mismatch and prove that the consumed
  authorization carries the exact Optional Stop and Block Delete values. The
  Tauri parser adapter verifies that Block Delete changes preview bounds, while
  the actor fixture checks exact Check-mode writes and confirms neither deleted
  blocks nor checksum suffixes reach GRBL.
- Delayed-response fixtures prove Feed Hold is serviced within one 10 ms read
  slice instead of waiting for the command timeout. A separate fixture injects
  realtime status ahead of a delayed `ok`, verifies live `Bf/Ov` telemetry, and
  keeps FIFO acknowledgement counts unchanged. It remains active beyond the
  configured command timeout, proving that valid status refreshes the liveness
  deadline without falsely acknowledging the oldest line; the terminal-command
  stall fixture still proves that a genuinely silent controller times out.
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
  acknowledges `M2/M30` locally without writing it, and never emits Hold or Reset.
- Check plans use Cutting grammar: M3/M4/S are accepted for firmware validation,
  while the same source remains forbidden by Air policy. An isolated M6 is a
  locally acknowledged host barrier after its `Tn`; coolant, probing,
  coordinate mutation, and reference/machine movement remain blocked.
- Actor tests reject non-serial targets before I/O, run the complete multi-plane
  fixture, verify every approved line exactly once, cover correlated error, and
  require automatic cleanup to `Idle`.
- A stalled-response regression cancels an active Check, verifies the second
  `$C`, fresh `Idle`, terminal `Cancelled`, and absence of a Check certificate.
- An actor integration test proves Cutting is blocked before Check, then follows
  `Check completed -> verified Idle -> certificate -> ready preflight`; changing
  Optional Stop afterwards blocks the same source again.
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
  physical-run operator pause state. The physical controller completed 27/27
  sender steps on 2026-08-12, returned to `Idle`, and the utility then proved
  the issued certificate through a fresh Cutting preflight:

```bash
cargo run -p millo-desktop --example hardware_check_run -- \
  /dev/cu.usbmodem11101 fixtures/programs/grbl-cutting-check.nc
```

The current firmware emits a reset banner while disabling `$C`. Two initial
certificate runs correctly failed closed on that notice. The final path accepts
only one newly observed reset count inside the verified Check cleanup, clears
it, repeats status, and then issues the certificate. M30 is now host-validated
in Check and never written; physical Air/Cut behavior is unchanged.

- `grbl-path-control-check.nc` validates the exact-path command accepted by the
  target GRBL 1.1 controller:

```bash
cargo run -p millo-desktop --example hardware_check_run -- \
  /dev/cu.usbmodem11101 fixtures/programs/grbl-path-control-check.nc
```

The physical target accepted `G61` and rejected `G64` with `error:20` on
2026-08-12. Parser regression coverage therefore rejects `G64` before a plan
can reach the sender.

- `grbl-tool-change-check.nc` exercises `T2 M6` plus linear and arc geometry.
  On 2026-08-12 the physical run completed 16/16 sender steps and returned to
  `Idle`; core and actor assertions prove `T2` is the controller line while M6
  is a locally acknowledged host barrier:

```bash
cargo run -p millo-desktop --example hardware_check_run -- \
  /dev/cu.usbmodem11101 fixtures/programs/grbl-tool-change-check.nc
```

- `grbl-stream-semantics-check.nc` uses a validated checksum on every line,
  one optional motion block and M1. The two options can be exercised without
  physical motion through GRBL Check:

```bash
cargo run -p millo-desktop --example hardware_check_run -- \
  /dev/cu.usbmodem11101 fixtures/programs/grbl-stream-semantics-check.nc \
  --optional-stop --block-delete
```

On 2026-08-12 the physical controller accepted all 10/10 sender steps with both
options enabled and returned to `Idle`. The parser reported two motions because
the optional `N30` block was removed before modal/geometry construction. The
additional two steps are the typed M5/M9 shutdown epilogue before M30.

- The React check-run read model requires a loaded program, typed gateway, and
  serial target and refuses to replace an active sender. Program workspace
  exposes the action only through `start_check_run`; Rust reparses the source.
- Browser inspection at 1440 x 900 and 390 x 844 verifies the additional action
  in the preflight panel. The mobile page remains exactly 390 px wide; the
  340 px Check button stays inside its 25..365 px content bounds.

## Sender journal coverage

- `millo-journal` persists a new run immediately, throttles same-state progress
  to 250 acknowledgements or two seconds, and always persists state changes and
  terminal snapshots.
- Failed entries preserve typed GRBL failure, exact current/last-acknowledged
  line and command, but expose `RestartBlocked`, not an executable resume token.
- The JSON store keeps at most 100 runs. Tests use a two-entry bound, corrupt
  the active file, and prove load recovery from the preceding `.bak` checkpoint.
- `millo-storage` fixtures verify synced replacement preserves the previous
  generation and removes stale temporary files. Profile and settings tests each
  corrupt the primary, require backup recovery plus primary self-repair, and
  require a typed error when both copies are invalid.
- Tauri owns only the platform config path and observes the existing sender
  event stream; it cannot alter journal recovery disposition.

## Structured diagnostic log coverage

- `millo-audit` tests enforce a bounded in-memory tail, monotonic sequence,
  fixed-capacity writer queue, 5 MiB rotation policy, recent-entry restoration
  across generations, and serialized text/JSONL export.
- The desktop adapter records controller and sender snapshots plus typed
  connect, settings, Jog, zero, preflight, authorization, Start, Hold, Reset,
  resume, cancellation, disconnect, persistence, and event-bridge outcomes.
  Full source text is excluded; sender source line and current command remain.
- A persistent-log initialization error degrades to an in-memory Critical event
  instead of preventing Millo startup. Dropped queue entries and write failures
  remain visible in the UI health row.
- UI tests hide Debug by default, combine level/category/search predicates, and
  count warning/error attention. `/?fixture=logs` covers the colored structured
  viewer, expandable JSON fields, and stable overlay layout.

## Interrupted-job recovery coverage

- `millo-recovery` tests bind exact source and program fingerprint to a machine
  fingerprint and sender sequence, throttle execution checkpoints, recover a
  corrupt primary from backup, and hide completed jobs.
- Planner fixtures rewind an interruption to a prior clearance rapid, restore
  WCS/modal/tool/spindle state, omit already-finished geometry, reject safe Z
  below the program envelope, and limit missing-`Ln:` recovery to a full restart
  that includes all original geometry.
- Actor fixtures prove a prepared physical run emits no `N` block before a
  matching commit and that mismatch/discard remains motion-free. Store fixtures
  prove a failed run stays available only for the matching machine fingerprint,
  while completed runs disappear.
- Link-loss fixtures prove an active Air/Cut sender becomes terminal, the actor
  closes the controller session, no further `N` block is written, and an
  explicit reconnect cannot revive the failed FIFO.
- React model tests require all six recovery confirmations, finite Safe Z at or
  above the parser envelope, availability of the selected strategy, and atomic
  busy-state consumption. Component markup also keeps the explicit
  `Работа уже завершена` versus `Подготовить повторный запуск` decision and
  excludes the backend persistence wording from operator copy. The recovery
  fixture verifies that outstanding evidence replaces Start with
  `Разобраться с прошлым запуском` and is inspected at desktop and
  390 x 844: conservative default, continuity choices, banner, checklist,
  disabled/enabled action, low-Safe-Z rejection, and browser console.

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
- The GRBL encoder accepts only X/Y/Z, one finite signed
  `0.01..100000 mm` axis word, and `10..100000 mm/min`; it always injects
  `G91 G21` and never trusts UI formatting.
- Command actor tests prove that machine-profile distance, selected-axis travel,
  and live `$110/$111/$112` maximum rate remain authoritative below the encoder's
  technical envelope.
- Actor tests prove a lease produces at most one `$J=` write and remains consumed
  after validation failure. Missing, stale, or reused leases produce no motion.
- Mock step jog changes exactly one coordinate, reports `Jog`, completes to
  `Idle`, models bounded motion duration from distance/feed, and responds to
  realtime Jog Cancel without Reset.
- Tauri mock smoke covers Run to Hold, two-stage Reset, reset-banner
  acknowledgement, preflight, one step-jog write, lease relock, and an exact
  single-axis position update. Rust actor/mock tests cover Jog Cancel gating.
- Jog-pad actor tests prove that every press begins with a fresh status and full
  Inspector, forwards the selected bounded feed, accepts profile-approved
  distances, and does not issue another `$J` while the refreshed controller
  state is `Jog`. UI model tests cover a default `50 mm` desktop profile and a
  `3000 mm` large-router profile.

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

The physical Cutting harness performs Check and Cut in one controller session,
requires every cutting confirmation flag, rejects an out-of-envelope XY path,
uses the ordinary one-use authorization and production sender, and requests
Hold plus challenge-confirmed Soft Reset on a terminal failure:

```bash
cargo run -p millo-desktop --example hardware_cut_run -- \
  /dev/cu.usbmodem11101 job.nc \
  --execute-cut --confirm-stock-secured --confirm-tool-secured \
  --confirm-xyz-zero --confirm-safe-z --confirm-spindle-running \
  --confirm-path-clear --confirm-power-control
```

Its argument/envelope tests run under normal Rust verification; CI never passes
the required physical flags. On 2026-08-12 it completed the 1045/1045-command
solar guilloche engraving in 226.5 s and verified final `Idle`, G54 WPos
X30/Y30/Z3, and the sender shutdown tail.

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

## Contact probe regression

The repeatable typed Z-probe suite is hardware-free. Mock GRBL models a configurable
contact distance, emits `PRB:...:1`, updates the active WCS after `G10 L20`, and
reports the bounded `$J=` retract through `Jog` back to `Idle`. Tests assert the
exact probe/offset/retract commands, the contact machine position, final work Z,
and neutral `G0 G21 G90 G94` restoration. A delayed-response fixture proves
that confirmed Soft Reset preempts the probe and prevents any later `G10`.
An acknowledgement-order fixture reproduces the physical controller's
`PRB -> ok -> Run -> Idle` sequence and proves `G10` is not written before
fresh `Idle`.
Separate cases prove that an already
active `Pn:P` or an uninstalled profile probe performs no probe movement. UI
tests cover the stable clickable lamp, measured-thickness validation, live
closed-input warning, final Z calculation, and the manual-Z lockout when
`useForWorkZero` is enabled. An actor regression also proves that a direct
typed request cannot start probing while that preference is disabled.
An actor concurrency fixture starts a delayed contact, submits Work Zero while
the probe is active, and asserts an immediate `MachineOperationBusy`, no `G10`,
and no delayed replay after Soft Reset.

Automated tests never execute `G38.3` on a physical transport. A real
contact test must start with a measured plate, stationary spindle, open input,
short search distance, low feed, and the operator at the machine.

## Heightmap regression

`millo-heightmap` tests bounded serpentine planning, grid spacing, duration,
bilinear interpolation across serpentine storage, probe misses, direct/fixed
contact semantics, JSON round trips, atomic active-map replacement, and restart
disarming. Resume fixtures prove that only probe depth may change while grid and
measured samples remain immutable. Domain fixtures preserve legacy
`useForWorkZero` profiles while an
explicit new mode wins.

Command-actor tests execute a 2 x 2 map on Mock GRBL, assert exact XY order and
four contacts, prove every terminal acknowledgement settles to `Idle`, and prove
no `G10 L20` or absolute heightmap `$J=G90` is written. Failure after a delayed
contact must issue Feed Hold plus Soft Reset without any recovery motion. A
runaway regression scales an intended relative jog 35 times, verifies that the
first target mismatch is detected, and proves no probe or correction command is
sent. Dedicated Stop must consume the active operation, end in `Cancelled`, and
remain terminal without delayed writes. A Soft Reset fixture cancels before
another probe point. A link-loss fixture proves the failure cannot claim a
successful emergency stop when serial delivery was impossible. A durable-start regression
holds an operation after `prepare_heightmap`, yields the actor repeatedly, and
asserts that neither `$J` nor `G38.3` appears and the public operation remains
Idle. It then discards the exact operation sequence and proves that no motion was
published. Production Tauri persists the pending surface session between this
prepare phase and the matching commit, so fast mock runs cannot outrun their
first durable checkpoint.
The start-readiness fixture reproduces the hardware-observed race where the UI
action arrives while the last controller status is still `Run`. The actor polls
without dispatching motion, accepts a fresh `Idle` within three seconds, and
still proves that neither `$J` nor `G38.x` is written before durable commit.
Companion fixtures prove that realtime Hold remains serviceable during this
wait and that an active sender is rejected as Busy without emitting `G38.x`.
The recoverable-miss fixture verifies that clearance 2 mm plus reserve 2 mm
emits `G38.3 Z-4.000`, leaves GRBL Idle without byte `0x18`, preserves the
failing sequence, and raises back to safe Z. The resume fixture starts with two
of four samples already durable, increases reserve to 4 mm, emits exactly two
remaining probes with a 6 mm search travel, and preserves the original values.
Critical link-loss and target-mismatch fixtures continue to require quarantine.
The LUNYEE hardware regression fixture reports host-issued `$J` retract, safe-Z,
and XY moves as `Run` rather than `Jog`; both one-point calibration and a full
serpentine map must complete within computed motion deadlines. A sparse-status
fixture starts with only `MPos`, proves `$#` is read before motion, derives XYZ,
and prevents the old three-second timeout on a long first move. The full-grid
fixture also asserts a final measured-safe-Z and XY return to the captured start
position. It intentionally does not descend back to an old Z; `Completed` is
emitted only after XY settles to fresh `Idle`.
The IPC contract is pinned on both sides: Vitest serializes the complete
webview request with `originXMm`, `originYMm` and `clearanceZMm`, while a Rust
fixture deserializes that same camelCase shape into `HeightmapStartRequest`.

`millo-dry-run` additionally fixtures the execution boundary. A completed
sloped 2 x 2 map must lower a nominal Z-0.2 path to the interpolated local
surface while preserving the unmodified safe Z command. Programs outside the
measured perimeter, missing map data, incomplete samples, and implicit map
selection are rejected with `heightmap-compensation` blockers. The same map ID
is serialized in execution options so the preflight report and GRBL Check
certificate become stale when application changes.

The final-confirmation fixture also renders both execution choices. A complete,
usable but disabled map must produce a red warning with its measured Z range
and the explicit **Start processing without map** label. An enabled map must
render the positive state and require probe-wire removal. Changing that switch
is wired to a new Check run with the returned execution options; the processing
authorization path cannot reuse the report or certificate from the previous
map selection.

Vitest covers auto-perimeter margins, density independent from display
interpolation, readable serpentine matrix ordering, machine-travel validation,
real coordinate headers, explicit empty cells, and a stable low-to-high color
scale. Visual verification uses `/?fixture=heightmap` at wide and 820 px
viewports. It must show one perimeter, program motion outside it in red, probe
points, a nonblank colored mesh, and a numeric view with absolute Z plus delta;
`NaN` is never accepted. The wide layout keeps the action dock visible while
only settings scroll. The 820 px layout places setup before preview without
overlap or horizontal scrolling. `PCB/relief` presets and a permanent layer
checkbox row are regressions: actual first-contact and surface-variation limits
remain visible, while rendering layers live in the on-demand `Слои` menu.
Measured grids up to 49 points show every signed Z label and a high-contrast
ring; denser grids thin labels deterministically while retaining endpoints and
the active point. Partial adjacent cells render incrementally during probing.
The scene-model camera-scope fixture proves that another sample in the same
perimeter preserves the current 3D camera, while changing the perimeter or
Top/3D mode deliberately requests a fresh frame.
Heightmap transformation fixtures also use a non-zero first grid sample and
assert that compensation remains relative to the established work Z0 instead of
renormalizing to that first sample. Surface-quality fixtures cover a smooth
slope and a 2 mm local cliff; the final-start fixture exposes the cliff and its
physical review requirement.
Program UI tests keep the map switch in the primary readiness card, verify its
map identity and coverage warning, and keep optional-stop/block-delete controls
in the advanced drawer. One-point Z probing disarms map application without
deleting the measured data.
