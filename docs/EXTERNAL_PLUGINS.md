# External plugins and macros

This page is the operator/security overview. Authors should use the complete
[Plugin development guide](PLUGIN_DEVELOPMENT.md), which also explains the
separate trusted TypeScript SDK, package schema, lifecycle and test contract.

Millo accepts external source packages with the `.millo-plugin` extension. The
format is JSON so the manifest, UI declaration, requested authority, and Rhai
source can be inspected before installation. Import never enables a package.

## Operator flow

1. Open the puzzle icon in the top bar.
2. Import a `.millo-plugin` file or create a local macro.
3. Review its source, commands, requested capabilities, and short digest.
4. Enable it. All required capabilities and the selected optional capabilities
   are granted only to that exact SHA-256 digest.
5. Run its command from `Создать` or the compact macro panel beside machine
   controls. A machine action has one concise readiness confirmation.

Editing or reimporting a package changes its digest, disables it, and clears all
previous grants. Packages can be exported for review and sharing. Bundled
packages cannot be edited or deleted, but they can be disabled.

## Runtime boundary

Rhai scripts receive `command`, validated `input`, and either a detached machine
snapshot or `null` depending on the `machine.read` grant. A package defines:

```rhai
fn run(command, input, machine) {
  return #{
    kind: "notice",
    title: "Macro result",
    message: "Ready",
    tone: "success"
  };
}
```

One invocation returns exactly one action:

- `createProgram { sourceName, source }`
- `jog { axis, distanceMm, feedMmPerMin }`
- `setZero { axis }`
- `returnZero { axis, feedMmPerMin }`
- `rawCommand { command }`
- `notice { title, message, tone }`

`createProgram` is parsed by `millo-gcode` and enters the normal Program flow.
It is not sent to GRBL. Machine actions require their capability, a fresh UI
confirmation, a connected profile, and all checks already enforced by the Rust
command actor. `rawCommand` additionally requires the explicitly reviewed
`machine.commands` grant and globally disabled safe command mode. A script never
receives a serial handle, response reader, sender handle, or realtime-byte API.

The runtime bounds operations, call depth, expression depth, strings, arrays,
maps, source size, commands, fields, and generated G-code. No filesystem,
network, serial, sender, Tauri, DOM, module import, or dynamic `eval` functions
are registered. This is an in-process language sandbox, not OS process
isolation; only install source you are willing to review.

## Package structure

The package and API versions are currently `1`. A command selects either
`workspaceTools` or `machinePanel`, an allowlisted Lucide icon name, typed input
fields, and capabilities needed to display it. Unknown icons receive a neutral
fallback rather than loading an external asset.

Capabilities:

- `ui.contribute`
- `machine.read`
- `machine.jog`
- `machine.coordinates`
- `machine.commands`
- `jobs.create`

`machine.commands` is the expert escape hatch, not ordinary plugin authority.
It accepts one printable ASCII line up to 255 bytes, rejects multiline input and
typed realtime commands, runs only through the single actor in Idle/Alarm, and
invalidates stale Check, authorization, Z-datum, homing and envelope evidence.
It remains blocked while `Настройки -> Приложение -> Безопасный режим команд`
is enabled, which is the default.

The manager's `Новый макрос` template exposes optional machine/job capabilities
but grants none of them until selected and enabled. Parameterized commands or
multiple command surfaces can be authored by exporting the template, editing
its JSON declaration, and reimporting it.

Every package must require `ui.contribute`. A returned action's capability must
also appear in that command's `requiredCapabilities`; a broad manifest grant
alone cannot turn an innocuous-looking command into a machine action. Generated
job names must be plain file names without directories.

The persistent store publishes install/configure/delete mutations only after an
atomic disk write succeeds, restores a corrupt primary from its preceding
backup, and bounds package count and total storage size. Configuration and
execution are serialized, so a disable/update cannot race an already validated
machine action.

## Bundled macros

`millo.operator-macros` is enabled by default:

- `Проверить границу` creates a spindle/coolant-off rectangle for preview,
  GRBL Check, or a separately authorized Air run.
- `Поднять Z` issues one guarded positive Z jog with bounded distance and feed.
- `Вернуть Z в ноль` uses the typed absolute work-zero return.
- `Z0 здесь` writes and verifies the current G54-G59 Z zero through `$#`.

Z-probe and center-finder are intentionally not emulated with raw G-code. They
need a future typed `machine.probe` action, installed-probe profile evidence,
contact/timeout limits, and dedicated fixtures. This keeps an imported example
from bypassing the same hardware policy as core UI.

## Verification

```bash
cargo test -p millo-script
npm run test:ui
npm run verify
```
