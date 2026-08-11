# Architecture

## Direction of dependencies

```text
                     +------------------+
                     |   React / TS UI   |
                     +---------+--------+
                               |
                        typed IPC/events
                               |
                     +---------v--------+
                     |  Tauri adapter    |
                     +----+---------+----+
                          |         |
                +---------v--+   +--v---------------+
                |profile store|   | command arbiter  |
                +------+-----+   +---+-------+-------+
                       |             |       |
                  +----v----+   +----v---+ +-v---------+
                  | Domain  |   |readiness| |controller|
                  +---------+   +---+----+ +-----+-----+
                                    |            |
                               +----v----+  +----v-----+
                               | Domain  |  |Transport |
                               +---------+  +----+-----+
                                                  |
                                             +----v-----+
                                             |Mock/Serial|
                                             +-----------+
```

Dependencies point inward. Domain types do not import Tauri, an I/O library, or
a controller protocol. A transport moves bytes and lines; it does not interpret
GRBL. The command actor owns the controller and active transport; the controller
owns protocol state and the current snapshot. Tauri only converts application
calls into commands and events.

Program loading is a parallel read-only path:

```text
File API -> ProgramGateway -> typed Tauri parse command -> millo-gcode
                                                        -> immutable DTO
                                                        -> TS read model -> Three.js
```

It does not pass through the command actor because parsing owns no controller
state and cannot produce transport writes.

Mock dry-run execution is a separate, explicitly gated path:

```text
Original source -> Rust reparse -> millo-dry-run -> opaque DryRunPlan
                                                    |
                                            command actor/sender
                                                    |
                                            Controller -> Mock GRBL
```

Neither an immutable preview DTO nor a React flag can become a sender plan.

Serial real-run preparation has a read-only report followed by a separate
one-use authorization gate:

```text
Original source -> Rust reparse -> command actor -> ? + Inspector + ?
                                      |                    |
                                   serial-only         fresh snapshot
                                      +--------> millo-run report -> React

Original source + six confirmations -> command actor -> fresh preflight
                                                       -> first-cut lease
```

The lease is not a sender plan and this path still has no serial dispatch.

## Rules

1. CNC behavior belongs in Rust and must be testable without Tauri.
2. The UI never parses controller wire messages.
3. Protocol crates never open ports or sockets.
4. Transport implementations never mutate machine state.
5. New compatibility behavior starts as a fixture or failing test.
6. Public domain types are serializable, explicit, and preserve unknown input.
7. Safety-critical actions will be modeled as state transitions, not raw UI
   strings.
8. Parsed source and preview geometry never imply permission to send a program.

## Implemented vertical slices

The desktop command `connect_transport` selects either a deterministic mock or a
discovered native serial port, sends the GRBL realtime `?` byte, parses the returned status frame, updates
`ControllerSnapshot`, emits `machine-state`, and returns the same snapshot to the
caller. The UI treats the event as authoritative.

### Machine profiles

- `millo-profile` owns validation, schema versioning, stable IDs, selected state,
  GRBL-derived drafts, and JSON I/O without depending on Tauri or serial code.
- A profile requires a name and finite positive XYZ travel. Spindle workflow,
  homing, limits, probe, and physical emergency-stop declarations are explicit;
  unverified hardware defaults to absent.
- Tauri resolves the application configuration path and exposes typed
  list/create/select/detect/update operations. A disconnected operator may
  select a profile; a connected unknown serial device may create and bind only
  the draft derived from that live controller session.
- Detection opens a temporary controller session, performs only status and
  Inspector reads, derives travel from `$130/$131/$132`, and closes the port.
  `$21/$22` remain controller settings and do not prove physical switches;
  limits, homing, probe, and emergency-stop facts all stay off until the
  operator declares the installed hardware.
- Connection now precedes identity resolution. Tauri performs a complete
  Inspector read, builds a fingerprint, selects exactly one matching profile or
  opens onboarding, and blocks Jog, Work Zero, and serial run preflight while a
  serial controller is unbound.
- A real USB serial number forms a strong fingerprint. Devices such as the
  current LUNYEE controller that report no unique serial use a clearly labelled
  port-bound fallback containing VID/PID, product, and native port. Firmware is
  displayed as observed metadata but is deliberately excluded from the stable
  key so a firmware update does not create a new machine.
  Multiple matches fail closed and require operator resolution.

### Controller settings synchronization

- `millo-settings` is independent of Tauri, React, and serial I/O. It maps every
  standard GRBL 1.1 `$` key to a group, type, and unit while retaining unknown
  numeric firmware settings under Advanced.
