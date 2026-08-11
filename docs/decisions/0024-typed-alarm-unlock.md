# ADR 0024: Keep GRBL alarm unlock typed and freshly verified

## Status

Accepted.

## Context

An interrupted hardware Air run correctly left GRBL in `Alarm`. The next run
needed `$X`, but Millo intentionally exposes no raw command path. Unlock removes
a controller interlock and therefore must not become an unverified convenience
write.

## Decision

- Unlock requires an explicit operator-confirmation value at the command actor.
- A rejected or missing confirmation performs no controller I/O.
- The controller reads a fresh status and permits the operation only from
  `Alarm`.
- The typed operation emits exactly `$X`, waits for its correlated `ok`, then
  reads status again.
- Success requires fresh `Idle` and no retained alarm. Any rejection, timeout,
  transport failure, or non-Idle result fails closed.
- Unlock invalidates pending jog and program authorizations. It does not itself
  authorize movement.

## Consequences

- Hardware recovery does not require an arbitrary G-code or console API.
- Mock controller and command-actor fixtures verify confirmation, exact bytes,
  and post-command state.
- A new movement still needs its own current preflight and one-use authorization.
