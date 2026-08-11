# ADR 0013: Use one PluginHost as the application composition root

## Status

Accepted on 2026-08-11.

## Context

The application previously kept controller state in React while the plugin
loader accepted a separate optional machine source. Leaving both paths in place
would let operator UI and plugins observe different snapshots. Directly binding
Tauri events inside plugin code would also leak the desktop adapter boundary.

Tauri listener registration is asynchronous. React StrictMode can dispose an
effect before that registration resolves, and the initial snapshot request can
complete after a newer event. Both races need deterministic handling.

## Decision

`bootstrapPluginHost` creates one UI extension registry, one
`MachineSnapshotStore`, and one `InMemoryPluginLoader` for each application
instance. It registers core UI contributions and passes the shared state and
machine-command gateways into the loader. Bootstrap does not discover, import,
or activate plugin modules; the active plugin list starts empty.

React observes the host's machine store through `useSyncExternalStore` instead
of owning a second controller snapshot. Results from typed controller actions
publish to the same store.

`MachineStateEventStream` represents the desktop event boundary with
`readCurrent()` and `listen()`. Its Tauri adapter delegates to
`controller_snapshot` and the `machine-state` event. `bindMachineStateStream`
publishes both paths into the host store and applies these lifecycle rules:

- a live event increments a revision, preventing a later initial response from
  replacing newer state;
- no value is published after disposal;
- an unlisten callback that resolves after disposal is executed immediately;
- stream, callback, diagnostics, and cleanup failures do not bypass teardown.

The bridge remains outside the plugin activation context. Plugins can observe
the resulting store only through an explicitly granted `machine.read` proxy.

## Consequences

- React core UI and plugins share one authoritative, immutable machine snapshot.
- Tauri imports stay in a narrow adapter instead of entering plugin modules.
- StrictMode remounts cannot leave a late event listener behind.
- No external code loading or implicit plugin activation is introduced.
- Persistent grants, package discovery, and external plugin isolation remain
  future decisions.

## Validation

Unit tests exercise bootstrap ownership and explicit activation, initial and
event publications, stale-response ordering, post-disposal suppression, and
late-listener cleanup. The full UI smoke test confirms the shell still renders
one core Jog Pad and no fixture plugin contribution.
