# ADR 0006: Hardware readiness is a separate policy module

## Status

Accepted on 2026-08-11.

## Context

The first physical controller answers the read-only Inspector, but that alone
does not make movement safe. GRBL settings must be interpreted against facts
that are outside the protocol: this machine has XYZ motion, a manually switched
spindle, no homing switches, no limit switches, and no physical emergency stop.
Embedding those rules in React, the serial transport, or the GRBL parser would
make them difficult to test and reuse.

## Decision

Create `millo-readiness` as a pure Rust policy crate. It receives a typed
`DeviceInspection`, current `ControllerSnapshot`, and explicit
`HardwareProfile`. It returns ordered pass, caution, and blocker checks plus two
separate gates: guarded test jog and probing.

The command actor constructs the report after the four allow-listed read-only
queries. Tauri only serializes `HardwareInspection`, and React localizes and
renders the result. No movement or raw command endpoint is added in this slice.

## Consequences

- Hardware policy is fixture-testable without serial hardware or a desktop UI.
- A profile contradiction such as `$21=1` without installed limit switches is
  visible and blocks the future test jog.
- Expected risk does not disappear: unhomed operation and manual spindle remain
  cautions rather than being rendered as success.
- The report describes the inspection moment. Any future movement action must
  run a fresh live-state gate inside the actor; the UI report is never sufficient
  authorization to write motion bytes.
- Probe readiness remains locked until a dedicated stationary electrical-input
  test exists.
