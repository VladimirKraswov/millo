# ADR 0010: UI and machine access cross capability boundaries

## Status

Accepted on 2026-08-11.

## Context

Millo is expected to support plugins that can add, remove, replace, and reorder
UI, contribute complete workflows, create jobs, and control the machine. Binding
features directly to React globals or Tauri `invoke` would make that goal brittle.
Giving plugin code a serial handle or raw GRBL endpoint would also bypass the
safety properties already enforced by the command actor.

## Decision

Core behavior remains a set of typed Rust application use cases behind the
single command actor. React features depend on narrow TypeScript gateway
interfaces; only platform adapters know about Tauri IPC. The first concrete
boundary is `MachineCommandGateway`, consumed by the standalone jog-pad feature.

The future plugin host will use these rules:

1. A versioned manifest declares UI contributions and requested capabilities.
2. UI is composed from named slots, routes, panels, commands, and inspectors.
   Core UI will migrate into the same registry, allowing plugins to replace or
   disable contributions without mutating arbitrary DOM nodes.
3. Every contribution has an owner ID. Disabling or unloading an owner removes
   its UI, commands, subscriptions, and resources deterministically.
4. Machine access is a host-provided capability proxy. Initial capability groups
   will include read-only state, guarded motion, probing, sender control, and
   overrides; permissions remain separate.
5. Job creation uses a versioned job/program service rather than internal React
   state or filesystem conventions.
6. Plugins never receive the serial transport, command actor, Tauri internals,
   raw realtime bytes, or unrestricted G-code writes. Safety-critical use cases
   retain their readiness, confirmation, authorization, and state checks.
7. Untrusted plugin execution and native/plugin signing are separate design
   decisions. This ADR defines host boundaries, not a loader implementation.

## Consequences

- Feature modules can move between the core UI and a plugin without rewriting
  their machine orchestration.
- Plugins can eventually reshape every compositional UI surface while machine
  safety remains owned by Rust and independent of presentation.
- New core features must expose an application use case and gateway contract
  instead of importing Tauri APIs inside components.
- Direct arbitrary DOM replacement is intentionally unsupported; replacement is
  explicit, ordered, reversible, and attributable to a plugin owner.
- The minimal in-memory UI registry is now implemented. Manifest parsing,
  capability grants, isolation, persistence, and the job service remain
  incremental follow-up decisions.

## Validation

Jog Pad is registered by owner `core` as contribution `core.jog-pad` in the
named `control.machine` slot. The generic registry is independent of React and
supports deterministic ordering, replacement, individual disposal, and atomic
owner unload. Removing a plugin replacement reveals the core contribution again.
The React bridge subscribes only to a monotonically increasing registry revision
and renders the currently active contributions.
