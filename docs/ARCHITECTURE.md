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
                                      Controller -> Mock or serial GRBL
```

Neither an immutable preview DTO nor a React flag can become a sender plan.

Serial execution has a read-only report followed by a separate one-use
authorization and Start gate:

```text
Original source -> Rust reparse -> command actor -> ? + Inspector + ?
                                      |                    |
                                   serial-only         fresh snapshot
                                      +--------> millo-run report -> React

Original source + intent confirmations -> command actor -> fresh preflight
                                                           -> one-use lease
Original source + lease ID -> command actor -> fresh status -> consume lease
                                                        -> sender -> GRBL
```

The lease is not a sender plan. Only the actor can turn the original source and
matching lease into serial dispatch.

GRBL Check is a sibling validation path, not a weaker run authorization:

```text
Original source -> typed Tauri command -> Rust reparse -> Cutting policy
                                                   -> actor -> $C -> one-line FIFO
                                                   -> verified $C exit -> Idle
                                                   -> program/session certificate
                                                   -> Cutting preflight
```

The UI cannot pass a prepared plan or toggle `$C`. M0 is a syntax-validation
line in this mode; M1 is sent only when Optional Stop is enabled. Neither enters
the physical-run operator pause state while GRBL is in Check.

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

### Webview security boundary

- Production CSP admits only bundled/local assets and Tauri IPC. It has no
  remote network origin, wildcard, object embedding, frame ancestry, or
  `unsafe-eval`; development adds only the fixed Vite websocket origin.
- `scripts/check-security.mjs` keeps that policy structural and runs in the
  default test pipeline. Tauri still injects its build-time nonces/hashes for
  bundled assets as documented by the official Tauri v2 CSP contract.
- The main window has only `core:default` capability. Custom Millo commands are
  additionally constrained by their Rust-side typed state and safety checks;
  frontend visibility is never treated as authorization.

### Local persistence boundary

- `millo-audit` is an independent best-effort diagnostic boundary. Calls append
  to a bounded memory tail and use a fixed nonblocking queue; a dedicated thread
  owns JSONL writes, flush, rotation, restore, and export. Audit backpressure or
  disk failure can increment health counters but cannot delay controller/sender
  I/O or prevent application startup. Audit entries are evidence only and can
  never authorize motion or resume a job.
- `millo-storage` is the only implementation of local temp/backup replacement.
  It writes a new file with `create_new`, flushes it with `fsync`, moves the
  current primary to `.bak`, renames the complete temporary file, and syncs the
  parent directory on Unix.
- Profiles, controller-setting archives, and sender journals use that contract.
  A malformed primary is read from the preceding valid backup and immediately
  repaired while the backup remains available. If both JSON copies are corrupt,
  startup returns an explicit error instead of silently resetting operator data.
- `millo-recovery` persists one exact active source, its fingerprint, machine
  fingerprint, execution options, start positions, sender sequence and latest
  physical `Ln:` checkpoint. It uses the same synced temp/backup replacement.
  Completed jobs are never offered. A missing `Ln:` disables checkpoint replay
  but preserves a guarded full restart from the beginning.
- Physical Start uses a prepared/commit actor boundary. The prepared sender has
  a run sequence but dispatch is disabled. Tauri arms and syncs recovery first;
  only a matching commit releases the source FIFO. Persistence failure discards
  the prepared sender without writing a G-code block.
- Recovery planning is pure and motion-free. It reparses the stored source,
  verifies its fingerprint, and creates one of two explicit programs. Proven
  continuity plus `Ln:` rewinds to the latest preceding rapid at clearance;
  unknown/lost motion power starts the exact source from its first checkpoint.
  Both use M5/M9 and an operator-reviewed Safe Z. The generated program must
  still pass preview, GRBL Check, preflight and one-time authorization.
- Domain-specific stores still own schema validation and bounded history;
  `millo-storage` knows nothing about profiles, GRBL, sender state, or JSON.

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
- Every queued write is fenced by dialog lifecycle, controller fingerprint, and
  bound profile ID. Closing the dialog or switching machines cancels pending
  timers and makes queued completions stale, so an old draft cannot be written
  into a newly connected controller even when setting keys/revisions coincide.
- A successful firmware write remains `saved` even if the secondary profile-list
  refresh fails; that ancillary error is reported separately instead of falsely
  claiming the controller rejected a change it already stored.

The command arbiter owns the periodic driver. Every tick calls one core method:
a connected controller is polled, while a recovering controller attempts
disconnect/connect/status synchronization. Tauri subscribes to snapshots and
does not schedule or execute controller I/O.

### G-code program boundary

- `millo-gcode` owns source limits, lexical normalization, modal parsing,
  warnings, safety features, bounds, distances, and sampled preview geometry.
  It imports neither GRBL controller code nor Tauri.
- The parser supports comments, compact words, `G0/G1/G2/G3`, circular and
  helical IJK/R arcs in `G17/G18/G19`, full circles, `G20/G21`, `G90/G91`,
  GRBL's incremental `G91.1` arc centers, `G93/G94`, `G4`, common modal
  cancels, and millimetre normalization. Unsupported behavior is retained as a
  source-line warning; it is never guessed into geometry.
- Modal-group conflicts, wrong-plane arc offsets, misplaced context words, and
  cutting moves without a usable feed are errors before policy or hardware.
  Absolute arc centers (`G90.1`) can be previewed but block GRBL 1.1 execution.
- Every cutting segment carries a feed and estimated duration. Dwell contributes
  exact commanded time. A rapid segment makes the program estimate incomplete,
  because controller acceleration and axis-specific maximum rates are not part
  of a portable G-code file.
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
- Program composes a host-owned job-readiness read model from typed controller,
  parser, work-coordinate, preflight, and GRBL Check facts. The model returns
  four stable status rows and exactly one contextual action; React never infers
  authority from button order or locally cached success.
- G54-G59 work position is the primary operator coordinate. A pure read model
  prefers controller WPos and otherwise derives it from MPos plus WCO or fresh
  Inspector G5x/G92/TLO evidence. G53 is retained as a compact secondary
  diagnostic coordinate.
- Alarm Unlock and work-zero actions remain typed actor requests. The Program
  surface can invoke them contextually, but cannot write `$X`, `G10`, or raw
  serial bytes itself. See ADR 0043.
- The original source is retained beside the immutable preview. Starting a dry
  run sends that source back to Rust for a fresh parse and independent policy
  check; the UI's `dryRunEligible` display flag is never authority.
- `millo-dry-run` rejects every parser safety/error plus explicit M3/M4,
  non-zero S, M7/M8, Air-run M6, G38.x, G28/G30/G53, and G10/G92 families. It limits a
  normalized command to 255 bytes and prepends only the safe M5/M9 off commands.
- `DryRunPlan` and `DryRunLine` have private fields and are not deserializable.
  Only the Rust policy can mint commands accepted by the controller sender API.
- `millo-sender` limits plan lines and bytes, fills at most its configured GRBL
  RX byte window, releases exact command-plus-newline bytes on correlated FIFO
  `ok`, and models Ready/Running/Paused/ToolChange/Draining/Completed/Failed/Cancelled.
- Every successful plan load increments `runSequence`; it is stable for the
  complete run and independent from the per-acknowledgement progress sequence.
- Cutting policy compiles isolated `M6` into a private host barrier. Any `Tn`
  is sent and acknowledged first; the barrier waits for an empty FIFO and never
  reaches GRBL. Completion is a separate typed actor request bound to source
  line/tool, full operator confirmation, fresh `Idle`, Inspector, G54-G59, and
  final `Idle`. See ADR 0034.
- Sender dispatch runs inside the existing command actor. Requests remain
  prioritized between lines, lifecycle polling uses the same controller, and
  no second task can write to the transport.
- Waiting for the oldest program response is incremental. The controller keeps
  the pending command, diagnostic lines, and original command deadline across
  10 ms read slices; actor requests run before the next slice.
- While a command response is pending, the ticker writes realtime `?` instead
  of starting a competing status transaction. The same response demultiplexer
  applies status frames and preserves the oldest FIFO `ok/error/ALARM`.
- Sender reconciliation distinguishes a newly observed status frame from a
  pending read or terminal response. Stale preflight `Idle` can never release a
  deferred `M2/M30`.
- Tauri checks the active descriptor and the actor checks `DryRunTarget`; both
  must identify Mock GRBL. Serial replacement automatically disables and
  cancels dry-run execution.
- Mock Pause and Resume are target-checked again inside the actor. Direct IPC or
  a future adapter cannot apply host-only dry-run transitions to an Air/Cutting
  sender; physical pause/resume remains the typed Feed Hold/Cycle Start workflow.
- React receives a separate `dry-run-state` event with bounded progress,
  current source line, and terminal error. Plugins receive no sender or raw-line
  capability. `jobs.create` remains reserved.
- The same event bridge passes snapshots through a bounded queue to a dedicated
  persistence worker thread. JSON serialization, `write`, and `fsync` therefore
  cannot block the Tokio controller/sender runtime. The worker updates journal
  history at state changes, every 250 acknowledgements or two seconds, and at
  terminal state; it independently checkpoints changed physical `Ln:` recovery
  evidence at most once per second and at terminal state. Backpressure may delay
  journal/UI forwarding, but it never delays controller I/O or drops a terminal
  checkpoint. The adapter owns config paths; each store owns schema, throttling,
  backup replacement, corruption reporting, and domain policy.

### GRBL Check-run boundary

- Check run is a serial-only typed use case. It builds the same opaque,
  parser-approved Air-run plan before touching the controller; React and plugins
  still cannot supply lines.
- The actor performs fresh status, Inspector, and status transactions, then
  `Controller::set_check_mode(true)` permits only `Idle -> Check`, sends `$C`,
  and verifies the result through another status read.
- `SenderMode::CheckRun` uses the same source-line correlation and capacity
  validation but allows only one unacknowledged command. A rejected block
  therefore leaves no later response tail that could be mistaken for the `$C`
  cleanup acknowledgement.
- Program end is parser/policy-validated and acknowledged locally in Check mode;
  M2/M30 does not reach firmware and there is no motion planner to drain.
  Completion, correlated error, cancellation, disconnect,
  and transport replacement all attempt a verified `Check -> Idle` transition.
- Firmware may emit a reset banner while disabling `$C`. The actor accepts only
  one reset count increment first observed inside that successful cleanup,
  clears its notice, and requires another clean `Idle` status. Any earlier or
  unrelated reset still fails the run and mints no certificate.
- Completion mints a `ProgramCheckCertificate` only after the verified exit.
  The certificate expires after 15 minutes and is bound to source SHA-256,
  Optional Stop, Block Delete, reset count, and reconnect count. Failed,
  cancelled, disconnected, or uncleanly closed Check runs mint nothing.
- Check-run failures do not send Feed Hold or Soft Reset. Physical Air/Cut runs
  retain their buffered-motion abort policy and full RX-window streaming.
- The physical GRBL 1.1f fixture covers G17/G18/G19 arcs, helices, explicit
  full-circle endpoints, G90/G91, G93/G94, and G4. All 25 sender lines were
  accepted and the controller returned to `Idle`. The newer cutting fixture
  completed 27/27 sender steps and its post-Check Cutting preflight accepted the
  certificate on the physical controller.

### Real-run execution boundary

- `millo-run` owns the first-run report. React displays its typed checks but
  does not decide whether a program or controller is ready.
- Tauri reparses the retained original source. The preview DTO and its
  `dryRunEligible` flag are not accepted as execution evidence.
- Both Tauri and the command actor require the serial execution target. Mock and
  disabled targets fail before controller I/O.
- The actor performs one serialized `?`, `$I`, `$$`, `$G`, `$#`, `?`
  transaction. It requires stable `Idle` before Inspector and assesses the final
  status, so a stale UI snapshot cannot clear preflight.
