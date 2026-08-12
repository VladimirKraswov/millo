# Millo

Millo is a modular CNC controller application with a Rust core and a
TypeScript user interface. The project is being built in small vertical slices;
each slice must work end to end and remain testable without a desktop window or
physical machine.

The name is pronounced "Mee-lo" ("Милло") and keeps a light association with
milling without tying the application to one machine type.

The current slices form this path:

```text
Serial / Mock -> command arbiter -> GRBL lifecycle/parser -> typed Tauri IPC -> React
File source -> millo-gcode parser -> immutable program DTO -> Three.js preview
File source -> intent-aware policy -> one-use authorization -> bounded GRBL sender
Physical run -> bounded RX FIFO -> ok/error correlation -> fresh Idle completion
```

The command arbiter now owns the active transport, periodic status polling, and
all controller requests. The controller handles bounded response timeouts,
reset banners, persistent alarm state, and automatic reconnection after repeated
communication failures.

The desktop app discovers native serial ports and always synchronizes `$I`,
`$$`, `$G`, and `$#` immediately after connection. A known controller is bound
to its persistent machine profile by a stored fingerprint. An unknown serial
controller opens onboarding with firmware-backed travel already filled from
`$130/$131/$132`; motion remains blocked until that binding is complete.
`millo-profile` owns local facts that firmware cannot prove: the machine name,
spindle workflow, and declared homing, limits, probe, and emergency-stop
hardware. It never infers a physical probe or emergency stop from firmware.

The controller is the source of truth for every value reported by `$$`.
`millo-settings` catalogs known GRBL 1.1 settings, retains unknown firmware
keys, and stores one bounded per-machine JSON archive. Each connection creates
an immutable session baseline. Debounced edits are serialized by the Rust actor,
compare the expected old value against a fresh `$$`, write one `$n=value`, then
repeat status and the complete Inspector before the UI shows the value as
saved. Rollback always targets the connection baseline; reconnect archives the
old baseline and makes the controller's newly observed state the next baseline.

Device Inspector displays parsed firmware, `[OPT]` controller capabilities,
settings, modal state, and coordinate parameters. Status parsing also publishes
typed planner/RX availability, feed/rapid/spindle overrides, input pins,
accessories, and line number when GRBL reports them. A separate Rust readiness
policy evaluates the inspected values against the selected profile. The desktop
API exposes typed operations and a policy-approved file sender, never an
arbitrary raw-line endpoint. Mock GRBL remains available for development and
lifecycle tests without hardware.

The first safety controls are now available without opening a G-code endpoint.
Feed Hold sends the GRBL realtime `!` byte when the controller reports active
motion. Soft Reset sends `Ctrl-X` only after a short-lived actor-issued challenge
is confirmed. The jog UI asks for one explicit readiness decision and expands it
into the three typed physical facts required by test-jog preflight. The actor
runs a fresh Inspector transaction and can issue a 15-second single-use backend
authorization. The low-level typed motion use case consumes that authorization
inside the same Rust actor and emits one `$J=G91 G21` step on exactly one XYZ
axis. The operator chooses distance and feed in the Motion Deck. A machine-local
maximum distance defaults to `50 mm`, is editable from `0.01 mm` through the
largest configured axis, and can represent desktop and multi-meter routers. The
actor clamps each request again to the selected axis travel and to that axis'
live GRBL `$110/$111/$112` maximum rate. Every attempt consumes its lease before
writing; another step requires another full preflight. GRBL Jog Cancel (`0x85`)
is a separate named safety action.
Physical smoke tests have now disabled profile-inconsistent `$21/$22`, verified
the persisted values, and completed separate X, Y, and Z `+0.100 mm` steps at
`10 mm/min`. Every run returned to `Idle`, and only its selected coordinate
changed.

The first operator jog pad is a separate feature module. Its Motion Deck offers
precision, positioning, and machine-scaled traverse presets plus explicit
distance and feed controls. Every press executes a new status, Inspector,
readiness, and one-use authorization cycle inside the Rust actor. React reaches
it through a platform-neutral `MachineCommandGateway`, establishing the same
capability boundary planned for plugins.

