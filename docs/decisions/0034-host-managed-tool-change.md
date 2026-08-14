# ADR 0034: Treat M6 as a verified host-managed barrier

## Status

Accepted.

## Context

GRBL 1.1 accepts tool selection words but does not provide a complete automatic
tool-change cycle for the target machine. Sending `M6` as an ordinary buffered
line makes behavior firmware-dependent. Pausing immediately after its `ok` is
also insufficient because earlier motion may still remain in the planner.
Ordinary Resume cannot prove that the requested tool, Z zero, safe clearance,
work coordinate system, spindle, or emergency power access were rechecked.

## Decision

- Cutting policy accepts `M6` only in a block containing `N`, `T`, and `M6`.
  Air policy continues to reject it.
- A known initial `Tn M6` before the first cutting motion is startup setup. The
  final start dialog shows `Tn`, and the initial `M6` does not create a redundant
  pause. An unknown initial `M6` remains a barrier.
- A same-block `Tn` is split into a normal `Tn` controller line followed by an
  opaque host-only barrier for every later change. A previously selected `Tn`
  is carried into the barrier metadata.
- Sender dispatch waits for an empty response FIFO before entering
  `ToolChange`. The `M6` text is never passed to the transport.
- GRBL Check sends and validates `Tn`, accounts for the host barrier locally,
  and never sends `M6`.
- Tool-change confirmation is bound to the exact source line and requested
  tool. Ordinary program Resume is invalid in this state.
- Completion requires every operator fact plus a fresh `Idle`, complete
  Inspector transaction, active G54-G59, and a final fresh `Idle`.
- Elapsed runtime excludes time spent at the tool-change barrier. Alarm, reset,
  disconnect, transport replacement, and confirmed stop still terminate it.

## Consequences

- Tool-change behavior is independent of controller-specific `M6` handling.
- Buffered motion is acknowledged before the barrier, and continuation cannot
  occur while the controller still reports motion.
- The operator gets an explicit line/tool workflow instead of a generic pause.
- Starting a job with the already installed first tool no longer presents a
  misleading tool-change interruption.
- Jogging, probing, or zero mutation inside the barrier remain future typed
  interventions. They must capture and revalidate modal/position state before
  they are enabled.
