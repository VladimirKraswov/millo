# Gantryon

Gantryon is a modular CNC controller application with a Rust core and a
TypeScript user interface. The project is being built in small vertical slices;
each slice must work end to end and remain testable without a desktop window or
physical machine.

The first slice is intentionally narrow:

```text
Mock transport -> GRBL status parser -> controller state -> Tauri event -> React
```

## Run

Requirements: Node.js 20+, Rust 1.85+, and the platform prerequisites for
Tauri 2.

```bash
npm install
npm test
npm run tauri dev
```

The Vite-only preview (`npm run dev`) renders the interface, but controller
commands are enabled only inside Tauri.

## Workspace

| Package | Responsibility |
| --- | --- |
| `gantryon-domain` | Stable machine and controller types |
| `gantryon-grbl` | GRBL wire-format parsing and encoding |
| `gantryon-transport` | Controller-independent I/O contract |
| `gantryon-mock` | Deterministic virtual machine for tests |
| `gantryon-controller` | Connection lifecycle and state orchestration |
| `gantryon-desktop` | Thin Tauri command/event adapter |

See [Architecture](docs/ARCHITECTURE.md) and the project
[decisions](docs/decisions/0001-modular-core.md).

## Reference policy

Candle is used as a behavioral reference and a source of compatibility
scenarios. Gantryon does not copy Candle modules or mirror its Qt architecture.
Observed behavior is first captured as a fixture or test and then implemented
against Gantryon's own domain boundaries.

The project license has not been selected yet.