Manual work-zero controls are another narrow typed use case. Zero X, Y, and Z
are available only for a connected, stable `Idle` controller after an explicit
operator confirmation. The actor reads a fresh status and `$G`, maps the active
G54-G59 system to the matching `G10 L20 Pn` command, reads `$#`, and checks both
the selected offset and final work position before reporting success. React
cannot choose `Pn`, format a line, or reuse the confirmation. This operation has
been verified against Mock GRBL only; no work-zero write was sent to the physical
machine in this slice. The probe is not installed or connected, so probing and
heightmap motion remain unavailable.

The Program workbench loads `.nc`, `.ngc`, `.gcode`, `.tap`, and `.cnc` files up
to 2 MB through a separate `ProgramGateway`. Rust parses compact words,
comments, metric/imperial and absolute/incremental modes, G93/G94 feed modes,
linear motion, dwell, and circular/helical motion in G17/G18/G19 into an
immutable millimetre-based program model. IJK and R arcs include full circles;
GRBL-compatible full circles still require an explicit XYZ target word. Modal
conflicts and words used outside their GRBL context fail closed.
Standalone `%` program delimiters are retained as non-executable source lines.
Warnings retain source line numbers; spindle activation, tool change, probing,
machine-coordinate motion, malformed geometry, and unsupported commands fail
the dry-run gate. Feed and dwell time is estimated per segment. A program with
rapid moves is explicitly marked as a lower-bound estimate because controller
acceleration and rapid limits are machine-specific. For
parser-clean programs, Tauri reparses the original source and `millo-dry-run`
builds an opaque plan with an `M5/M9` safety preamble. `millo-sender` permits
only a bounded GRBL RX window and advances only after correlated FIFO `ok`
responses; `error`, `ALARM`, disconnect, reset, timeout, or invalid controller
state stops the run.
The same state machine serves Mock dry runs and authorized serial runs. A lazily
loaded Three.js adapter renders rapid and
cutting geometry from a pure read model with top/isometric views. Loading and
preview have no access to the command actor, serial transport, or machine
capabilities; only the separate policy can mint sender lines.

The diagnostics rail now exposes virtualized Program Lines and Warnings tabs.
Selecting a source line keeps the immutable parser DTO unchanged and updates a
dedicated Three.js overlay: matching motion is isolated in a bright layer while
the rest of the toolpath is dimmed. Non-motion and warning lines remain
selectable and explicitly report that they have no preview geometry. The table
renders only a small overscanned window, so the 200,000-line parser limit does
not become 200,000 DOM nodes.

The connection panel also opens a persistent structured event journal. It
correlates controller lifecycle, preflight, safety actions, sender source lines,
GRBL failures, Hold/Reset/disconnect, and persistence health by session and
sequence. The viewer colors severity, filters category/level/text, expands JSON
details, and exports through a native save dialog as readable `.log` or machine-
processable `.jsonl`. Debug traffic is hidden by default. The bounded audit
writer is asynchronous and cannot authorize motion or block the command actor.

On an active serial target, Program exposes intent-aware real-run preflight.
Tauri reparses the retained source, and the command actor performs a fresh
`? -> $I/$$/$G/$# -> ?` transaction before `millo-run` combines the strict
execution policy with motion-critical hardware readiness. `Air run` rejects
spindle activation and speed words; `Cutting` permits standard `M3/M4/S` while
the operator separately confirms the physical spindle workflow. Coolant,
probing, coordinate mutation, and machine/reference-coordinate motion remain
blocked until their own hardware-aware workflows exist. The report
links blockers to exact source lines and keeps unhomed travel, manual spindle,
and physical setup visible as cautions. A clear report opens one mode-specific
readiness decision; its disclosure lists every physical fact behind that
decision. Air run still requires a removed tool and stopped spindle; cutting
still requires secured stock and tool plus a running manually controlled
spindle. Both still require verified XYZ work zero, safe Z, clear path, and
immediately reachable power control. Submission does not trust the displayed
report: one UI action repeats the complete serial preflight, issues a 30-second
program/position-bound lease, reparses the source, refreshes status, and
atomically consumes the lease before the first line can be dispatched. The run contract also
requires explicit `G21`, `G90`, either `G93` or `G94`, and an explicit
`G17/G18/G19` before arcs, so preview cannot silently rely on ambient controller
modes.

