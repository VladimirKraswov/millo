# ADR 0041: Stable operator layout

## Status

Accepted.

## Context

Short-lived controller states previously mounted and removed controls, notices,
and error rows in normal document flow. Starting a jog inserted `Cancel jog`, a
reset banner displaced the coordinate readout, and sender transitions changed
the action-row geometry. These movements made nearby controls harder to track
and could move a button between pointer down and the operator's next action.

## Decision

Frequently changing operator state must not change the geometry of the primary
control surface.

- Safety actions keep fixed Hold, Reset, and Cancel slots. State changes alter
  labels, emphasis, and availability, not the number of controls.
- Controller Alarm and reset notices share a bounded status slot beside the
  controller mode. An empty slot remains reserved.
- Sender controls use one primary-action slot and one cancel slot for every
  state. Unavailable actions remain invisible placeholders.
- Modal validation errors and autosave status use fixed rows or icon slots.
- Recovery and program errors that must not resize the workspace are bounded
  overlays.
- Scrollable panels reserve their scrollbar gutter.

Large workflow transitions, such as opening a modal or replacing the connection
setup with the connected cockpit, may intentionally change layout. Transient
changes inside an active workflow may not.

## Consequences

- Operators retain spatial memory while Jog, Hold, Reset, Alarm, sender, and
  validation states change.
- Hidden placeholders consume a small amount of space even when inactive.
- New dynamic controls need an explicit stable slot or an overlay with bounded
  dimensions.
- State-to-slot mapping is pure data and can be tested without a browser.

## Verification

- Safety-control rendering tests require the same three actions in Idle and Jog.
- Sender model tests cover the primary and cancel slots for every sender state.
- Browser fixtures compare bounding boxes for Idle, Jog, Alarm, and reset states;
  the controller heading, coordinates, workbench tabs, safety actions, jog pad,
  and coordinate disclosure must have zero position and size delta.
- `npm run verify` remains the repository gate.
