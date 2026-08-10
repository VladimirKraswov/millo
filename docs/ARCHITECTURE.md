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
                        | command arbiter  |
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
GRBL. The command actor owns the controller and active transport; the controller
owns protocol state and the current snapshot. Tauri only converts application
calls into commands and events.

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

The desktop command `connect_transport` selects either a deterministic mock or a
discovered native serial port, sends the GRBL realtime `?` byte, parses the returned status frame, updates
`ControllerSnapshot`, emits `machine-state`, and returns the same snapshot to the
caller. The UI treats the event as authoritative.

The command arbiter owns the periodic driver. Every tick calls one core method:
a connected controller is polled, while a recovering controller attempts
disconnect/connect/status synchronization. Tauri subscribes to snapshots and
does not schedule or execute controller I/O.

### Lifecycle invariants

- Only one actor task owns controller I/O and periodic polling.
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
- Disconnect changes actor lifecycle state, closes the transport, and clears
  session telemetry; the dormant ticker performs no I/O while disconnected.

### Native serial boundary

- `millo-serial` owns OS port discovery, baud configuration, asynchronous byte
  writes, and CR/LF line framing.
- It implements the same `Transport` contract as `millo-mock`; neither serial
  nor mock parses GRBL or changes machine state.
- The command actor stores `Controller<BoxedTransport>`, so transport selection
  changes construction only, not controller policy.
- Serial targets are checked against fresh native discovery before opening.
- The default UI filter uses only discovery metadata: USB transport kind,
  GRBL/CNC/FluidNC names, common board and USB-UART names, and known vendor IDs.
  It is intentionally advisory and can be disabled to expose every port.
- EOF and pre-connect I/O become `TransportError::NotConnected`; platform I/O
  failures preserve their message as `TransportError::Io`.
- Reconnection drops and reopens the native handle through the existing
  controller lifecycle.
- Physical hardware is not required by the automated suite. Native enumeration
  and an operator hardware smoke test cover the OS boundary.
- macOS `/dev/cu.*` and `/dev/tty.*` aliases with the same device suffix are
  deduplicated in `millo-serial`; the callout (`cu`) path wins before descriptors
  reach Tauri.

### Command and inspection boundary

- A bounded FIFO channel is the only path to the active controller.
- Realtime bytes and newline-terminated commands are distinct request types.
- Status `?` consumes its matching status frame before another request runs.
- Device Inspector permits only `$I`, `$$`, `$G`, and `$#`.
- `ok`, `error:n`, `ALARM:n`, and reset terminate and classify the active line
  request; asynchronous status/reset information still updates the snapshot.
- Rust parses firmware, settings, modal state, and coordinate parameters. The UI
  never receives a responsibility to interpret wire lines.
- Tauri exposes no raw command, G-code motion, or spindle-control endpoint.

## Near-term sequence

1. Safety controls and hardware-profile validation.
2. Short-distance step jog with explicit no-homing constraints.
3. Work coordinates and touch-probe validation.
4. Command queue and sender state machine.
5. G-code domain, parser fixtures, and program model.
6. Visualization read model and Three.js adapter.

Ant Design and Three.js are intentionally absent from the first slice. They will
be added when the first operator workflow and visualizer require them, keeping
the initial dependency surface small.
