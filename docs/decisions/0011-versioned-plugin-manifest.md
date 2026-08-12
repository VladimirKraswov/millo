# ADR 0011: Version plugin manifests and gate host capabilities

## Status

Accepted on 2026-08-11.

Extended by ADR 0050 on 2026-08-12 for sandboxed external source packages.

## Context

The extension registry proves that UI contributions can be inserted, replaced,
and removed by owner. A plugin host additionally needs a stable compatibility
contract and explicit authority boundaries before any plugin activation occurs.
Loading code first and checking permissions later would let an incompatible or
overprivileged plugin run outside those boundaries.

## Decision

Millo introduces `PluginManifestV1` with independent `manifestVersion` and
`apiVersion` fields, both currently `1`. A manifest contains a stable plugin ID,
display name, plugin version, and distinct required and optional capability
lists. IDs use lowercase dot-separated namespaces and plugin versions use a
SemVer-compatible form.

The v1 capability catalog is:

- `ui.contribute`: add, replace, and remove owned UI contributions.
- `machine.read`: observe typed machine state through immutable snapshots and
  tracked subscriptions.
- `machine.jog`: invoke the existing typed guarded-jog use case.
- `jobs.create`: create versioned jobs; reserved, not yet implemented.

Activation authority is the intersection of manifest requests, explicit host
grants, and capabilities implemented by the host. A missing required capability
rejects the plugin before activation. A missing optional capability is reported
and omitted. Unknown capabilities, duplicate declarations, overlap between
required and optional lists, unsupported manifest versions, and incompatible API
versions are rejected.

The normalized manifest and granted capability list are immutable. Activation
receives only narrow capability proxies. The UI proxy fixes the owner to the
plugin ID and requires contribution IDs in that owner's namespace. The guarded
jog proxy delegates to `MachineCommandGateway`, retaining all Rust-side safety
checks. The read proxy delegates to `MachineStateSource` and cannot initiate
controller I/O. Plugins receive no serial handle, Tauri API, raw GRBL endpoint,
or shell UI context.

This slice uses only in-memory modules linked into the application. It does not
read external manifests or assets and does not dynamically execute third-party
code. A built-in fixture plugin exists only for host regression tests.

## Consequences

- Compatibility and requested authority are inspectable before activation.
- Optional permissions support graceful degradation without silently granting
  access.
- Activation failures roll back partial UI registration; unload removes all UI
  owned by the plugin even when its deactivation callback fails.
- `machine.read` fails closed unless a typed state source is wired into the host;
  `jobs.create` remains unavailable until its service is implemented.
- Persistent user grants, package verification, process isolation, and external
  code loading require separate decisions before third-party plugins are enabled.

## Validation

Unit tests cover valid and invalid manifests, deterministic grants, required and
optional capability behavior, API mismatch rejection, proxy exposure, namespace
enforcement, activation rollback, and unload cleanup. The test plugin replaces
`core.jog-pad`; unloading it reveals the original core contribution again.