Cutting programs may contain an isolated `M6`. Millo sends and acknowledges its
bounded `Tn` selection, drains the sender FIFO, and then stops at a host-only
tool-change barrier; `M6` itself never reaches GRBL. The operator dialog is
bound to the exact source line and tool. One readiness decision expands into
the unchanged typed facts for replacement tool, Z zero, safe Z, remaining path,
running manual spindle, and reachable power. The actor then repeats fresh `Idle`, Inspector, G54-G59, and
final `Idle` verification. Ordinary Resume cannot bypass this workflow, and
Air runs continue to reject `M6`.

The production serial sender derives its usable window as `reported RX - 1`
from the authorization's fresh `[OPT]` inspection, falls back to 127 bytes,
accounts for each newline, and correlates every FIFO terminal response with its
source line.
Each immutable plan also carries per-line timing in integer milliseconds.
Sender snapshots publish active elapsed time plus completed/remaining/total
estimates. Hold time is excluded and terminal snapshots are frozen. Rapid moves
and other unknown timing keep a visible lower-bound flag instead of presenting
false precision.
Program-response reads are sliced into short bounded waits, so actor requests
such as Feed Hold and Soft Reset preempt a delayed `ok`. Periodic realtime `?`
continues during an active FIFO; interleaved status frames update position,
buffer, overrides, pins, and accessories without consuming a command
acknowledgement.
Feed, rapid, and spindle overrides are exposed as typed actor/Tauri operations
and map only to GRBL 1.1 realtime bytes. They remain responsive while a line is
in flight and are verified through the parsed `Ov:` status field. This surface
cannot start a spindle, submit a line, or bypass sender authorization.
`M0` is always an acknowledged program barrier. `M1` becomes a barrier only
when Optional Stop is enabled; otherwise it is omitted locally. Leading `/`
blocks are conditional on Block Delete. Changing Block Delete reparses the
original source in Rust, so preview geometry, modal state, bounds, preflight,
fingerprint, authorization, and sender plan describe the same program. Decimal
XOR checksums are verified against untouched source bytes before normalization,
then removed because GRBL 1.1 does not consume that transport syntax. Corrupt
or ambiguous checksums fail closed. `M2/M30` end dispatch. After the final `ok`,
physical runs enter `Draining` and complete only after a fresh GRBL
`Idle` status. Feed Hold uses realtime `!`, Resume uses `~` when GRBL reports
Hold, and an operator stop remains challenge-confirmed. A physical `error`,
`ALARM`, response timeout, or write failure automatically sends Hold then Soft
Reset so already-buffered commands cannot continue. Polling failure, reset
banner, or disconnect also fails closed. The sender is available only for an
active, profile-bound serial target and has no plugin or raw-command entry point.
Every immutable plan adds a non-configurable `M5`, `M9` shutdown tail before
its deferred end command. Sender snapshots expose the last correlated `ok`, its
source line, acknowledgement age, a monotonic progress sequence, and whether
both shutdown commands were accepted. Snapshot work is constant-time; a
100,000-line regression completes with only the bounded RX FIFO in flight.
Each loaded plan also receives a stable process-local run sequence.
`millo-journal` records a bounded 100-run history at start, state transitions,
throttled progress checkpoints, and terminal state. Its temp/backup JSON keeps
the preceding valid checkpoint, while failed/cancelled entries are explicitly
diagnostic and cannot themselves be used as a resume lease.

