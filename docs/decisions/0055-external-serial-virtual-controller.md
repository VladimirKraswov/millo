# ADR 0055: External serial virtual controller

## Status

Accepted. Supersedes ADR 0054 for production application integration.

## Context

The built-in Mock transport used the production sender but still leaked a
simulation-specific transport kind, profile exception, IPC commands and UI into
Millo. It also made the controller lifetime equal to the desktop lifetime and
could not prove the native serial adapter boundary.

## Decision

The virtual machine is the standalone `millo-virtual-controller` process. It
owns a raw Unix PTY and the GRBL firmware model. A generic external-endpoint
registry lets `millo-serial` merge PTYs with native serial discovery on hosts
whose hardware APIs omit pseudo terminals.

The registration schema contains normal serial metadata only. The desktop has
one production transport kind, Serial, and no virtual-controller commands,
fault controls, synthetic profile, or in-process firmware dependency. VMC-3 is
identified only by product/serial metadata and its `$I` response. It follows the
same connect, inspect, onboard, Check, authorize and sender path as USB hardware.

The motion planner runs at wall-clock speed, derives axis rates and acceleration
from GRBL settings, interpolates lines and arcs, brakes for Hold/Cancel/end of
queue, and preserves speed across collinear feed blocks. Corners remain
conservative until a bounded junction-deviation model is justified by a fixture.

## Consequences

- Millo cannot tell whether the selected serial endpoint is physical or virtual.
- Serial discovery/opening and byte framing are covered by the virtual fixture.
- The controller can remain alive while Millo restarts or is upgraded.
- Fault injection remains a Rust test capability, not production UI.
- PTY registration is a host integration detail below the transport contract.
- The model validates software behavior, not mechanical or electrical safety.