- The connected controller is always authoritative. The local file at
  `machines/<profile-id>.settings.json` is a duplicate for rollback and history,
  never an offline configuration pushed automatically on connect.
- `connect_transport` reads all settings and starts an immutable session
  baseline. Reconnecting archives the previous baseline when the session or
  controller state changed, keeps at most 20 revisions, and starts a new
  baseline from the newly observed controller values.
- Controller edits carry the UI revision, expected old value, target value, and
  explicit editing grant. The Tauri adapter rejects stale revisions. The actor
  then performs `?`, complete Inspector, one typed `$n=value`, another `?`, and
  another complete Inspector. A changed external value or mismatched stored
  result fails the operation.
- The archive's current values are replaced only after a successful fresh read.
  A crash, timeout, `error`, or `ALARM` cannot manufacture a saved state.
- Controller fields autosave after a 650 ms debounce. The UI serializes multiple
  field edits, shows pending/writing/verified/error per field, rolls back to the
  connection baseline, and can explicitly restore a value from the preceding
  archived session.

The command arbiter owns the periodic driver. Every tick calls one core method:
a connected controller is polled, while a recovering controller attempts
disconnect/connect/status synchronization. Tauri subscribes to snapshots and
does not schedule or execute controller I/O.

### G-code program boundary

- `millo-gcode` owns source limits, lexical normalization, modal parsing,
  warnings, safety features, bounds, distances, and sampled preview geometry.
  It imports neither GRBL controller code nor Tauri.
- The first parser supports comments, compact words, `G0/G1/G2/G3`, G17 XY
  arcs with I/J or R, `G20/G21`, `G90/G91`, common modal cancels, and millimetre
  normalization. Unsupported behavior is retained as a source-line warning; it
  is never guessed into geometry.
- Input is bounded to a 255-byte source name, 2 MB, and 200,000 lines. Preview
  output is bounded to 500,000 points.
- Safety/error warnings make `dryRunEligible` false. `M3/M4`, non-zero spindle
  speed, `M6`, `G38.x`, `G53`, coordinate mutation, coolant activation, and
  malformed/unsupported geometry are among the fail-closed cases.
- React reads files through the browser File API and passes only name/source to
  a platform-neutral `ProgramGateway`. The Tauri adapter runs parsing on a
  blocking worker and returns the typed immutable DTO.
- `toolpathReadModel` is a pure TypeScript adapter that separates rapid and cut
  line pairs and derives a stable scene frame. Three.js is lazy-loaded only when
  a parsed program exists.
- Program line selection is component-local read state keyed by the parser's
  one-based `sourceLine`. It never edits `ProgramLine`, normalized source, or
  sender plans.
- `programLineTableModel` computes a fixed-height overscanned window, and the
  React table mounts only that slice. The full line count is represented by a
  spacer inside a bounded desktop/mobile viewport.
- `buildToolpathHighlightReadModel` extracts only segments whose `sourceLine`
  matches the selection and uses the same scene center as the base model.
- Three.js keeps a persistent selection line/point layer. Selection changes
  replace only those two geometries and adjust base material opacity; they do
  not recreate the renderer, camera, controls, grid, or full path buffers.
- Lines without preview motion produce an empty overlay and a visible
  `No preview motion` state rather than borrowing adjacent geometry.
- The scene contains one XY grid, functional rapid/cut colors, top and
  isometric views, bounded zoom/pan, and no machine-state mutation.
- `Program` and `Controller` are separate retained workbench views. Program
  state survives tab changes; Device Inspector remains available without being
  mixed into preview diagnostics.
- The original source is retained beside the immutable preview. Starting a dry
  run sends that source back to Rust for a fresh parse and independent policy
  check; the UI's `dryRunEligible` display flag is never authority.
- `millo-dry-run` rejects every parser safety/error plus explicit M3/M4,
  non-zero S, M7/M8, M6, G38.x, G28/G30/G53, and G10/G92 families. It limits a
  normalized command to 255 bytes and prepends only the safe M5/M9 off commands.
- `DryRunPlan` and `DryRunLine` have private fields and are not deserializable.
  Only the Rust policy can mint commands accepted by the controller sender API.
- `millo-sender` limits plan lines and bytes, permits one in-flight command,
  advances only on correlated `ok`, and models Ready/Running/Paused/Completed/
  Failed/Cancelled explicitly.
- Sender dispatch runs inside the existing command actor. Requests remain
  prioritized between lines, lifecycle polling uses the same controller, and
  no second task can write to the transport.
