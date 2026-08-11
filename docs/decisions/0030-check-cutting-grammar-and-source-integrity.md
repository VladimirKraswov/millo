# ADR 0030: Validate cutting grammar without rewriting source semantics

## Status

Accepted.

## Context

GRBL Check mode is useful for production engraving files, which commonly carry
spindle speed and M3/M4 words. Building its plan with Air-run policy rejected
those files before firmware validation. Separately, normalization removed `/`
optional-block markers and `*checksum` suffixes while treating both as warnings,
which could produce an executable command with different semantics.

## Decision

- Check mode builds with the same Cutting grammar that permits M3/M4/S, while
  retaining policy blocks for M6, coolant, probing, machine/reference movement,
  and coordinate mutation.
- `$C` remains serial-only, starts from stable Idle, runs one outstanding line,
  and must return to verified Idle. It grants no motion authorization.
- A metadata-only `O` program-number line is retained in the parser DTO but is
  non-executable. An O word mixed with executable words is rejected.
- Optional-block and checksum syntax is a parser error until Millo implements a
  policy that can preserve and verify its meaning. It is never silently removed
  from an executable plan.

## Consequences

- Normal cutting files can be checked by real GRBL without starting outputs.
- Sender normalization cannot turn optional or integrity-tagged input into a
  different unconditional program.
- Mock fixtures and the physical `grbl-cutting-check.nc` run verify the boundary;
  the physical controller accepted all 24 planned lines and returned to Idle.
