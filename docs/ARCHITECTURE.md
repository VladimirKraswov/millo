# Architecture

## Direction of dependencies

```text
                        +------------------+
                        |   React / TS UI   |
                        +---------+--------+
                                  |
                           typed IPC/events
                                  |
                        +---------v--------+
                        |  Tauri adapter    |
                        +---------+--------+
                                  |
                        +---------v--------+
                        | controller       |
                        +----+---------+---+
                             |         |
                    +--------v--+   +--v-----------+
                    | GRBL      |   | Transport    |
                    +-----+-----+   +------+-------+
                          |                |
                    +-----v-----+   +------v-------+
                    | Domain    |   | Mock / Serial|
                    +-----------+   +--------------+
```

Dependencies point inward. Domain types do not import Tauri, an I/O library, or
a controller protocol. A transport moves bytes and lines; it does not interpret
GRBL. The controller orchestrates both and owns the current snapshot. Tauri only
converts application calls into commands and events.

## Rules

1. CNC behavior belongs in Rust and must be testable without Tauri.
2. The UI never parses controller wire messages.
3. Protocol crates never open ports or sockets.
4. Transport implementations never mutate machine state.
5. New compatibility behavior starts as a fixture or failing test.
6. Public domain types are serializable, explicit, and preserve unknown input.
7. Safety-critical actions will be modeled as state transitions, not raw UI
   strings.

## First vertical slice

The desktop command `connect_mock` connects a deterministic transport, sends the
GRBL realtime `?` byte, parses the returned status frame, updates
`ControllerSnapshot`, emits `machine-state`, and returns the same snapshot to the
caller. The UI treats the event as authoritative.

This slice proves the dependency direction before serial I/O, G-code streaming,
or visualization add concurrency and safety requirements.

## Near-term sequence

1. Controller lifecycle: reset banner, polling, disconnect, alarm and timeout.
2. Native serial transport with the same `Transport` contract.
3. Command queue and sender state machine.
4. G-code domain, parser fixtures, and program model.
5. Visualization read model and Three.js adapter.

Ant Design and Three.js are intentionally absent from the first slice. They will
be added when the first operator workflow and visualizer require them, keeping
the initial dependency surface small.