- Tauri checks the active descriptor and the actor checks `DryRunTarget`; both
  must identify Mock GRBL. Serial replacement automatically disables and
  cancels dry-run execution.
- React receives a separate `dry-run-state` event with bounded progress,
  current source line, and terminal error. Plugins receive no sender or raw-line
  capability. `jobs.create` remains reserved.

### Real-run preflight boundary

- `millo-run` owns the first-run report. React displays its typed checks but
  does not decide whether a program or controller is ready.
- Tauri reparses the retained original source. The preview DTO and its
  `dryRunEligible` flag are not accepted as execution evidence.
- Both Tauri and the command actor require the serial execution target. Mock and
  disabled targets fail before controller I/O.
- The actor performs one serialized `?`, `$I`, `$$`, `$G`, `$#`, `?`
  transaction. It requires stable `Idle` before Inspector and assesses the final
  status, so a stale UI snapshot cannot clear preflight.
- The program must pass the existing fail-closed motion-only policy and contain
  complete bounded preview motion. M3/M4/non-zero S, coolant, probing, M6,
  machine/reference-coordinate motion, coordinate mutation, malformed geometry,
  and unsupported safety behavior remain blockers.
- Before relevant motion, the file must explicitly establish `G21` units,
  `G90` distance mode, `G94` feed mode, and `G17` for XY arcs. Ambient GRBL
  modal state cannot fill parser defaults for a physical run.
- Probe readiness is not motion-critical for a program that is forbidden from
  probing. Firmware, XYZ tuning, unhomed settings, units, milling mode, active
  G54-G59, alarm/reset, and controller state remain enforced.
- Missing homing/limits, manual spindle workflow, and unconfirmed stock, cutter,
  work zero, safe Z, and clearance remain explicit cautions.
- The Preflight diagnostics tab links a source-addressable blocker back to the
  immutable program-line selection. That selection still cannot alter policy or
  future execution order.
- A clear report only reveals the first-cut checklist. All six physical facts
  are mandatory: stock, cutter, XYZ work zero, safe Z, running manual spindle,
  and reachable power control.
- The authorize request reparses the retained source and repeats the complete
  preflight inside the actor. React cannot submit its previous report as
  evidence.
- `millo-run` binds the 30-second lease to a SHA-256 program fingerprint,
  reset/reconnect counters, and observed machine/work positions. It is removed
  by expiry, a non-Idle observation, position/session change, profile/settings
  mutation, jog, work-zero, reset, reconnect, or a failed consumption attempt.
- A future serial sender must consume the matching lease once inside its start
  transaction. There is currently no serial sender, safety preamble, or Start
  control, so authorization still emits no program command.

### Lifecycle invariants

- Only one actor task owns controller I/O and periodic polling.
- Poll ticks use skip semantics and never build a backlog.
- Every status transaction has a bounded timeout.
- A successful transport connection starts polling even if the initial status
  synchronization fails.
- A single failed poll is transient; the configured failure threshold moves the
  controller to `Recovering`.
- `NotConnected` moves to recovery immediately.
- Recovery succeeds only after reconnect and a valid status frame.
- A reset banner clears stale machine/alarm state and remains visible until the
  operator acknowledges it.
- An `ALARM:n` line and `<Alarm|...>` status are machine state, not transport
  failure. Alarm clears only after a non-alarm status arrives.
- Disconnect changes actor lifecycle state, closes the transport, and clears
  session telemetry; the dormant ticker performs no I/O while disconnected.

### Native serial boundary

- `millo-serial` owns OS port discovery, baud configuration, asynchronous byte
  writes, and CR/LF line framing.
- It implements the same `Transport` contract as `millo-mock`; neither serial
  nor mock parses GRBL or changes machine state.
- The command actor stores `Controller<BoxedTransport>`, so transport selection
  changes construction only, not controller policy.
- Serial targets are checked against fresh native discovery before opening.
- The default UI filter uses only discovery metadata: USB transport kind,
  GRBL/CNC/FluidNC names, common board and USB-UART names, and known vendor IDs.
  It is intentionally advisory and can be disabled to expose every port.
- EOF and pre-connect I/O become `TransportError::NotConnected`; platform I/O
  failures preserve their message as `TransportError::Io`.
- Reconnection drops and reopens the native handle through the existing
  controller lifecycle.
- Physical hardware is not required by the automated suite. Native enumeration
  and an operator hardware smoke test cover the OS boundary.
