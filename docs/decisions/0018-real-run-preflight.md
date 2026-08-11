# ADR 0018: Serial real-run preflight is read-only

## Status

Accepted.

## Context

Millo can parse and preview G-code, execute a bounded sender against Mock GRBL,
and perform tiny authorized jogs on the first physical machine. A serial program
sender would nevertheless be premature: the machine has no homing, limits, or
physical emergency stop, uses a manual spindle, and still needs an explicit
first-cut setup workflow.

The next useful step must inspect real controller and program state without
allowing a green UI flag, stale snapshot, or parsed preview to become motion
authority.

## Decision

- Add `millo-run` as a pure Rust policy module for real-run preflight reports.
- Reparse the retained original source in Tauri, then pass the Rust program DTO
  through the existing single-owner command actor.
- Require the actor's execution target to be Serial and perform a fresh
  `?`, `$I`, `$$`, `$G`, `$#`, `?` transaction.
- Reuse the strict motion-only dry-run policy for the first physical program:
  automatic spindle/coolant control, probing, tool change, machine/reference
  motion, coordinate mutation, unsupported safety behavior, incomplete preview,
  and oversized commands remain blockers.
- Require explicit `G21` and `G90` before first motion, `G94` before first feed
  motion, and `G17` before first XY arc. Parser defaults never substitute for
  physical-controller modal declarations.
- Combine that result with final controller state, motion-critical readiness,
  complete bounded geometry, and active G54-G59 checks.
- Treat probe readiness separately because probing is forbidden by the program
  policy. Keep the unhomed envelope, manual spindle, and unconfirmed physical
  setup visible as cautions.
- Return a source-addressable report only. Do not mint a plan, lease,
  authorization, serial sender, or Start control.

## Consequences

- Passing preflight means the observed program/controller pair is eligible for
  the next operator-confirmation design step; it does not permit motion.
- Mock and disabled execution targets are rejected before controller I/O, while
  automated actor tests can model the serial class with deterministic transport.
- Program blockers can select their immutable source row and 3D relation without
  changing future execution order.
- A future Start transaction must repeat or consume equally fresh evidence
  atomically and remain entirely inside Rust.

ADR 0021 adds that next evidence boundary as a short-lived first-cut lease. It
does not change this report's read-only semantics or introduce a serial sender.
