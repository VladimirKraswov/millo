# ADR 0044: Sparse GRBL status reconciliation

## Status

Accepted.

## Context

GRBL status frames are incremental telemetry rather than complete machine
snapshots. In particular, `WCO` and `Ov` are refreshed periodically and omitted
from intervening frames. Millo parsed each frame correctly but then replaced the
entire `MachineState`. A controller configured to report `MPos` therefore made
the derived work position appear only on `WCO` refresh frames. React alternated
between `Work zero` and `Validation` at the polling rate even though neither the
machine nor its origin changed.

## Decision

The Rust controller reconciles sparse telemetry before publishing a snapshot.

- The last observed `WCO` and overrides remain valid within one controller
  session when an individual status frame omits them.
- A frame containing both `MPos` and `WPos` derives a fresh `WCO`; explicit WCO
  remains authoritative when present.
- Given `WCO`, every status derives the missing side of `MPos = WPos + WCO`, so
  both coordinate readouts advance from the same physical observation.
- Transient fields whose absence has meaning, including pins and accessories,
  are not retained.
- Disconnect and reset still replace `MachineState` with its default. Cached
  coordinate evidence never crosses a controller session or reset boundary.

## Consequences

- Coordinate readouts and job readiness no longer flicker with GRBL's periodic
  WCO reporting cadence.
- Sender, plugins, logging, and UI all consume the same reconciled snapshot;
  there is no separate React debounce or display-only cache.
- Immediately after a reset, work position remains unknown until the new
  controller session provides sufficient coordinate evidence.

## Verification

- A controller test consumes a full `MPos + WCO + Ov` frame followed by sparse
  `MPos` and verifies continuously updated WPos plus retained WCO/overrides.
- The same test proves a later `MPos + WPos` frame refreshes the cached offset.
- A reset test proves old WCO/WPos cannot leak into the next sparse frame.
- `npm run verify` remains the repository gate.