- macOS `/dev/cu.*` and `/dev/tty.*` aliases with the same device suffix are
  deduplicated in `millo-serial`; the callout (`cu`) path wins before descriptors
  reach Tauri.

### Command and inspection boundary

- A bounded FIFO channel is the only path to the active controller.
- Realtime bytes and newline-terminated commands are distinct request types.
- Status `?` consumes its matching status frame before another request runs.
- Device Inspector permits only `$I`, `$$`, `$G`, and `$#`.
- `ok`, `error:n`, `ALARM:n`, and reset terminate and classify the active line
  request; asynchronous status/reset information still updates the snapshot.
- Rust parses firmware, settings, modal state, and coordinate parameters. The UI
  never receives a responsibility to interpret wire lines.
- Tauri exposes no raw command, general G-code, or spindle-control endpoint. Its
  machine-changing calls are named, typed use cases described below.

### Work-coordinate boundary

- Work Zero accepts only X, Y, or Z plus a one-attempt operator confirmation.
  Missing confirmation is rejected before controller I/O.
- The actor obtains a fresh status and requires stable `Connected + Idle` with no
  alarm or pending reset before any write.
- A fresh `$G` determines the active G54-G59 coordinate system. The Rust encoder,
  not React, maps it to `P1..P6` and emits exactly one `G10 L20 Pn <axis>0` line.
- The actor then reads `$#`, requires the matching coordinate parameter to be
  present and parseable, refreshes status, and verifies the selected work
  coordinate is within `0.002 mm` of zero.
- Any work-zero attempt invalidates an outstanding test-jog authorization. It
  grants no movement capability and exposes no arbitrary coordinate command.
- The first slice is covered by Mock GRBL and unit/UI tests only. Physical
  execution remains a separate operator-confirmed hardware check.

### Hardware readiness boundary

- `millo-readiness` evaluates parsed domain data; it neither sends GRBL commands
  nor imports Tauri or a transport implementation.
- The selected hardware profile is explicit: XYZ, manual spindle, no homing,
  no limit switches, and no physical emergency stop.
- Missing or invalid axis steps, rates, acceleration, travel, firmware identity,
  required query responses, milling mode, or profile-consistent `$20/$21/$22`
  settings block the future guarded test jog.
- Unhomed coordinates, manual spindle operation, missing emergency stop, active
  `G91`, and an electrically untested probe remain visible cautions.
- `testJogReady` is an inspection result, not a general motion permission. The
  step-jog command re-checks live controller state inside the command actor
  immediately before writing bytes.
- React discards the displayed report when the live snapshot leaves stable
  `Connected + Idle`, receives an alarm, or receives a reset notice. The
  Inspector must be read again after recovery.
- Probe readiness stays false until a separate stationary electrical test is
  implemented and passed.

### Realtime safety boundary

- `millo-safety` owns actor-local reset challenges and test-jog authorization;
  React state cannot authorize a controller write by itself.
- Feed Hold is encoded as the GRBL realtime `!` byte. It is exposed as a named
  Tauri command, never as a raw byte or arbitrary G-code endpoint.
- Soft Reset requires an actor-issued challenge that expires after 10 seconds.
  Confirmation consumes the challenge before `Ctrl-X` is written, so retrying
  the same confirmation cannot reset the controller twice.
- Test-jog preflight requires explicit confirmation that the spindle is off,
  the tool is clear, and machine power is within operator reach. The actor then
  re-runs all four Inspector queries and assesses the resulting live snapshot.
- A successful preflight creates a 15-second single-use lease. Alarm, reset,
  reconnect, disconnect, non-idle state, expiry, or another realtime command
  invalidates it.
- Every preflight starts with a fresh realtime status transaction before the
  four Inspector queries, so a stale UI `Idle` snapshot cannot authorize a
  second click while GRBL already reports `Jog`.
- The typed step-jog endpoint consumes the lease inside the actor before the
  controller validates and writes the command. Validation or transport failure
  does not restore the lease.
- The GRBL encoder always emits `$J=G91 G21` with exactly one of X/Y/Z. Absolute
  distance is limited to `0.01..1.00 mm`; feed is limited to
  `10..100 mm/min`. UI values cannot widen this backend envelope.
- A successful `ok` means GRBL accepted the jog for execution; periodic status
  remains authoritative for `Jog` and final position. The jog pad never receives
  or stores its actor-local lease.
- The operator jog pad uses one higher-level actor request per click. That
  request performs status, Inspector, readiness, lease issue, lease consumption,
  and one typed step without exposing the authorization to React. It accepts
  only `0.01` or `0.10 mm` and always uses `10 mm/min`; the broader typed
  step-jog envelope remains available to explicit hardware tooling.
