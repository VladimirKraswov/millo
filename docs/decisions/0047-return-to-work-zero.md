# ADR 0047: Return to an existing work zero

## Status

Accepted.

## Context

A G-code program normally finishes above the material at Safe Z. For a second,
slightly deeper engraving pass, the operator must first recover the original
surface Z0, jog down by a measured increment, redefine only Z, and rerun. A
generic coordinate input or another `G10` command would make “return” and
“redefine” dangerously ambiguous.

## Decision

Millo models return-to-work-zero as a separate typed actor request. The only
variable target is axis X, Y, or Z; the target coordinate is always zero. Rust
refreshes status and modal state, requires connected stable `Idle`, checks the
active G54-G59 system, bounds feed and travel, and emits one absolute GRBL jog:

```text
$J=G90 G21 Z0.000 F100.000
```

X/Y return additionally requires positive work Z. The request never emits
`G10`, so it cannot alter the saved work offset. Program places the common Z0
action directly in the completed-job card and keeps X0/Y0/Z0 together in the
full Work Zero panel.

## Consequences

- “Go to zero” and “make this zero” are distinct operations in code and UI.
- A repeat-depth workflow is discoverable where the previous run ends.
- Plugins and React never receive arbitrary absolute-motion authority.
- The operator still re-runs readiness and authorization after changing Z0.
