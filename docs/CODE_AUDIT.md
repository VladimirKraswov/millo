# Code audit

Audit date: 2026-08-20.

## Scope

The review covered the Rust workspace, Tauri command adapter, native serial
transport, local persistence, React state/effects, plugin host, Three.js preview,
tests, configuration, and direct/transitive dependency reports.

## Closed findings

- Homing and continuous jog no longer depend on a UI wait loop. The command
  actor owns `$H` correlation, timeout, settle-to-Idle, session invalidation,
  machine-envelope capture, bounded `$J=` lifetime, and one-shot `0x85`
  watchdog cancellation. React owns gestures only and cancels on pointer/key
  release, focus loss, visibility loss, unmount, disable, and disconnect.
- Optional A-axis limits are expressed in degrees and cannot be narrowed by a
  linear millimetre profile value. G54-G59 and machine outputs are typed,
  Idle-only and modal-verified. Undeclared flood/mist outputs fail before serial
  I/O, while legacy profiles deserialize with both capabilities disabled.

- The React composition root no longer embeds development fixture construction,
  controller Inspector rendering, coordinate readouts, or the complete transport
  panel. Those responsibilities now have narrow modules and focused render
  tests; `App.tsx` keeps state and gateway orchestration.
- The program workspace delegates Three.js presentation, line diagnostics,
  sender timing, and physical-run controls to independent components. Gateway
  availability remains an explicit fail-closed input, so extraction cannot make
  a Pause/Resume/Cancel control available without its host capability.
- Sender active/terminal classification and execution-option identity no longer
  have local copies across read models and React. Exhaustive table tests cover
  every sender state and every execution option.
- Connected + stable Idle is now one shared domain predicate per language.
  Rust readiness, safety leases, run certificates, preflight, probe startup and
  the command actor use `ControllerSnapshot::is_stable_idle()`; React Jog,
  work-zero, Safety Controls and run availability use the equivalent typed read
  model. Each layer retains its own error type without duplicating policy.
- Script-plugin request DTOs and pure axis/confirmation mappings were separated
  from the Tauri command adapter. Public command signatures are re-exported, so
  the IPC surface and generated handler names remain unchanged.
- Repeated component setup in readiness/authorization tests was replaced with
  typed builders and table cases. Distinct plugin lifecycle race tests were kept
  instead of merging unlike behavior merely to shorten a suite.

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
- First-party plugins now use one public `src/plugin-sdk` contract. An automated
  architecture test rejects direct Tauri/API and loader-internal imports from
  plugin production code. Capability grants are runtime-validated, and scoped
  `machine.coordinates` proxies now match the external plugin catalog.
- External `.millo-plugin` packages now execute as bounded Rhai scripts. Their
  declarative UI is mounted only after a digest-bound capability review; source
  changes disable the package and clear every grant. The runtime exposes no
  serial, sender, Tauri, DOM, filesystem, network, module import, or dynamic
  `eval` API. Machine actions return to the typed command actor and generated
  G-code is reparsed before it reaches Program.
- External plugin persistence is transactional. Failed disk writes cannot leave
  grants enabled only in the current process; a corrupt primary is repaired from
  `.bak`; package count/store bytes, text inputs, numeric metadata and generated
  leaf names are bounded. Command declarations must match returned action
  capabilities, and a backend execution fence closes configure/delete/run
  TOCTOU races.
- The operator console is not a raw transport exception. Safe mode defaults to
  five exact read-only requests. Persisted expert mode admits one bounded line,
  but the actor still fences operations, correlates the response, and
  invalidates stale execution evidence. Plugins additionally need the
  digest-bound `machine.commands` grant and operator confirmation.
- Plugin render failures are isolated per contribution. Heightmap checkpoint,
  lock and Tauri event-emission failures now produce structured audit records
  instead of being silently dropped.
- Heightmap startup now has a durable prepare/persist/commit barrier. Planning
  and controller inspection cannot emit motion; the actor starts probing only
  after `surface-session.json` contains the matching pending generation. A
  failed persistence write discards the preparation and leaves the last active
  workpiece map untouched.
