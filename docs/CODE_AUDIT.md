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
- Sender journal `fsync` runs outside Tokio. Profiles, settings, and journal use
  one synced temp/backup replacement implementation with corruption recovery.
- Native serial lines are bounded to 4 KiB and incomplete EOF frames fail closed.
- Settings autosave is fenced by dialog lifecycle, controller fingerprint, and
  profile ID. Pending work is invalidated when the operator closes or switches.
- Plugin unload wins a race with asynchronous activation and immediately closes
  retained UI/read/jog capability proxies.
- Promise rejection paths in application bootstrap/profile refresh are reported.
- Production and development Tauri CSPs now allow local assets and IPC only;
  a repository check prevents accidental return to null/wildcard/eval policy.

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

- External plugin code is still deliberately unsupported. Before enabling it,
  Millo needs signature/trust policy, isolated storage/network scopes, and
  per-command capability grants; the in-memory host is not a sandbox.
- Tauri's Linux GTK warning remains upstream/transitive. Recheck RustSec on every
  Tauri upgrade and before shipping Linux packages.
- Physical motion was not needed for this review. Protocol and sender behavior
  remains covered by Mock/serial-class fixtures; the existing operator-controlled
  hardware Check/Air procedures remain the release gate for machine movement.

## Verification contract

Run `npm run verify` after every slice. Supply-chain review additionally runs
`npm audit --json` and `cargo audit`. The CSP regression is part of normal
`npm test` through `npm run test:security`.
