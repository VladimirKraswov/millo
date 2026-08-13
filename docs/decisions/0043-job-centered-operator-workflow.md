# ADR 0043: Job-centered operator workflow

## Status

Accepted.

## Context

Program loading, connection, Alarm unlock, work-zero setup, preflight, GRBL
Check, and run authorization were individually available but lived in different
parts of the shell. An operator who wanted to engrave one file had to infer the
correct order from controller terminology and disabled buttons. A numbered
wizard would make that order visible, but would also hide context, interrupt
experienced workflows, and become awkward when the machine state changes
asynchronously.

Machine coordinates were also the largest readout even though ordinary G-code
runs in a G54-G59 work coordinate system. That made a technically accurate but
operator-hostile hierarchy.

## Decision

Program owns a compact, non-modal job-readiness surface beside the toolpath.

- Four persistent facts describe the current job: Machine, File, Work zero, and
  Validation. They are status rows, not wizard steps, and remain visible while
  the operator changes the execution mode.
- A pure `jobReadinessModel` selects exactly one contextual primary action. Its
  priority is connection, Alarm unlock, reset acknowledgement, stable Idle,
  unresolved recovery, parser review, work zero, preflight, required GRBL Check,
  and finally Start. Start is unavailable while the initial recovery query is
  still pending.
- Alarm unlock is a typed Tauri command that delegates to the sole Rust command
  actor. The click itself is explicit operator confirmation; the actor rereads
  Alarm, sends exactly `$X`, and verifies Idle before reporting success.
- Work-zero setup opens from the readiness row. The dialog offers one prominent
  XYZ action plus individual X/Y/Z actions; every axis still uses the existing
  typed `G10 L20` request and `$G`/`$#` verification.
- G54-G59 work position is the primary readout. Controller-reported WPos is
  preferred; otherwise a pure read model derives it from MPos, WCO or Inspector
  G5x/G92/TLO data. G53 remains visible as compact secondary evidence.
- Execution options, full parser rows, warnings, preflight evidence, and GRBL
  Check remain available through disclosures. A blocking report opens the
  relevant evidence instead of adding more primary buttons.
- Completing GRBL Check automatically returns to readiness and reruns preflight,
  allowing the new certificate to expose Start without manual navigation.
- An active physical sender owns Pause/Resume and a dedicated two-press Finish
  action in the run card. Finish invokes one typed actor operation for Hold plus
  Reset. Its terminal view then exposes recovery/new-start resolution instead of
  leaving the operator in a disabled sender card.
- A completed GRBL Check run sequence is consumed exactly once. Repeated backend
  snapshots with that sequence remain hidden while automatic preflight exposes
  an intent-specific `Начать обработку` or `Начать проверку движения` action.
- An active GRBL Check always has an explicit Cancel action. Cancellation and
  failure return to readiness through a dedicated Check action model and never
  invoke physical-run recovery lookup.

The model is host-owned and receives typed machine facts. Plugin UI slots may
decorate or replace presentation, but cannot mint readiness, unlock, work-zero,
or sender authority.

## Consequences

- A first-time operator can follow the current primary action without losing the
  full preview or machine context.
- Experienced operators can jump directly to work zero, diagnostics, or mode
  selection without stepping through a modal sequence.
- The UI has one deterministic place to add future probe, tool, stock, or plugin
  readiness facts without duplicating safety policy in React.
- Derived work position depends on a fresh status/Inspector snapshot; when the
  required evidence is absent the UI displays an unknown value and asks for work
  zero instead of assuming zero.
- A previous interruption cannot appear only as a raw persistence failure after
  launch confirmation; it is a named readiness fact and next action.

## Verification

- Read-model tests cover connection, Alarm priority, pending/outstanding
  recovery, missing origin, required GRBL Check, and fully ready Start.
- Coordinate tests cover controller WPos and MPos derivation through G5x, G92,
  and TLO.
- Component tests keep four facts, one primary action, and XYZ/single-axis work
  zero controls visible.
- `/?fixture=first-cut` visually verifies unchecked, ready, work-zero dialog, and
  final confirmation plus Pause/Finish/new-run states. `/?fixture=check-complete`
  verifies the terminal Check-to-Start transition; `/?fixture=check-running`
  verifies Cancel-to-preparation. `/?fixture=recovery`
  verifies that unresolved evidence replaces Start and opens the explicit
  continue-or-dismiss decision.
- `npm run verify` remains the repository gate.
