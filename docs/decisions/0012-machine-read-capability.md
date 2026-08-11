# ADR 0012: Expose machine state as immutable scoped observations

## Status

Accepted on 2026-08-11.

## Context

Plugin panels and workflows need live controller state, but the application
currently receives mutable DTOs directly from Tauri events. Giving plugins that
event channel or the React store would expose platform internals, allow accidental
state mutation, and leave subscriptions alive after a plugin is unloaded.

## Decision

The platform defines `MachineStateSource` with only `current()` and
`subscribe(listener)`. `MachineSnapshotStore` is its mutable host-side
implementation. Every published `ControllerSnapshot` is detached from the input
and deeply frozen across nested machine state, positions, reset notices, and
alarms before it crosses the capability boundary.

The `machine.read` plugin proxy exposes the same two read operations. A
subscription receives future publications only; plugins call `current()` when
they need the initial value. Reading state never triggers polling, refresh,
connection changes, or other controller I/O.

Every activation receives a private resource scope. The loader tracks each
machine-state unsubscribe in that scope and disposes all of them after failed
activation and when unload begins, before awaiting plugin deactivation. Once
closed, retained read, UI, and jog proxies reject further operations. Exceptions
from a plugin listener are isolated from other listeners and passed to an
optional host diagnostics callback; exceptions from that callback are also
contained.

The capability remains fail-closed: a host state source and an explicit grant
must both exist. Merely wiring a source does not authorize a plugin.

## Consequences

- Plugins can render current machine state without access to transport, Tauri,
  the command actor, or mutable React state.
- Mutation attempts cannot alter the host snapshot or another observer's view.
- Unloaded and partially activated plugins cannot keep receiving machine events
  or reuse saved capability proxies.
- The application shell still needs to wire its authoritative event stream into
  `MachineSnapshotStore` when a production plugin host is introduced.
- History, replay, throttling, selector subscriptions, and persistent grants are
  deliberately outside this slice.

## Validation

Tests mutate source DTOs after ingestion, inspect runtime freezing at every
nested level, exercise current and future reads, verify explicit grants, isolate
throwing listeners, and prove cleanup after activation failure and unload.
