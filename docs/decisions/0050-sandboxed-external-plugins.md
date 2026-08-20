# ADR 0050: Execute external plugins as bounded typed-action scripts

## Status

Accepted on 2026-08-12.

## Context

The linked extension host proved UI ownership, capability proxies, and unload,
but did not let users share source. Candle's QtScript ecosystem is valuable,
while exposing application, device, and sender objects would undo Millo's
single-owner serial and authorization boundaries.

## Decision

External code uses a versioned `.millo-plugin` JSON package and the embedded
Rhai language. UI is declarative: named surfaces, buttons with allowlisted
icons, generated modal windows, and typed fields. The script cannot provide
HTML, React, JavaScript, a native library, or a Tauri command.

The host validates and compiles source before storing it. Imported or changed
packages are disabled. Enabling records explicit capabilities against the exact
SHA-256 package digest. Runtime evaluation has operation, call, expression,
string, collection, source, and output bounds; import and dynamic eval are
disabled and no filesystem, network, serial, sender, DOM, or Tauri APIs are
registered.

A call returns one tagged action. Job source is reparsed and published to the
ordinary Program workflow. Jog and coordinate actions require an operator
confirmation and delegate to existing typed actor methods. The optional
`machine.commands` capability may return one bounded `rawCommand`, but only for
the reviewed digest, after confirmation, while global safe command mode is
disabled. It still enters the same actor and never exposes serial bytes or a
response reader. The actor remains the only serial owner.

The system operator-macro package runs through this same path. First-party React
plugins remain on the linked in-memory host because they are compiled and
reviewed with the application.

## Consequences

- Community macros can add and remove UI, open parameter windows, create jobs,
  observe granted state, and request guarded machine actions without owning raw
  serial.
- Source changes cannot inherit silent authority from an older reviewed build.
- The package is portable and auditable without bundling a JavaScript toolchain.
- Version 1 does not provide QtScript compatibility, arbitrary custom layout,
  native code, storage, network, probing, sender ownership, or arbitrary bytes.
  Each may be added only as a separate capability with its own policy and tests.

## Validation

Rust tests cover package validation, parsing, bounded loops, disabled eval,
input bounds, G-code reparsing, digest grants, and trust reset after updates.
Vitest covers declaration-to-slot mounting, capability denial, grouped machine
commands, and deterministic UI cleanup.