- Z-probe and heightmap no longer queue ordinary machine commands for delayed
  execution. They fail immediately with typed Busy; only Status, Hold, Reset and
  the operation's own Pause/Resume/commit controls remain preemptive. A command
  clicked during a long probe therefore cannot move the machine minutes later.
- Heightmap-internal travel no longer emits absolute `$J=G90`. Bounded relative
  deltas are verified against fresh work coordinates; a mismatch or timeout
  produces Feed Hold plus Soft Reset and no follow-up motion. The lifecycle
  ticker also stands down while the operation actor owns status polling, which
  prevents duplicate serial traffic and WebView event floods.
- CAM now uses VTracer 1.0 alpha's library-only pipeline. Existing PNG fixtures
  remain equivalent while old `clap 2`, `atty`, `image 0.23` and `adler` paths
  have left the dependency graph.
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

- `npm audit`: 0 vulnerabilities in runtime and development dependencies.
- `cargo audit`: 0 vulnerability failures across 528 locked crates. It reports
  18 allowed warnings from
  all-target transitive dependencies: GTK3/unic/proc-macro crates are
  unmaintained, Rhai still depends on unmaintained `smartstring`, and
  `glib 0.18.5` has `RUSTSEC-2024-0429`. The GTK/glib path is the Linux backend
  of current Tauri 2.11.5 and is not compiled for the current macOS target.
- `npm outdated` reports only major-version migrations outside the current
  declared ranges: Vite 8, the React plugin 6, and TypeScript 7. They are not
  mixed into this safety refactor; each requires its own migration and fixture
  pass.

## Residual boundaries

The monolith, dialog, WebKit, notices and glib items below are a historical
snapshot. See [TECHNICAL_DEBT.md](TECHNICAL_DEBT.md) for the subsequent refactor
and the current separation between resolved work and upstream constraints.

- `styles.css` is still a large ordered cascade shared by mature operator
  screens. Splitting it mechanically during a behavior refactor could change
  precedence without a useful product change. Extract it feature by feature
  only with matching desktop/mobile screenshots and computed-style checks.
- `millo-command` is large because it owns the single serialized controller
  actor plus an extensive colocated hardware-protocol regression module. Its
  runtime state machine should be decomposed by complete use case (sender,
  probe, heightmap), never by moving arbitrary line ranges or exposing actor
  internals. `src-tauri/commands.rs` likewise needs small `AppState` services
  before more async command groups can move without widening field visibility.
- `ProgramWorkspace` and `App` are still substantial orchestration roots. New
  visual behavior must land in feature modules; further extraction should target
  complete stateful use cases and preserve the current gateway boundaries.

- External native libraries, JavaScript, QtScript compatibility modules, and
  arbitrary HTML/React injection remain unsupported. Version 1 deliberately
  accepts auditable Rhai source plus declarative commands only. Sandboxed
  per-plugin storage/network capabilities and package signing can be added as
  separate APIs; they must not be inferred from script installation.
- Tauri's Linux GTK warning remains upstream/transitive. Recheck RustSec on every
  Tauri upgrade and before shipping Linux packages.
- Rhai 1.25.1 has no active RustSec vulnerability but depends on the newly
  unmaintained `smartstring 1.0.1`. Millo's v1 sandbox needs Rhai serde/sync;
  monitor its replacement upstream and do not patch the string representation
  independently without sandbox/fixture review.
- Physical motion was not needed for this review. Protocol and sender behavior
  remains covered by Mock/serial-class fixtures; the existing operator-controlled
  hardware Check/Air procedures remain the release gate for machine movement.
- Three.js is shared by two lazy scene entry points in a 549 KiB chunk (137 KiB
  gzip); the initial application chunk is 458 KiB (136 KiB gzip). Build budgets
  enforce initial <=500 KiB and lazy <=600 KiB. Further Three.js reduction is a
  performance task, not a sender or operator-workflow blocker.

## Verification contract

Run `npm run verify` after every slice. Supply-chain review additionally runs
`npm run test:dependencies` and `cargo audit`. CSP and architecture regressions
are part of normal `npm test` through `test:security` and `test:architecture`.
