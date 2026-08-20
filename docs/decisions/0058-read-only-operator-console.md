# ADR 0058: Read-only operator console

## Status

Accepted on 2026-08-20.

## Context

Operators need firmware, settings, modal, coordinate, and status diagnostics in
one transcript. A conventional raw GRBL terminal would duplicate serial
ownership and bypass typed work-zero, homing, probing, sender, output, and reset
policies. A UI-only denylist would also be bypassable through Tauri IPC.

## Decision

Millo exposes one actor-owned operator-console request. Rust normalizes and
classifies input against the exact read-only allowlist `?`, `$I`, `$$`, `$G`,
and `$#`. No arbitrary line or byte reaches the controller. Line queries are
available only in Idle or Alarm and never while a sender or another machine
operation owns a response lifecycle.

The status response is rendered from the parsed controller snapshot. Query
responses retain GRBL lines and terminal completion. Tauri records every result
in the persistent controller audit category. The frontend keeps only a bounded
session transcript and repeats the policy for immediate feedback; Rust remains
authoritative.

## Consequences

- Diagnostics become accessible without introducing a raw serial endpoint.
- Sender acknowledgements and long-running operation responses cannot be stolen.
- Adding another console command requires an explicit core review and Rust enum
  change, not a UI configuration update.
- Plugins receive no implicit console or transport capability.
- Machine-changing commands continue through named, typed, verifiable use cases.
