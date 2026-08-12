# ADR 0053: Keep trusted UI plugins separate from external script packages

Date: 2026-08-13

## Context

Millo needs both rich first-party modules and community extensions. Treating
both as arbitrary JavaScript would allow a downloaded package to import Tauri,
retain actor-adjacent services, replace safety UI, or perform I/O outside the
capability review. Treating every built-in feature as declarative Rhai would
make complex React tools impractical and duplicate application logic in scripts.

## Decision

Millo has two explicit plugin tiers:

1. Trusted TypeScript plugins are compiled with the application. They import
   only `src/plugin-sdk`, receive scoped typed capabilities from `PluginHost`,
   and may register React contributions. They are reviewed and shipped as part
   of the Millo binary.
2. External `.millo-plugin` packages contain versioned JSON declarations and
   bounded Rhai. They render host-owned command UI and return exactly one typed
   action. They cannot inject React/HTML/JavaScript or access Tauri, files,
   network, serial or sender.

Both tiers use the same capability vocabulary where behavior overlaps. The
architecture test forbids trusted plugin production code from bypassing the SDK.
External grants are digest-bound, persisted transactionally, and serialized
against execution. Every capability proxy closes on unload; UI contributions
are isolated by an error boundary.

## Consequences

- Rich bundled tools remain modular without making downloaded code trusted.
- Community packages are less expressive than native JavaScript, but source and
  authority remain reviewable before motion.
- A new capability requires a typed core use case, both runtime contracts where
  applicable, lifecycle tests, and documentation. It cannot be introduced as a
  raw Tauri or serial escape hatch.
- Supporting signed native/JavaScript plugins later would be a third trust tier
  and requires a separate ADR, process isolation and signing policy.
