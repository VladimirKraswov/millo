# Code audit

Audit date: 2026-08-12.

## Scope

The review covered the Rust workspace, Tauri command adapter, native serial
transport, local persistence, React state/effects, plugin host, Three.js preview,
tests, configuration, and direct/transitive dependency reports.

## Closed findings

- Controller/program invariants return typed errors instead of panicking.
- Reset challenge validation happens before sender mutation; stale confirmation
  cannot cancel only the host while GRBL continues buffered motion.
- Connect and transport replacement fail unless the actor is disconnected.
  Partial Tauri connection setup is cleaned up before an error reaches React.
- Mock Pause/Resume cannot mutate a physical sender.
- Sender journal and recovery `fsync` run outside Tokio. Profiles, settings,
  journal, and active-job recovery use one synced temp/backup replacement
  implementation with corruption recovery. Physical Start is not committed to
  the actor until the first recovery generation is durable.
- Native serial lines are bounded to 4 KiB and incomplete EOF frames fail closed.
- Settings autosave is fenced by dialog lifecycle, controller fingerprint, and
  profile ID. Pending work is invalidated when the operator closes or switches.
- Plugin unload wins a race with asynchronous activation and immediately closes
  retained UI/read/jog capability proxies.
- External `.millo-plugin` packages now execute as bounded Rhai scripts. Their
  declarative UI is mounted only after a digest-bound capability review; source
  changes disable the package and clear every grant. The runtime exposes no
  serial, sender, Tauri, DOM, filesystem, network, module import, or dynamic
  `eval` API. Machine actions return to the typed command actor and generated
  G-code is reparsed before it reaches Program.
- Promise rejection paths in application bootstrap/profile refresh are reported.
- Production and development Tauri CSPs now allow local assets and IPC only;
  a repository check prevents accidental return to null/wildcard/eval policy.
- GRBL Check is no longer an operator dead end: active checks expose typed
  cancellation, terminal checks return to readiness, and their failures cannot
  trigger physical-run recovery lookup. A stalled-response actor regression
  verifies `$C` cleanup, fresh `Idle`, and absence of a false certificate.
- The main operator surface, dialogs, settings, line table, diagnostics, and
  accessibility labels now use one Russian operator vocabulary. GRBL/G-code
  tokens remain untranslated where changing them would reduce diagnostic value.

## Dependency evidence

- `npm audit --json`: 0 vulnerabilities across 167 dependencies.
- `cargo audit`: 0 vulnerability failures. It reports 17 allowed warnings from
  all-target transitive dependencies: GTK3/unic/proc-macro crates are
  unmaintained, and `glib 0.18.5` has `RUSTSEC-2024-0429`. The `glib` path is the
  Linux GTK backend of current Tauri 2.11.5 and is not compiled for the current
  macOS target.
- `npm outdated` reports only major-version migrations outside the current
  declared ranges: Vite 8, the React plugin 6, and TypeScript 7. They are not
  mixed into this safety refactor; each requires its own migration and fixture
  pass.

## Residual boundaries

- External native libraries, JavaScript, QtScript compatibility modules, and
  arbitrary HTML/React injection remain unsupported. Version 1 deliberately
  accepts auditable Rhai source plus declarative commands only. Sandboxed
  per-plugin storage/network capabilities and package signing can be added as
  separate APIs; they must not be inferred from script installation.
- Tauri's Linux GTK warning remains upstream/transitive. Recheck RustSec on every
  Tauri upgrade and before shipping Linux packages.
- Physical motion was not needed for this review. Protocol and sender behavior
  remains covered by Mock/serial-class fixtures; the existing operator-controlled
  hardware Check/Air procedures remain the release gate for machine movement.
- The lazily loaded Three.js preview chunk is about 534 kB minified (134 kB
  gzip), so Vite reports its default 500 kB warning. It is excluded from the
  345 kB initial application chunk; further reduction is a performance task,
  not a sender or operator-workflow blocker.

## Verification contract

Run `npm run verify` after every slice. Supply-chain review additionally runs
`npm audit --json` and `cargo audit`. The CSP regression is part of normal
`npm test` through `npm run test:security`.
