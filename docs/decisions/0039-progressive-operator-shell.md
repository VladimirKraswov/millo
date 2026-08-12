# ADR 0039: Progressive operator shell

## Status

Accepted.

## Context

The first complete control surface exposed connection tuning, lifecycle metrics,
Mock fault injection, every readiness pass, work-coordinate tools, stream
options, and the full G-code table at the same visual level as normal operation.
All of those capabilities are useful, but their permanent visibility made state,
preview, motion, and the next operator action harder to find.

The original safety dialogs also represented every backend fact as a separate
checkbox and separated run authorization from Start. This was auditable but
added repetitive UI work without increasing the freshness or strength of the
Rust actor's validation.

## Decision

The shell uses progressive disclosure.

- Connection state, selected machine, machine coordinates, preview, run state,
  Hold/Reset, and connected jog controls remain immediately visible.
- An empty Program workspace presents `Open G-code` as its primary action and
  accepts the same file types by picker or drag-and-drop. After loading, the
  action becomes a compact `Replace file` command in the program toolbar.
- Port selection, baud rate, lifecycle counters, raw status requests, Mock fault
  injection, work-zero controls, passed readiness evidence, G-code rows,
  optional stop/block delete, and GRBL Check are available in named disclosures.
- Parser warnings and preflight evidence open diagnostics automatically when
  attention is required.
- Decorative protocol/pipeline status is removed. The same information remains
  observable through typed snapshots and connection diagnostics.

Jog, launch, tool change, and recovery each present one action-level readiness
decision. A feature-local pure function expands that decision into the existing
typed backend confirmation fields. The backend API, actor guards, fresh status
and Inspector transactions, bounds, and one-use authorization semantics are not
weakened.

Program launch uses one UI command to authorize and immediately consume the
lease. These remain two ordered backend operations. A failed authorization,
stale state, changed position, failed reparse, or failed lease consumption sends
no source G-code. Soft Reset keeps its short-lived actor challenge but confirms
with a second press of the same button.

Core and plugin UI continue to use named extension slots. Progressive disclosure
changes slot placement, not capability ownership or unload behavior.

## Consequences

- Normal setup and execution have a shorter visual and interaction path.
- Advanced and diagnostic capabilities remain reachable without dominating the
  operator cockpit.
- A compact confirmation has broader wording, so its exact facts must remain
  inspectable in the adjacent disclosure and tested as a pure mapping.
- Safety remains enforced in Rust when React or a future plugin calls the API
  incorrectly.

## Verification

- UI model tests cover compact-to-typed mappings for jog, launch, M6, and
  recovery.
- Vite fixtures cover the normal program shell and launch dialog.
- `npm run verify` remains the repository gate.
