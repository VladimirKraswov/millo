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
Safe source -> dry-run policy -> bounded sender -> Mock GRBL only
Candidate source -> Rust preflight -> fresh serial Inspector -> read-only report
```

The command arbiter now owns the active transport, periodic status polling, and
all controller requests. The controller handles bounded response timeouts,
reset banners, persistent alarm state, and automatic reconnection after repeated
communication failures.

The desktop app discovers native serial ports and connects only after the
operator selects a persistent machine profile. `millo-profile` validates the
machine name, positive XYZ travel, spindle workflow, and declared homing,
limits, probe, and emergency-stop hardware before storing a versioned JSON file.
The compact header switcher keeps that safety context visible throughout the
app. A disconnected read-only detection flow can query `$I`, `$$`, `$G`, and
`$#`, prefill XYZ from `$130/$131/$132`, and retain the matching serial/baud
preset. It never infers a physical probe or emergency stop from firmware.

Device Inspector displays parsed firmware, settings, modal state, and coordinate
parameters. A separate Rust readiness policy evaluates those values against the
selected profile. No arbitrary line, general motion, or spindle command is
exposed by the desktop API. Mock GRBL remains available for development and
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
arcs into an immutable millimetre-based program model. Warnings retain source
line numbers; spindle activation, tool change, probing, machine-coordinate
motion, malformed geometry, and unsupported commands fail the dry-run gate. For
parser-clean programs, Tauri reparses the original source and `millo-dry-run`
builds an opaque plan with an `M5/M9` safety preamble. `millo-sender` permits
only one in-flight line and advances only after its correlated `ok`; `error`,
`ALARM`, disconnect, or invalid controller state stops the run. This execution
path is locked to Mock GRBL in both the Tauri adapter and command actor. Serial
hardware cannot start it. A lazily loaded Three.js adapter renders rapid and
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

On an active serial target, Program also exposes a read-only real-run preflight.
Tauri reparses the retained source, and the command actor performs a fresh
`? -> $I/$$/$G/$# -> ?` transaction before `millo-run` combines the strict
motion-only program policy with motion-critical hardware readiness. The report
links blockers to exact source lines and keeps unhomed travel, manual spindle,
and physical setup visible as cautions. It creates no authorization, sends no
program line, and exposes no serial Start action. The first-cut contract also
requires explicit `G21`, `G90`, `G94`, and `G17` before the motions that depend
on them, so preview cannot silently rely on ambient controller modes.

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
| `millo-serial` | Native asynchronous serial discovery and byte/line I/O |
| `millo-controller` | Connection lifecycle and state orchestration |
| `millo-dry-run` | Fail-closed program policy and opaque approved plans |
| `millo-command` | Single-owner command actor, polling, and response arbitration |
| `millo-readiness` | Hardware-profile policy and guarded test-jog readiness |
| `millo-run` | Read-only real-run preflight policy and operator report |
| `millo-safety` | Reset challenges and short-lived test-jog authorization |
| `millo-sender` | Bounded one-line-in-flight sender state machine |
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
the [persistent machine-profile boundary](docs/decisions/0019-machine-profiles.md).
The
required verification workflow is recorded in [Testing](docs/TESTING.md); the
known first-machine configuration is in [Hardware target](docs/HARDWARE_TARGET.md).

## Reference policy

Candle is used as a behavioral reference and a source of compatibility
scenarios. Millo does not copy Candle modules or mirror its Qt architecture.
Observed behavior is first captured as a fixture or test and then implemented
against Millo's own domain boundaries.

The project license has not been selected yet.