- The selected intent is part of preflight evidence. `AirRun` blocks
  M3/M4/non-zero S; `Cutting` permits those spindle words after its physical
  checklist. Coolant, probing, machine/reference-coordinate motion,
  coordinate mutation, malformed geometry, and unsupported safety behavior
  remain blockers in both modes.
- `Cutting` additionally requires a valid program/session-bound Check
  certificate. Rechecking a different file or changing Optional Stop/Block
  Delete cannot authorize the current preflight. `AirRun` remains available as
  the spindle-off physical validation path.
- An isolated Cutting `M6` is the exception: it becomes a host-only barrier,
  and the operator must verify the replacement tool, Z zero, safe Z, remaining
  path, manual spindle, and power access. Ordinary Resume cannot leave this
  state; the actor repeats fresh `Idle -> Inspector -> Idle` before dispatch.
- Before relevant motion, the file must explicitly establish `G21` units,
  `G90` distance mode, either `G93` or `G94` feed mode, and `G17`, `G18`, or
  `G19` before its first arc. Ambient GRBL modal state cannot fill parser
  defaults for a physical run.
- Probe readiness is not motion-critical for a program that is forbidden from
  probing. Firmware, XYZ tuning, unhomed settings, units, milling mode, active
  G54-G59, alarm/reset, and controller state remain enforced.
