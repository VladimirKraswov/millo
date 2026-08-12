# ADR 0037: certified Check and bounded run journal

## Status

Accepted.

## Context

A successful GRBL Check action is useful only if the later Cutting run can
prove that the same source and optional-program interpretation were checked in
the same controller session. A sender failure also needs durable diagnostic
evidence, but an acknowledged line number alone is not enough to resume safely.

## Decision

`millo-run` owns a 15-minute `ProgramCheckGate`. The command actor issues a
certificate only after the complete Check plan is acknowledged, GRBL leaves
`$C`, and fresh state is `Idle`. The certificate binds source SHA-256, Optional
Stop, Block Delete, reset count, and reconnect count. Cutting preflight requires
it; Air run does not.

M2/M30 is validated by parser/policy and acknowledged locally during Check. It
does not reach firmware. For firmware that emits a reset banner while disabling
`$C`, the actor accepts only one new reset count observed inside that successful
cleanup, clears the notice, and requires another clean `Idle` status. Any other
reset remains terminal.

`millo-sender` assigns each loaded plan a process-local `runSequence`.
`millo-journal` stores at most 100 runs and checkpoints start, state changes,
every 250 acknowledgements or two seconds, and every terminal snapshot. Writes
use a synced temporary file and preserve the preceding JSON as a backup.
Failed and cancelled entries are marked `RestartBlocked`.

## Consequences

- Checking one file or option set cannot authorize another.
- Reset, reconnect, disconnect, expiry, incomplete Check, and failed `$C`
  cleanup invalidate validation evidence.
- A crash leaves bounded last-ACK and typed-failure evidence without frequent
  per-line disk writes.
- The journal cannot start motion. ADR 0038 adds those modal, position,
  work-coordinate, safe-approach, and operator-authorization proofs in a
  separate single-record recovery store; journal entries remain non-executable.
