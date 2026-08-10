# ADR 0004: Native serial behind the transport contract

- Status: accepted
- Date: 2026-08-10

## Context

Millo needs to discover and open native GRBL serial devices without coupling OS
I/O to the controller state machine. Runtime selection must retain the mock
transport for deterministic development and must reuse lifecycle timeout and
recovery behavior unchanged.

## Decision

Create `millo-serial` as a leaf adapter built on `tokio-serial`. It owns native
port discovery, validated port/baud configuration, asynchronous writes, and
newline-delimited reads. It implements `millo_transport::Transport` and maps OS
errors into the transport error vocabulary.

Use `Controller<BoxedTransport>` in the Tauri session. Selecting a transport
constructs a new controller session with either `MockTransport` or
`SerialTransport`; polling and recovery continue to call the same controller
methods. Tauri exposes serial metadata to TypeScript but does not expose native
handles.

Port discovery also calculates an advisory `likely_grbl` flag from USB type,
vendor IDs, and descriptive metadata. The UI enables this filter by default but
always lets the operator reveal all ports. The heuristic never replaces a GRBL
protocol exchange.

On macOS, discovery deduplicates `/dev/cu.<device>` and `/dev/tty.<device>` as
two aliases of the same interface and retains the callout path. Other platforms
and unpaired paths are preserved verbatim.

This slice permits only realtime GRBL status polling. Buffered G-code writes,
flow control, cancellation, and sender recovery will be introduced together in
the command queue slice.

## Consequences

- Core lifecycle tests remain hardware-independent.
- Native serial and mock exercise identical GRBL parsing and state transitions.
- Device permissions, unplug behavior, and driver-specific failures stay at one
  adapter boundary.
- A disconnected serial target is rediscovered before an explicit connection;
  automatic lifecycle recovery reopens the configured native path.
- Hardware smoke testing remains necessary because CI cannot reproduce every
  USB chipset and operating-system driver.