- Missing homing/limits, manual spindle workflow, and unconfirmed stock, cutter,
  work zero, safe Z, and clearance remain explicit cautions.
- The Preflight diagnostics tab links a source-addressable blocker back to the
  immutable program-line selection. That selection still cannot alter policy or
  future execution order.
- A clear report reveals the matching checklist. Air run requires removed tool
  and stopped spindle; cutting requires secured stock/tool and running manual
  spindle. XYZ work zero, safe Z, clear path, and reachable power are common.
- The authorize request reparses the retained source and repeats the complete
  preflight inside the actor. React cannot submit its previous report as
  evidence.
- `millo-run` binds the 30-second lease to a SHA-256 program fingerprint,
  reset/reconnect counters, and observed machine/work positions. It is removed
  by expiry, a non-Idle observation, position/session change, profile/settings
  mutation, jog, work-zero, reset, reconnect, or a failed consumption attempt.
- Production Start refreshes status, rebuilds the intent-specific plan, consumes
  the matching lease, and starts the shared bounded-RX state machine in one
  actor request. Authorization alone still emits no program command.
- `SenderSnapshot.mode` distinguishes `mockDryRun`, `checkRun`, `airRun`, and `cutRun`
  without creating separate implementations. `M0` pauses after `ok`; `M1`
  does so only when Optional Stop was bound into the plan;
  `M2/M30` terminate dispatch. For physical modes the terminal line is retained
  as an unsent barrier while the sender enters `Draining`; a fresh `Idle`
  dispatches it and only its `ok` permits completion. Hold/Resume and Reset stay
  responsive while the planner drains. Error, alarm, reset, timeout, polling
  failure, or transport loss fails at the correlated line. Because later FIFO
  lines may already be accepted by GRBL, a physical command rejection or
  response failure triggers best-effort realtime Hold followed by Soft Reset to
  flush the controller receive and planner queues.
