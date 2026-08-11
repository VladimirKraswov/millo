# ADR 0027: Validate approved programs through typed GRBL Check mode

## Status

Accepted.

## Context

Mock fixtures prove sender state transitions but cannot prove that physical
GRBL accepts every modal combination emitted by a CAM file. GRBL's `$C` Check
mode parses program blocks without executing motion, but a raw `$C` or raw-line
endpoint would bypass the actor and could leave the controller in Check mode.
Buffered lines also complicate cleanup after a correlated parser error.

## Decision

- The controller exposes one typed, verified Check-mode transition. Enabling is
  allowed only from fresh `Idle`; disabling is allowed only from `Check`.
- The command actor exposes a serial-only Check run that first builds an opaque
  policy-approved plan and performs fresh status/Inspector/status reads.
- `SenderMode::CheckRun` validates normal command and plan bounds, but dispatches
  only one unacknowledged line. This keeps an error attached to one source block
  and leaves no queued response ahead of the cleanup `$C` acknowledgement.
- Check run does not use physical motion draining and sends `M30` normally.
- Completion, error, cancellation, disconnect, or transport replacement exits
  Check mode and verifies fresh `Idle`. A cleanup failure changes the sender to
  Failed.
- Check-mode errors do not send realtime Hold or Soft Reset because no motion
  was planned. Air and Cutting runs retain buffered RX streaming and their
  stronger abort sequence.

## Consequences

- Physical firmware compatibility can be tested without granting a raw command
  channel or a movement authorization.
- Check throughput is intentionally lower than execution throughput; this is a
  diagnostic mode, while real runs still fill the reported GRBL RX window.
- The first physical fixture exposed GRBL `error:26` for a center-only
  full-circle block. Requiring an explicit target axis is now protected by the
  parser fixture and the successful 25-line physical rerun.
