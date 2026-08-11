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
is confirmed. Test-jog preflight requires three physical operator confirmations,
runs a fresh Inspector transaction, and can issue a 15-second single-use backend
authorization. The low-level typed motion use case consumes that authorization
inside the same Rust actor and emits one `$J=G91 G21` step on exactly one XYZ
axis. Distance is limited to `0.01..1.00 mm` and feed to `10..100 mm/min` in the
backend. Every attempt consumes its lease before writing; another step requires
another full preflight. GRBL Jog Cancel (`0x85`) is a separate named safety
action.
Physical smoke tests have now disabled profile-inconsistent `$21/$22`, verified
the persisted values, and completed separate X, Y, and Z `+0.100 mm` steps at
`10 mm/min`. Every run returned to `Idle`, and only its selected coordinate
changed.

The first operator jog pad is a separate feature module. It exposes only fixed
`0.01` and `0.10 mm` XYZ steps at `10 mm/min`; every press executes a new status,
Inspector, readiness, and one-use authorization cycle inside the Rust actor.
React reaches it through a platform-neutral `MachineCommandGateway`, establishing
the same capability boundary planned for plugins.

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
comments, metric/imperial and absolute/incremental modes, linear motion, and XY
arcs into an immutable millimetre-based program model. Standalone `%` program
delimiters are retained as non-executable source lines. Warnings retain source
line numbers; spindle activation, tool change, probing, machine-coordinate
motion, malformed geometry, and unsupported commands fail the dry-run gate. For
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

On an active serial target, Program exposes intent-aware real-run preflight.
Tauri reparses the retained source, and the command actor performs a fresh
`? -> $I/$$/$G/$# -> ?` transaction before `millo-run` combines the strict
execution policy with motion-critical hardware readiness. `Air run` rejects
spindle activation and speed words; `Cutting` permits standard `M3/M4/S` while
the operator separately confirms the physical spindle workflow. Coolant,
probing, M6, coordinate mutation, and machine/reference-coordinate motion remain
blocked until their own hardware-aware workflows exist. The report
links blockers to exact source lines and keeps unhomed travel, manual spindle,
and physical setup visible as cautions. A clear report opens a separate
mode-specific checklist. Air run requires a removed tool and stopped spindle;
cutting requires secured stock and tool plus a running manually controlled
spindle. Both require verified XYZ work zero, safe Z, clear path, and immediately
reachable power control. Submission does not trust the displayed report: the
actor repeats the complete serial preflight and then issues a 30-second,
program-bound, position-bound, single-use lease. A separate Start action reparses
the original file, refreshes status, and atomically consumes that lease before
the first line can be dispatched. The run contract also
requires explicit `G21`, `G90`, `G94`, and `G17` before the motions that depend
on them, so preview cannot silently rely on ambient controller modes.

The production serial sender derives its usable window as `reported RX - 1`
from the authorization's fresh `[OPT]` inspection, falls back to 127 bytes,
accounts for each newline, and correlates every FIFO terminal response with its
source line.
`M0/M1` are acknowledged program barriers; `M2/M30` end dispatch. After the
final `ok`, physical runs enter `Draining` and complete only after a fresh GRBL
`Idle` status. Feed Hold uses realtime `!`, Resume uses `~` when GRBL reports
Hold, and an operator stop remains challenge-confirmed. A physical `error`,
`ALARM`, response timeout, or write failure automatically sends Hold then Soft
Reset so already-buffered commands cannot continue. Polling failure, reset
banner, or disconnect also fails closed. The sender is available only for an
active, profile-bound serial target and has no plugin or raw-command entry point.

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

The mock panel can inject reset, alarm, timeout, and link-drop scenarios. Alarm
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
| `millo-domain` | Stable machine and controller types |
| `millo-gcode` | Immutable G-code program, warnings, parser, and preview geometry |
| `millo-grbl` | GRBL wire-format parsing and encoding |
| `millo-transport` | Controller-independent I/O contract |
| `millo-mock` | Deterministic virtual machine for tests |
| `millo-profile` | Validated machine profiles, GRBL-derived drafts, and JSON persistence |
| `millo-settings` | GRBL settings catalog, validated writes, session baselines, and per-machine revisions |
| `millo-serial` | Native asynchronous serial discovery and byte/line I/O |
| `millo-controller` | Connection lifecycle and state orchestration |
| `millo-dry-run` | Fail-closed program policy and opaque approved plans |
| `millo-command` | Single-owner command actor, polling, and response arbitration |
| `millo-readiness` | Hardware-profile policy and guarded test-jog readiness |
| `millo-run` | Intent-aware preflight, operator checklist, and one-use program-run lease |
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
the [guarded step jog](docs/decisions/0008-guarded-step-jog.md) and
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
The
required verification workflow is recorded in [Testing](docs/TESTING.md); the
known first-machine configuration is in [Hardware target](docs/HARDWARE_TARGET.md).

## Reference policy

Candle is used as a behavioral reference and a source of compatibility
scenarios. Millo does not copy Candle modules or mirror its Qt architecture.
Observed behavior is first captured as a fixture or test and then implemented
against Millo's own domain boundaries.

The project license has not been selected yet.