Physical Air/Cut Start now has a durable commit barrier. The actor prepares the
sender without dispatching a line; Tauri writes the exact source, SHA-256,
machine/profile identity, execution options, run sequence, and initial position
to `active-program-recovery.json` with file and directory `fsync`; only then may
the actor release the first G-code block. While running, Millo persists GRBL's
optional physical `Ln:` evidence independently from buffered `ok` responses.
After a crash, link loss, or power failure, `millo-recovery` verifies the source
and controller and asks which electrical path remained alive. A proven host-only
or controller interruption may rewind to a preceding clearance rapid using the
last physical `Ln:`. Loss of motion power, uncertain continuity, or firmware
without `Ln:` permits only a full restart from the beginning after XYZ reference
and work zero are restored. Both paths create a new M5/M9-prefixed program with
a reviewed Safe Z; neither moves automatically. The generated file must pass
preview, GRBL Check, preflight, and a new one-use authorization.

An active physical sender treats the first stream, transport, status, or
realtime I/O failure as a terminal interruption, closes the controller session,
and requires an explicit reconnect. It cannot silently resume after USB returns.
A USB-powered GRBL can,
however, keep reporting internal motion while separate motor power is absent;
without a wired power/position sensor software cannot detect that condition.
Such a run may finish logically without moving, so automatic recovery cannot be
offered until a power/position signal is wired; the original file must then be
re-run after inspection. For detected interruptions, the recovery dialog
defaults to the full-restart policy. An unresolved record blocks an unrelated
physical job and can be replaced only by its exact prepared recovery program or
by explicit dismissal.

A separate serial-only Check run validates an approved file through GRBL's
typed `$C` mode without executing motion. The actor enters only from fresh
`Idle`, verifies `Check`, correlates one outstanding line at a time, and always
toggles back to verified `Idle` after completion, parser error, or cancellation.
The complex multi-plane fixture has passed 25/25 lines on the physical GRBL
1.1f controller. Its first attempt exposed and then regression-locked GRBL's
requirement for an explicit axis target on a full-circle arc.
Check uses Cutting grammar, so production `M3/M4/S` syntax can be firmware-
validated while Air policy continues to reject it. A second physical fixture
completed 27/27 sender steps, including validation-only M0/M1, returned to Idle,
and produced a certificate accepted by fresh Cutting preflight. M2/M30 is
host-validated during Check and is still sent only by physical Air/Cut runs.
Program workspace exposes this lifecycle through typed `GRBL Check`; Tauri
reparses the retained source before the actor enters `$C`. Optional-block and
checksum semantics are covered by the same Check path; metadata-only `O`
program headers are retained but never sent.
A completed Check mints a 15-minute certificate only after the controller has
returned to verified `Idle`. Cutting preflight requires that certificate to
match the exact source fingerprint, Optional Stop/Block Delete options, reset
count, and reconnect count. Air run remains available without it as the
spindle-off physical validation path.

The first dense physical Cutting fixture, `millo-solar-guilloche.nc`, completed
1045/1045 sender commands in 226.5 seconds after a same-session 1045/1045 Check.
Its earlier line-44 false timeout led to a silence-based response watchdog:
realtime status proves controller liveness without pretending to acknowledge a
G-code line, while actual loss of responses still fails closed. The verified
final state was `Idle` at G54 WPos X30/Y30/Z3.

The execution-core differences from the Candle reference, the problem each one
solves, and the remaining deliberate capability gates are maintained in
[Millo sender compared with Candle](docs/CANDLE_SENDER_COMPARISON.md). The
current command matrix and physical evidence are in
[GRBL execution audit](docs/GRBL_EXECUTION_AUDIT.md). Issue-driven reliability
work and explicit limitations are recorded in
[Sender hardening](docs/SENDER_HARDENING.md).

UI composition now starts with a generic `ExtensionRegistry`. Jog Pad is the
first core contribution in the named `control.machine` slot; Work Zero occupies
the separate `control.coordinates` slot. Contributions have stable IDs, owners,
ordering, replacement declarations, and deterministic unload. The first
in-memory plugin host validates a versioned manifest before activation and
intersects required/optional capabilities with explicit grants and host support.
Its built-in test plugin can replace Jog Pad and unload cleanly without loading
external code. `ui.contribute` and guarded `machine.jog` are the first implemented
host capabilities. `machine.read` now exposes detached, deeply frozen controller
snapshots and tracked subscriptions when the host wires a state source. Unload
and failed activation remove those subscriptions and close retained proxies. Job
creation remains a declared future contract.

