# ADR 0001: Independent modular Rust core

- Status: accepted
- Date: 2026-08-10

## Context

The previous port demonstrated broad Candle compatibility, but it also showed
that translating a mature UI-centered architecture carries forward accidental
coupling. Gantryon needs deterministic tests, multiple transports, and CNC safety
logic that does not depend on a desktop framework.

## Decision

Build an independent Rust core around small domain, protocol, transport, and
controller crates. Keep Tauri as an adapter and React as a state consumer. Use
Candle only to discover operator scenarios and compatibility expectations.

Implement one end-to-end behavior at a time. A slice is complete only when its
core behavior has Rust tests and its desktop contract is usable from TypeScript.

## Consequences

- Core tests run without WebView, serial hardware, or JavaScript.
- Serial, WebSocket, Telnet, and mock transports can share one lifecycle.
- UI redesigns cannot silently change machine behavior.
- More explicit boundary types are required.
- TypeScript bindings are mirrored manually in the first slice; generated
  bindings should replace them before the command surface becomes large.
