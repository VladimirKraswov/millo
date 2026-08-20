# ADR 0058: Configurable actor-owned operator console

## Status

Accepted on 2026-08-20. Extended on 2026-08-20 with an opt-in expert policy.

## Context

Operators need firmware, settings, modal, coordinate, and status diagnostics in
one transcript. Experts and some community extensions also need firmware-specific
commands that do not yet justify a dedicated Millo use case. A conventional raw
serial terminal would duplicate transport ownership and could steal `ok`,
`error`, or `ALARM` responses from sender, probing, homing, or heightmap work.

## Decision

Millo exposes one operator-console request owned by the existing Rust command
actor. Application preferences persist `safeCommandMode`, defaulting to `true`.

In safe mode Rust accepts only `?`, `$I`, `$$`, `$G`, and `$#`. In expert mode it
also accepts one printable ASCII GRBL line up to 255 bytes. Expert lines are
available only in `Idle` or `Alarm` and only while no sender or actor-owned
machine operation is active. The actor performs a fresh status read before an
expert line and correlates its terminal response before polling status again.

Realtime `!`, `~`, Ctrl-X, overrides, and Jog Cancel remain named typed actions;
they are never accepted as line input. The setting does not expose a transport,
serial handle, arbitrary bytes, or a second response reader to the WebView.

An expert line invalidates first-cut authorization, GRBL Check evidence, verified
Z datum, homing envelope, and machine-reference evidence. Every result and every
policy change is written to the persistent audit log.

External Rhai packages can return `rawCommand` only when all of these are true:

1. the package and command declare `machine.commands`;
2. that capability is granted to the exact reviewed package digest;
3. the operator confirms the machine action;
4. global safe command mode is disabled;
5. the same actor lifecycle accepts the command.

## Consequences

- The default remains a diagnostic-only console suitable for normal operation.
- Experts can use controller-specific commands without bypassing serial
  serialization or response correlation.
- Sender and long-running operations cannot lose acknowledgements to the console
  or a plugin.
- Expert flexibility deliberately discards stale run and coordinate evidence;
  the next machining workflow must inspect and authorize the machine again.
- Useful expert operations should still graduate into typed, documented use
  cases when their hardware policy becomes known.