The application now creates one `PluginHost` bootstrap containing the UI
registry, machine snapshot store, and in-memory loader. React observes that store
through `useSyncExternalStore`; the initial controller query, live Tauri
`machine-state` events, and typed command results all publish into the same
source. Bootstrap registers core UI but activates no plugins and performs no
external code loading.

## Run

Requirements: Node.js 20+, Rust 1.85+, and the platform prerequisites for
Tauri 2.

```bash
npm install
npm run verify
npm run tauri dev
```

The Vite-only preview (`npm run dev`) renders the interface, but controller
commands are enabled only inside Tauri.

The operator shell uses progressive disclosure: connection, controller state,
coordinates, preview, run actions, Hold/Reset, and jog stay primary. Port tuning,
passed readiness evidence, G-code rows, optional stream semantics, lifecycle
metrics, and Mock scenarios open only on demand. Parser warnings and failed
preflight evidence open their diagnostics automatically. This hierarchy is
recorded in [ADR 0039](docs/decisions/0039-progressive-operator-shell.md).

The Mock diagnostics disclosure can inject reset, alarm, timeout, and link-drop scenarios. Alarm
remains active until `Clear alarm`; two consecutive silent polls exercise the
automatic recovery path.

The first physical-program candidate is the ordinary file
`fixtures/programs/air-square-20mm.nc`. Its repeatable read-only and confirmed
hardware procedures are documented in [Testing](docs/TESTING.md); the confirmed
path refuses to start unless the observed XYZ work position is at zero.

To use hardware, choose a discovered serial device, select its baud rate
(`115200` is the common GRBL 1.1 default), and connect. Refresh re-runs native
port discovery. Port access is controlled by the operating system; close other
serial monitors before connecting.

On macOS, the same USB interface is normally exposed as both `/dev/cu.*` and
`/dev/tty.*`. Millo collapses that pair and keeps the `/dev/cu.*` callout path,
which is the appropriate endpoint for initiating a controller connection.

`Только вероятные GRBL` is enabled by default. It keeps Mock GRBL and USB ports
whose metadata or vendor ID resembles common GRBL/FluidNC controllers and
USB-UART bridges. Disable it to inspect every serial port. This is discovery
filtering, not device authentication: Millo confirms the protocol only after a
successful GRBL status exchange.

## Workspace

| Package | Responsibility |
| --- | --- |
| `millo-audit` | Bounded structured JSONL diagnostics, rotation, tail, and export |
| `millo-domain` | Stable machine and controller types |
| `millo-gcode` | Immutable G-code program, warnings, parser, and preview geometry |
| `millo-journal` | Bounded crash-diagnostic history with throttled atomic JSON checkpoints |
| `millo-recovery` | Crash-safe interrupted-job evidence and conservative restart program builder |
| `millo-grbl` | GRBL wire-format parsing and encoding |
| `millo-transport` | Controller-independent I/O contract |
| `millo-mock` | Deterministic virtual machine for tests |
| `millo-profile` | Validated machine profiles, GRBL-derived drafts, and JSON persistence |
| `millo-settings` | GRBL settings catalog, validated writes, session baselines, and per-machine revisions |
| `millo-serial` | Native asynchronous serial discovery and byte/line I/O |
| `millo-storage` | Synced temp/backup replacement and crash recovery shared by local JSON stores |
| `millo-controller` | Connection lifecycle and state orchestration |
| `millo-dry-run` | Fail-closed program policy and opaque approved plans |
| `millo-command` | Single-owner command actor, polling, and response arbitration |
| `millo-readiness` | Hardware-profile policy and guarded test-jog readiness |
| `millo-run` | Intent-aware preflight, operator checklist, and one-use program-run lease |

