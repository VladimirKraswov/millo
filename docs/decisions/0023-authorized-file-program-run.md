# ADR 0023: Execute authorized files through one GRBL sender core

## Status

Accepted.

## Context

ADR 0022 proved lease consumption and sender failure behavior behind test-only
fixtures. The next hardware milestone needs to execute the exact file reviewed
in Program without adding a raw command API or a generated test-square path.

## Decision

- Tauri reparses the retained file for preflight, authorization, and Start.
- Preflight is explicit about intent. `AirRun` forbids spindle activation and
  non-zero speed. `Cutting` permits `M3/M4/S`; coolant, probing, M6,
  machine/reference-coordinate motion, coordinate mutation, unsupported parser
  behavior, incomplete preview, and overlong commands remain blockers.
- A 30-second authorization is bound to intent, source fingerprint, controller
  session, fresh status sequence, and observed machine/work positions.
- Start refreshes status and atomically consumes that authorization inside the
  single-owner command actor before loading the sender.
- Sender flow control and response correlation use the bounded FIFO contract
  defined by ADR 0025. Only `ok` advances. `error`, `ALARM`, reset, timeout,
  polling failure, or disconnect fail the run at the correlated line.
- `M0/M1` pause dispatch after their acknowledgement. `M2/M30` terminate the
  compiled plan. Physical modes defer the terminal command itself, enter
  `Draining`, and keep polling until a fresh `Idle`; only then is `M2/M30` sent
  and correlated with `ok`. This avoids treating GRBL planner synchronization as
  a command timeout while keeping Hold/Resume and Reset responsive.
- Feed Hold and Cycle Start use the realtime `!` and `~` bytes. Plain sender
  cancellation is forbidden for physical modes; stopping requires Hold and the
  existing challenge-confirmed Soft Reset workflow.
- The desktop API accepts a parsed file request and authorization ID. It exposes
  no arbitrary line sender and no machine-run plugin capability.

## Consequences

- The first air cut and engraving can use ordinary reviewed `.nc` files; there
  is no privileged built-in square.
- Mock and serial modes share one sender state machine and snapshot stream.
- ADR 0025 replaces the original one-outstanding-line implementation with
  character-counted receive-buffer streaming behind the same sender API.
- This change enables physical execution but does not itself run the connected
  machine. Every hardware program still requires current operator confirmation.
