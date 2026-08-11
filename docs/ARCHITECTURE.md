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
                        +----+--------+----+-+
                             |        |    |
                      +------v--+ +---v--+ +v----------+
                      |readiness| |safety| |controller |
                      +----+----+ +---+--+ +--+------+--+
                           |          |       |      |
                      +----v----------v-+ +---v---+ +v---------+
                      | Domain          | | GRBL  | |Transport |
                      +-----------------+ +---+---+ +----+-----+
                                              |          |
                                          +---v---+ +----v-----+
                                          |Domain | |Mock/Serial|
                                          +-------+ +-----------+
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
- Tauri exposes no raw command, general G-code, or spindle-control endpoint. Its
  sole motion call is the typed, actor-authorized step-jog operation described
  below.

### Hardware readiness boundary

- `millo-readiness` evaluates parsed domain data; it neither sends GRBL commands
  nor imports Tauri or a transport implementation.
- The selected hardware profile is explicit: XYZ, manual spindle, no homing,
  no limit switches, and no physical emergency stop.
- Missing or invalid axis steps, rates, acceleration, travel, firmware identity,
  required query responses, milling mode, or profile-consistent `$20/$21/$22`
  settings block the future guarded test jog.
- Unhomed coordinates, manual spindle operation, missing emergency stop, active
  `G91`, and an electrically untested probe remain visible cautions.
- `testJogReady` is an inspection result, not a general motion permission. The
  step-jog command re-checks live controller state inside the command actor
  immediately before writing bytes.
- React discards the displayed report when the live snapshot leaves stable
  `Connected + Idle`, receives an alarm, or receives a reset notice. The
  Inspector must be read again after recovery.
- Probe readiness stays false until a separate stationary electrical test is
  implemented and passed.

### Realtime safety boundary

- `millo-safety` owns actor-local reset challenges and test-jog authorization;
  React state cannot authorize a controller write by itself.
- Feed Hold is encoded as the GRBL realtime `!` byte. It is exposed as a named
  Tauri command, never as a raw byte or arbitrary G-code endpoint.
- Soft Reset requires an actor-issued challenge that expires after 10 seconds.
  Confirmation consumes the challenge before `Ctrl-X` is written, so retrying
  the same confirmation cannot reset the controller twice.
- Test-jog preflight requires explicit confirmation that the spindle is off,
  the tool is clear, and machine power is within operator reach. The actor then
  re-runs all four Inspector queries and assesses the resulting live snapshot.
- A successful preflight creates a 15-second single-use lease. Alarm, reset,
  reconnect, disconnect, non-idle state, expiry, or another realtime command
  invalidates it.
- The typed step-jog endpoint consumes the lease inside the actor before the
  controller validates and writes the command. Validation or transport failure
  does not restore the lease.
- The GRBL encoder always emits `$J=G91 G21` with exactly one of X/Y/Z. Absolute
  distance is limited to `0.01..1.00 mm`; feed is limited to
  `10..100 mm/min`. UI values cannot widen this backend envelope.
- A successful `ok` means GRBL accepted the jog for execution; periodic status
  remains authoritative for `Jog` and final position. The UI clears its lease
  before awaiting the response.
- Jog Cancel is the named `0x85` realtime operation and is accepted by the actor
  only while its current snapshot reports `Jog`.
- First-machine setup exposes no general `$n=value` endpoint. A narrow
  actor-only operation may disable `$21` hard limits and `$22` homing while
  stable `Idle`; it reads settings before and after, skips values already zero,
  and fails verification unless both final values are exactly `0`.
- The single actor remains the only writer to the port. A realtime byte is
  serialized behind an already active controller transaction; future sender
  work must preserve bounded command transactions and provide priority handling
  between streamed lines.

## Near-term sequence

1. Work coordinates and stationary touch-probe electrical validation.
2. Guarded Z probing and probe-result capture.
3. Command queue and sender state machine.
4. G-code domain, parser fixtures, and program model.
5. Visualization read model and Three.js adapter.

Ant Design and Three.js are intentionally absent from the first slice. They will
be added when the first operator workflow and visualizer require them, keeping
the initial dependency surface small.
