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

## Implemented vertical slices

The desktop command `connect_mock` connects a deterministic transport, sends the
GRBL realtime `?` byte, parses the returned status frame, updates
`ControllerSnapshot`, emits `machine-state`, and returns the same snapshot to the
caller. The UI treats the event as authoritative.

The lifecycle slice adds a periodic driver in the Tauri adapter. Every tick calls
one core method: a connected controller is polled, while a recovering controller
attempts disconnect/connect/status synchronization. The adapter owns scheduling;
the core owns every state transition.

### Lifecycle invariants

- Only one polling task exists for an active desktop connection.
- Poll ticks use skip semantics and never build a backlog.
- Every status transaction has a bounded timeout.
- A successful transport connection starts polling even if the initial status
  synchronization fails.
- A single failed poll is transient; the configured failure threshold moves the
  controller to `Recovering`.
- `NotConnected` moves to recovery immediately.
- Recovery succeeds only after reconnect and a valid status frame.
- A reset banner clears stale machine/alarm state and remains visible until the
  operator acknowledges it.
- An `ALARM:n` line and `<Alarm|...>` status are machine state, not transport
  failure. Alarm clears only after a non-alarm status arrives.
- Disconnect stops polling before closing the transport and clears session
  telemetry.

## Near-term sequence

1. Native serial transport with the same `Transport` contract.
2. Command queue and sender state machine.
3. G-code domain, parser fixtures, and program model.
4. Visualization read model and Three.js adapter.

Ant Design and Three.js are intentionally absent from the first slice. They will
be added when the first operator workflow and visualizer require them, keeping
the initial dependency surface small.
