# ADR 0009: First-machine configuration is narrow and verified

## Status

Accepted on 2026-08-11.

## Context

The operator confirmed that the first machine has no homing switches or limit
switches. A real read-only inspection nevertheless reported `$21=1` and `$22=1`,
so the guarded jog readiness policy correctly blocked motion. Disabling these
features changes persistent GRBL settings and must not be hidden inside a jog or
implemented through a general command console.

## Decision

Add an actor-only `configure_unhomed_operation` method. It is not exposed through
Tauri IPC. It operates only while the current controller snapshot is stable
`Connected + Idle`, invalidates any existing jog authorization, and:

1. reads a complete Inspector snapshot;
2. sends typed `$21=0` and `$22=0` commands only for non-zero or missing values;
3. stops immediately if either command is rejected or times out;
4. reads a second complete Inspector snapshot;
5. succeeds only when both final values are exactly `0`.

The hardware-smoke executable is the only current caller. It requires one exact
flag confirming the two persistent setting changes and a second exact flag
confirming physical motion. After configuration it performs another status poll
and obtains a new readiness-based single-use jog authorization.

## Consequences

- There is still no arbitrary `$n=value` API or raw controller console.
- A partially accepted sequence is visible as an error and is never silently
  retried; a later run starts by inspecting the actual persisted state.
- Settings already at zero are not rewritten, reducing EEPROM churn.
- The operation cannot enable homing or limits and cannot alter any setting other
  than `$21` and `$22`.
- Normal CI compiles and tests this path against dynamic Mock settings but never
  opens a physical serial port or changes controller EEPROM.

## Validation

The confirmed first-machine run changed both settings from `1` to `0`, verified
the values through a second Inspector read, and then completed the separately
authorized X `+0.100 mm` smoke jog with no reported Y/Z change.