- Physical modes cannot use plain Cancel. Operator stop is Feed Hold followed by
  challenge-confirmed Soft Reset.

### Lifecycle invariants

- Only one actor task owns controller I/O and periodic polling.
- Poll ticks use skip semantics and never build a backlog.
- Every status transaction has a bounded timeout.
- GRBL 1.1 `Bf`, `Ov`, `Pn`, `A`, and `Ln` status fields are parsed into typed
  buffer, override, input, accessory, and source-line state. Missing optional
  telemetry remains absent instead of being synthesized.
- Status polling continues while the bounded sender has lines in flight. The
  single actor writes `?`; program-response polling consumes and classifies the
  interleaved frame without a second transport reader.
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
- Alarm Unlock is a typed actor operation, not a raw-console command. It requires
  explicit operator confirmation, rereads `Alarm`, sends exactly `$X`, and is
  successful only after another status read reports `Idle` without alarm.
- Disconnect changes actor lifecycle state, closes the transport, and clears
  session telemetry; the dormant ticker performs no I/O while disconnected.
- Connect and transport replacement are accepted only from `Disconnected`.
  Reconnect/replacement requests cannot silently cancel an active sender or
  abandon buffered physical motion. If Tauri fails during status, Inspector, or
  profile synchronization after opening a port, it closes the controller and
  clears the incomplete settings session before returning the error.

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
- Native framing admits at most 4 KiB including the line terminator. A noisy
  device that never emits newline returns typed `LineTooLong` instead of growing
  process memory without bound; EOF with an incomplete frame is a disconnect,
  never a parseable GRBL response.
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
- Inspector retains raw `[OPT]` text and separately parses option flags, planner
  block count, and RX byte capacity. A first-cut lease carries the observed RX
  capacity to Start, which configures the sender to `RX - 1`; invalid, missing,
  or implausibly large reports fall back to or are capped by sender policy.
- `ok`, `error:n`, `ALARM:n`, and reset terminate and classify the active line
  request; asynchronous status/reset information still updates the snapshot.
- A missing or mismatched internal program-response context is returned as a
  typed controller error. Protocol/state desynchronization cannot panic the
  desktop process.
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
- Invalid or reused reset confirmation has no sender side effect. The actor
  cancels the active sender only after `Ctrl-X` is delivered; a transport error
  marks the sender failed so it cannot continue dispatching under a false local
  `Cancelled` state.
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
- The GRBL encoder always emits `$J=G91 G21` with exactly one of X/Y/Z. Its
  finite technical envelope is `0.01..100000 mm` and `10..100000 mm/min`; this
  prevents malformed or unbounded IPC values but is not operational permission.
- The command actor independently limits distance to the smaller of the
  selected machine's `maxJogDistanceMm` and selected-axis travel. It limits feed
  to that axis' inspected `$110/$111/$112` value. UI values cannot widen either
  backend boundary.
