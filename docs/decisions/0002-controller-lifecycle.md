# ADR 0002: Core-owned lifecycle with adapter-owned scheduling

- Status: accepted
- Date: 2026-08-10

## Context

Periodic polling requires a runtime timer, while timeout, reset, alarm, and
reconnection behavior must remain deterministic and testable without Tauri.
Putting the full loop in a desktop command would make machine state dependent on
window lifetime and difficult to verify in isolation.

## Decision

The Tauri adapter owns one cancellable polling task per active connection. It
uses a fixed interval with missed ticks skipped. Each tick calls
`Controller::lifecycle_tick` and publishes the resulting snapshot, including
snapshots produced after an error.

After the transport opens, the adapter starts this task even when the initial
status synchronization fails, allowing the same failure threshold and recovery
path to finish startup without another operator action.

The controller core owns connection state, failure thresholds, response timeout,
reset/alarm handling, and recovery synchronization. Recovery is complete only
after transport reconnect and a valid GRBL status frame.

Reset and alarm are modeled independently: reset is an operator notice that can
be acknowledged, while alarm is machine state that clears only when the
controller reports a non-alarm status.

## Consequences

- Lifecycle transitions are covered by fast Rust tests with a scripted mock.
- Tauri contains scheduling and event delivery but no CNC policy.
- Future serial, WebSocket, and Telnet transports share identical recovery logic.
- Commands may wait briefly for a poll holding the controller mutex, bounded by
  the configured status timeout.
- More advanced transports may later need a dedicated actor, but the domain
  state machine will not change.
