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
```

The command arbiter now owns the active transport, periodic status polling, and
all controller requests. The controller handles bounded response timeouts,
reset banners, persistent alarm state, and automatic reconnection after repeated
communication failures.

The desktop app discovers native serial ports and can connect to a GRBL
controller at a selected baud rate. Device Inspector automatically reads `$I`,
`$$`, `$G`, and `$#`, then displays parsed firmware, settings, modal state, and
coordinate parameters. A separate Rust readiness policy evaluates those values
against the first-machine profile: XYZ motion, manual spindle, no homing, no
limit switches, and no physical emergency stop. It reports blockers and
cautions for a guarded test jog; probing remains locked. No arbitrary line,
general motion, or spindle command is exposed by the desktop API. Mock GRBL
remains the default, so development and lifecycle tests do not require hardware.

The first safety controls are now available without opening a G-code endpoint.
Feed Hold sends the GRBL realtime `!` byte when the controller reports active
motion. Soft Reset sends `Ctrl-X` only after a short-lived actor-issued challenge
is confirmed. Test-jog preflight requires three physical operator confirmations,
runs a fresh Inspector transaction, and can issue a 15-second single-use backend
authorization. The only motion endpoint consumes that authorization inside the
same Rust actor and emits one `$J=G91 G21` step on exactly one XYZ axis. Distance
is limited to `0.01..1.00 mm` and feed to `10..100 mm/min` in the backend. Every
attempt consumes its lease before writing; another step requires another full
preflight. GRBL Jog Cancel (`0x85`) is exposed as a separate named safety action.
The first physical smoke test has now disabled profile-inconsistent `$21/$22`,
verified the persisted values, and completed X `+0.100 mm` at `10 mm/min` while
Y/Z remained unchanged.

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
| `millo-grbl` | GRBL wire-format parsing and encoding |
| `millo-transport` | Controller-independent I/O contract |
| `millo-mock` | Deterministic virtual machine for tests |
| `millo-serial` | Native asynchronous serial discovery and byte/line I/O |
| `millo-controller` | Connection lifecycle and state orchestration |
| `millo-command` | Single-owner command actor, polling, and response arbitration |
| `millo-readiness` | Hardware-profile policy and guarded test-jog readiness |
| `millo-safety` | Reset challenges and short-lived test-jog authorization |
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
[verified unhomed configuration](docs/decisions/0009-unhomed-controller-configuration.md).
The
required verification workflow is recorded in [Testing](docs/TESTING.md); the
known first-machine configuration is in [Hardware target](docs/HARDWARE_TARGET.md).

## Reference policy

Candle is used as a behavioral reference and a source of compatibility
scenarios. Millo does not copy Candle modules or mirror its Qt architecture.
Observed behavior is first captured as a fixture or test and then implemented
against Millo's own domain boundaries.

The project license has not been selected yet.
