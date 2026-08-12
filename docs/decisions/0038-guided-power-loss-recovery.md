# ADR 0038: Guided power-loss recovery

## Status

Accepted.

## Context

GRBL `ok` confirms that a block was accepted into its planner/RX pipeline; it
does not prove that the motion physically finished. A sudden host crash, serial
disconnect, controller reset, or power loss can therefore leave acknowledged
blocks ahead of the tool. GRBL's optional `Ln:` status field identifies the
currently executing `N` block, but firmware may compile that field out.

The initial Millo hardware has no homing cycle, limit switches, probe, or
physical emergency stop. After power loss its physical machine reference is
unknown. Re-sending from an acknowledged or selected source line would combine
unproven position with incomplete modal and planner state.

## Decision

Physical Air/Cut Start is a two-phase transaction. The command actor parses,
authorizes, and prepares a sender run with dispatch disabled. Tauri then stores
the exact source, SHA-256 fingerprint, machine/profile fingerprint, execution
options, sender run sequence, and observed start positions in one recovery
record. The write uses a synced temporary file, preceding `.bak`, atomic rename,
and parent-directory sync. Only a matching actor commit enables dispatch.

Millo replaces source `N` words with its own monotonically increasing source-line
tags on the wire. Sender snapshots keep `executingSourceLine` from GRBL `Ln:`
separate from accepted-line counters. A dedicated worker checkpoints changed
physical execution evidence at most once per second and always checkpoints a
terminal state. A delayed checkpoint is conservative: it replays more work
rather than skipping possibly unfinished work.

At startup or after a terminal interruption, recovery reparses the stored source
and verifies its fingerprint. It is offered only for the same controller and
only when a physical `Ln:` exists. The planner rewinds to the latest preceding
rapid segment that begins at the program's maximum clearance, falling back to
the first known motion. The chosen Safe Z must be finite, no lower than the
program envelope, and bounded.

Recovery produces a new, visible G-code program. Its preamble orders M5, M9,
metric absolute mode, the recorded WCS, Z clearance, XY approach, Z return,
tool/spindle state when applicable, and the parser modal checkpoint before
replaying original source from the anchor. It performs no movement itself.

The operator must confirm restored machine reference, restored G54-G59 work
zero, inspected restart point, clear Safe-Z/replay path, and reachable power
control. The generated program then passes the normal preview, GRBL Check,
preflight, and one-use launch authorization. Dismissal is bound to the recovery
record ID. Completion hides the record; failure/cancellation retains it. An
unresolved record blocks an unrelated physical Start. Only its exact prepared
recovery fingerprint may atomically replace it when the recovery run starts.
The replaced parent remains in the atomic backup until the new run persists its
first physical `Ln:`. Commit failure rolls it back immediately; process loss or
an early failed/cancelled recovery restores the cryptographically linked parent
instead of stranding a no-line child record.

## Consequences

- Millo can survive process or power loss without trusting buffered `ok` depth.
- The first physical block cannot be sent unless recovery evidence is durable.
- Recovery intentionally repeats a bounded section and may leave a small witness
  mark; it never promises an exact cut continuation.
- Firmware without `Ln:` can show the interrupted record but cannot generate a
  restart program.
- A machine without homing still requires manual reference and work-zero setup;
  software cannot reconstruct lost physical coordinates.
- Arbitrary send-from-line remains unavailable. This workflow applies only to a
  Millo-started run with matching durable evidence.
- Starting another file cannot silently erase unresolved recovery evidence.

## Verification

- `millo-gcode`: modal checkpoints and Millo-owned wire line tags.
- `millo-command`: no source block before matching prepared-run commit.
- `millo-recovery`: atomic/backup persistence, conservative rewind, Safe-Z
  bounds, source/machine binding, missing-`Ln:` blocker, terminal visibility.
- React: recovery confirmation/Safe-Z model tests and responsive browser fixture.
- Full repository gate: `npm run verify`.