- A successful `ok` means GRBL accepted the jog for execution; periodic status
  remains authoritative for `Jog` and final position. The jog pad never receives
  or stores its actor-local lease.
- The operator jog pad uses one higher-level actor request per click. That
  request performs status, Inspector, readiness, lease issue, lease consumption,
  and one typed step without exposing the authorization to React. Motion Deck
  provides three scale-aware presets and explicit distance/feed controls. The
  per-machine distance limit defaults to `50 mm` and may be configured from
  `0.01 mm` through the machine's largest axis.
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
- Runtime overrides are domain enums, not caller-selected bytes. Feed accepts
  reset, `+/-10`, and `+/-1`; rapid accepts only `100/50/25`; spindle accepts
  reset, `+/-10`, and `+/-1`. The controller maps those values to GRBL 1.1
  realtime bytes and status parsing publishes the resulting `Ov:` telemetry.
  Override requests may preempt an in-flight sender read without pausing or
  acknowledging its FIFO line. They do not expose spindle start/stop.
- Plan timing is calculated before dispatch and stored as integer milliseconds
  per source line. The sender accumulates estimate only after correlated `ok`,
  uses a monotonic active timer, excludes Paused intervals, and freezes elapsed
  on Completed/Failed/Cancelled. Unknown rapid duration leaves the estimate
  explicitly incomplete. Actor ticks publish fresh timing snapshots even while
  one command response is delayed.
- Sender failures are typed at the controller boundary. GRBL error/alarm/reset,
  timeout, disconnect, transport, unsafe-state, and internal faults retain the
  exact source line and command after buffered state is cleared.
- Plan construction injects two immutable epilogue lines, `M5` and `M9`, after
  the last source command and before an optional M2/M30. They use a distinct
  `SafetyEpilogue` kind and cannot be changed by UI, profile, plugin, or file.
  Physical completion requires those lines to be acknowledged and fresh `Idle`.
- Sender snapshots retain progress metadata directly: acknowledgement sequence,
  last accepted source line/command, age since the last `ok`, and whether the
  full shutdown tail was accepted. All values are O(1); no snapshot walks the
  plan or response history.
- Program blocks are tagged on the wire with a Millo-owned `N<source-line>`.
  Existing file line numbers are replaced after checksum validation, while the
  operator-facing command remains unchanged. RX accounting includes the wire
  prefix. When GRBL exposes `Ln:`, the actor records it separately from `ok` as
  evidence of the block physically executing; out-of-range values are ignored.
- The parser retains native-only modal checkpoints immediately before every
  executable block: XYZ entry position, units, distance/arc/feed modes, plane,
  WCS, feed, tool, spindle mode and speed. These checkpoints are not serialized
  to the webview and will be consumed only by the guarded recovery planner.
- The sender owns only the immutable plan plus a `VecDeque` bounded by reported
  RX capacity. It never creates one UI object or response future per source
  line. A 100,000-line regression checks byte bounds and peak in-flight depth.
- Stream-control syntax is interpreted before a plan exists. A final decimal
  `*checksum` is XOR-validated against the exact pre-separator source bytes; a
  mismatch, malformed value, duplicate separator, or checksummed block without
  a leading `N` is a parser error. The verified checksum is not sent to GRBL.
- A leading `/` marks an optional block. Block Delete is applied inside the
  parser so skipped modal commands cannot affect later preview geometry. The UI
  requests a complete Rust reparse when this option changes.
- `M0` is an unconditional empty-FIFO host pause. `M1` is either the same pause
  or is omitted, according to Optional Stop. Both must be isolated from other
  behavior so the host barrier is unambiguous.
- Optional Stop and Block Delete travel together through policy, preflight,
  operator confirmation, program fingerprint, and the atomically consumed
  first-cut lease. Sender code cannot reinterpret them after authorization.

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
- Loading plugins are also registered with their resource scope. An unload that
  races asynchronous activation closes capabilities and removes UI immediately;
  a late activation result runs its deactivation handler and is rejected instead
  of becoming active after the operator already removed it.
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

1. Create and review a bounded `20 x 20 mm` square `.nc` fixture with explicit
   `G21 G90 G94 G17`, conservative feed, and safe Z.
2. Perform an operator-confirmed Air run from that file with no tool and manual
   spindle power off; measure Hold response and verify final position plus Idle.
3. Only after that evidence, prepare a shallow engraving file and complete a new
   Cutting authorization. Keep probing unavailable until sensor hardware exists.

Three.js is now isolated behind the program preview adapter and a lazy bundle.
Ant Design remains absent until a workflow needs its component contracts rather
than adding it as general UI weight.