The current cross-layer review, closed findings, dependency evidence, and
remaining release boundaries are recorded in [`docs/CODE_AUDIT.md`](docs/CODE_AUDIT.md).
| `millo-safety` | Reset challenges and short-lived test-jog authorization |
| `millo-sender` | Bounded GRBL RX/FIFO sender with Mock, air-run, and cutting modes |
| `millo-desktop` | Thin Tauri command/event adapter |

See [Architecture](docs/ARCHITECTURE.md), the decisions for the
[modular core](docs/decisions/0001-modular-core.md) and
[controller lifecycle](docs/decisions/0002-controller-lifecycle.md), plus the
[project naming decision](docs/decisions/0003-project-name-millo.md) and
[native serial boundary](docs/decisions/0004-native-serial-transport.md), and
[command arbiter](docs/decisions/0005-command-arbiter-device-inspector.md), plus
[hardware readiness](docs/decisions/0006-hardware-readiness.md) and
[realtime safety controls](docs/decisions/0007-realtime-safety-controls.md), then
the [guarded step jog](docs/decisions/0008-guarded-step-jog.md),
[machine-scaled Motion Deck](docs/decisions/0040-machine-scaled-motion-deck.md), and
[verified unhomed configuration](docs/decisions/0009-unhomed-controller-configuration.md),
then the [extension host boundary](docs/decisions/0010-extension-host-boundaries.md)
and [versioned plugin manifest](docs/decisions/0011-versioned-plugin-manifest.md),
followed by the
[read-only machine capability](docs/decisions/0012-machine-read-capability.md)
and [PluginHost bootstrap](docs/decisions/0013-plugin-host-bootstrap.md), then the
[guarded work-zero transaction](docs/decisions/0014-guarded-work-zero.md) and
[G-code program boundary](docs/decisions/0015-gcode-program-boundary.md), then
the [Mock-only bounded sender](docs/decisions/0016-mock-dry-run-sender.md) and
[immutable line selection](docs/decisions/0017-program-line-selection.md), then
the [serial real-run preflight](docs/decisions/0018-real-run-preflight.md), and
the [persistent machine-profile boundary](docs/decisions/0019-machine-profiles.md),
the [controller settings and identity boundary](docs/decisions/0020-controller-settings-sync.md),
and the [first-cut authorization boundary](docs/decisions/0021-first-cut-authorization.md).
The test-only sender promotion is recorded in
[ADR 0022](docs/decisions/0022-serial-sender-fixtures.md).
The production file sender is recorded in
[ADR 0023](docs/decisions/0023-authorized-file-program-run.md).
Its bounded receive-buffer streaming contract is recorded in
[ADR 0025](docs/decisions/0025-grbl-rx-buffer-streaming.md).
Full modal arc parsing and conservative time estimation are recorded in
[ADR 0026](docs/decisions/0026-modal-parser-and-time-estimation.md).
The verified GRBL Check-run lifecycle is recorded in
[ADR 0027](docs/decisions/0027-grbl-check-run.md).
Responsive interleaved sender I/O is recorded in
[ADR 0028](docs/decisions/0028-responsive-sender-io.md).
Host-managed tool change is recorded in
[ADR 0034](docs/decisions/0034-host-managed-tool-change.md).
Certified Check evidence and the bounded run journal are recorded in
[ADR 0037](docs/decisions/0037-certified-check-and-run-journal.md).
Crash-safe guided restart is recorded in
[ADR 0038](docs/decisions/0038-guided-power-loss-recovery.md).
The progressive operator shell and compact confirmation mapping are recorded in
[ADR 0039](docs/decisions/0039-progressive-operator-shell.md).
The
required verification workflow is recorded in [Testing](docs/TESTING.md); the
known first-machine configuration is in [Hardware target](docs/HARDWARE_TARGET.md).

## Reference policy

Candle is used as a behavioral reference and a source of compatibility
scenarios. Millo does not copy Candle modules or mirror its Qt architecture.
Observed behavior is first captured as a fixture or test and then implemented
against Millo's own domain boundaries.

The project license has not been selected yet.
