# ADR 0040: Machine-scaled Motion Deck

## Status

Accepted on 2026-08-12.

## Context

A fixed `0.01/0.10 mm` jog pad was appropriate for first-motion bring-up but is
too slow for normal positioning. A global `50 mm` cap is also wrong: Millo must
serve both a `300 x 180 mm` desktop router and machines measured in meters.
Distance and speed need to feel direct without allowing React or a plugin to
bypass the selected machine's limits.

## Decision

Store `maxJogDistanceMm` in each machine profile. A newly detected or manually
created machine starts at the smaller of `50 mm` and its largest configured
axis. The operator may set it from `0.01 mm` through that largest axis; it is a
local preference and is not written to GRBL.

Motion Deck exposes precision, positioning, and traverse presets plus direct
distance and feed inputs. Presets scale to the profile and inspected controller
rates. Actual acceleration remains the controller-owned `$120/$121/$122`
settings and is edited through the verified controller-settings workflow.

Every press remains a typed single-axis `$J=` request. Before writing, the Rust
actor requires fresh readiness and applies all of these independent bounds:

- finite protocol hard limits;
- `maxJogDistanceMm` from the bound machine profile;
- travel of the requested X, Y, or Z axis;
- that axis' live `$110`, `$111`, or `$112` maximum rate.

## Consequences

- Small and large machines get useful controls without a global magic number.
- Switching profiles recalculates presets and clamps stale custom values.
- A compromised UI or plugin cannot exceed the actor-owned machine bounds.
- The profile JSON records operator intent while GRBL remains the source of
  truth for travel, maximum rates, and acceleration.

## Verification

- TypeScript tests cover `50 mm` and `3000 mm` profiles and signed requests.
- Rust profile tests accept a `2000 x 3000 mm` machine and reject a limit beyond
  travel.
- Rust actor tests reject profile overrun and feed above selected-axis GRBL rate
  before any `$J=` write.
- `npm run verify` remains the repository gate.
