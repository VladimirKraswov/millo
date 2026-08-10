# ADR 0003: Project name Millo

- Status: accepted
- Date: 2026-08-10

## Context

The original working title was visually distinctive but awkward to pronounce in
Russian and easy to mishear. The project needs a short name that works in spoken
operator conversations as well as package and binary names.

## Decision

Use **Millo**, pronounced "Mee-lo" ("Милло"), as the product and repository
name. Rust packages use the `millo-*` prefix, the npm package is `millo`, and the
desktop identifier is `io.millo.desktop`.

The existing geometric mark remains valid because it contains no lettering.

## Consequences

- Product, repository, package, crate, binary, and bundle names share one root.
- A brand contract test checks key metadata and rejects the retired working
  title in tracked text files.
- Existing Git history is not rewritten.