- Jog Cancel is the named `0x85` realtime operation and is accepted by the actor
  only while its current snapshot reports `Jog`.
- First-machine setup exposes no general `$n=value` endpoint. A narrow
  actor-only operation may disable `$21` hard limits and `$22` homing while
  stable `Idle`; it reads settings before and after, skips values already zero,
  and fails verification unless both final values are exactly `0`.
- The single actor remains the only writer to the port. A realtime byte is
  serialized behind an already active controller transaction; the sender
  returns to the actor queue after every correlated line result so queued
  realtime requests are considered before the next line.

### Extension host boundary

- React feature modules depend on typed platform gateways, not Tauri imports.
  The jog pad is the first feature following this rule through
  `MachineCommandGateway`.
- The generic `ExtensionRegistry` composes named slots and gives each
  contribution an ID, owner, order, replacement list, and deterministic unload
  lifecycle. It has no React, Tauri, or machine dependency.
- The React bridge exposes `control.machine` and `control.coordinates`. Jog Pad
  and Work Zero are separate `core` contributions, so coordinate controls do not
  become part of the guarded motion capability by accident. A later plugin can
  replace a contribution, and unloading it restores core UI without remounting
  the application shell.
- Machine, probing, sender, job, storage, and network access are separate,
  manifest-declared capabilities. A plugin receives only host proxies granted to
  it; it never receives serial I/O, actor internals, or raw command endpoints.
- Machine capability calls still execute Rust application use cases and preserve
  all safety policy regardless of whether the caller is core UI or a plugin.
- Plugin manifests have independent `manifestVersion` and `apiVersion` fields;
  both are version `1`. Required capabilities fail activation when absent, while
  denied optional capabilities are reported and omitted from the activation
  context.
- The current in-memory loader supports `ui.contribute`, `machine.jog`, and
  `machine.read` when their typed host services are supplied. `jobs.create`
  remains a reserved catalog entry. Unknown capabilities and API versions are
  rejected.
- `MachineSnapshotStore` clones each controller DTO and freezes the snapshot,
  machine state, positions, alarms, and reset notices before exposing them.
  `machine.read` provides only `current()` and future-update `subscribe()`; it
  cannot refresh, poll, connect, send commands, or access Tauri events.
- Every plugin activation owns a resource scope. Machine subscriptions are
  tracked there and disposed as unload begins or after failed activation, before
  waiting for plugin deactivation. Retained UI, read, and jog proxies reject use
  after that scope closes. Subscriber failures are isolated and may be reported
  through the host error callback without interrupting other listeners.
- UI plugins receive a registrar that binds contributions to the manifest owner
  and enforces an owner-prefixed ID. They do not receive the shell's internal UI
  context. Activation failure and unload remove every contribution owned by the
  plugin.
- This loader accepts only modules already linked into the application. It does
  not read plugin files, dynamically import code, or establish a sandbox or
  signature trust model.
- `bootstrapPluginHost` is the application composition root for the UI registry,
  `MachineSnapshotStore`, and in-memory loader. It registers core UI but does not
  discover or activate plugins. Grants therefore remain empty unless the host
  explicitly supplies them.
- React reads the shared machine store through `useSyncExternalStore`. Typed
  command results publish back to that store instead of maintaining a second
  component-owned controller snapshot.
- `bindMachineStateStream` connects the initial `controller_snapshot` query and
  live Tauri `machine-state` events to the same store. An event revision prevents
  a late initial response from overwriting newer state, and a late async listen
  setup is immediately cleaned up after effect disposal.
- See `docs/decisions/0010-extension-host-boundaries.md` and
  `docs/decisions/0011-versioned-plugin-manifest.md`, plus
  `docs/decisions/0012-machine-read-capability.md` and
  `docs/decisions/0013-plugin-host-bootstrap.md` for the accepted boundaries.

## Near-term sequence

1. Define a first-cut operator authorization covering stock, cutter, verified
   XYZ work zero, safe Z, manual spindle state, and immediately reachable power.
2. Generalize the opaque policy-approved plan without giving React raw command
   authority, then exercise it against Mock with current-line following.
3. Add a serial sender only after start/hold/cancel/error/reset behavior and
   one-line-in-flight recovery are proven by fixtures; keep probing unavailable.

Three.js is now isolated behind the program preview adapter and a lazy bundle.
Ant Design remains absent until a workflow needs its component contracts rather
than adding it as general UI weight.
