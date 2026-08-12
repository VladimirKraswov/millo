# ADR 0046: Core tool library and surfacing plugin

## Status

Accepted.

## Decision

Cutting-tool geometry and recommendations belong to the Rust application core,
not to an individual machine profile or plugin. `millo-tooling` owns validated,
atomically persisted CRUD. Plugins receive only deeply frozen `tools.read`
snapshots and tracked subscriptions.

Spoilboard surfacing is the second bundled plugin. Its React surface selects a
core tool and requests a typed plan, while `millo-cam` owns raster generation,
bounds, spindle-free output, and parser validation. The plugin cannot fabricate
tool geometry or reach machine control.

Bundled job creators share one compact `Создать` menu. The system tool library
lives under machine settings so infrequent configuration does not crowd the
operator header.

## Consequences

- Future plugins reuse one trusted tool model and CAM service.
- Unloading a plugin removes its UI and tool subscriptions deterministically.
- Preset research and Russian guidance remain inspectable and editable.
- A generated surfacing job still passes the ordinary sender safety workflow.
