# Sender hardening

This document maps field reports and the official GRBL interface contract to
concrete Millo behavior. Candle issues are evidence of observed symptoms, not
proof of a single root cause. The implementation follows GRBL's protocol rather
than attempting to reproduce issue-specific workarounds.

## Research matrix

| Report or protocol constraint | Risk | Millo response | Verification |
| --- | --- | --- | --- |
| [Candle #647](https://github.com/Denvi/Candle/issues/647): a 138 KB / 6,424-line FluidNC job reportedly stopped mid-run without a console error | A sender may appear active after acknowledgements stop and leave no useful post-crash evidence | Per-line response deadline, typed terminal failure, monotonically numbered `ok` heartbeat with source line and age, plus throttled persistent checkpoints; no silent continue policy | Stall/timeout/disconnect actor fixtures; heartbeat and journal tests |
| [Candle #205](https://github.com/Denvi/Candle/issues/205): a 91,000-line job reportedly developed pauses with exhausted host RAM and laser burn risk | Queue-sized UI/response objects can starve streaming or memory | Immutable bounded plan, RX-byte-bounded `VecDeque`, virtualized table, O(1) snapshots | 100,000-line sender regression; peak FIFO below 32 |
| [Candle #514](https://github.com/Denvi/Candle/issues/514): UI freeze while buffered motion continued, then spindle remained running | UI ownership and buffered motion can outlive visible application state | Rust actor owns serial independently of React; response faults request Hold + Soft Reset; normal plans issue M5/M9 and wait for fresh Idle | Delayed/fault actor fixtures; typed shutdown-tail Check 10/10 |
| [Candle #515](https://github.com/Denvi/Candle/issues/515): M00 could not be resumed | Program barriers need explicit lifecycle state | Isolated M0 is an empty-FIFO Paused state; only typed Resume from observed Hold/Idle continues | Sender and actor Hold/Resume tests |
| [Candle #464](https://github.com/Denvi/Candle/issues/464): request to run from an arbitrary selected line | Re-entering midway with the wrong modal/position state can crash a tool | Raw send-from-line is deliberately absent; the journal preserves diagnostic checkpoints but labels failed/cancelled runs `RestartBlocked` until position, modal state, safe approach, and a new authorization can be proven | Journal recovery tests; no executable resume token exists |
| [Candle #684](https://github.com/Denvi/Candle/issues/684): tool-change workflows need controlled intervention | M6 sent as ordinary G-code cannot prove tool, Z zero, or planner drain | Host-only M6 barrier, bounded Tn, line/tool-bound confirmation, fresh Inspector and Idle before continuation | Mock actor/UI tests; physical Check fixture |
| [Official GRBL interface](https://github.com/gnea/grbl/blob/master/doc/markdown/interface.md): push messages are not responses, character counting has an error reservation, EEPROM writes must use send-response, status should be limited, and Check mode is recommended before streaming | Naive FIFO accounting can steal responses, overflow RX, or continue buffered commands after an error | Typed frame demultiplexing; actual `[OPT] RX - 1`; status outside response FIFO; settings outside file stream; Check mode; Hold + Reset on physical stream fault | Interleaved-status tests, capacity fixtures, settings actor tests, physical Check |

## Stronger-than-Candle contract

- One actor owns every transport byte. React and plugins cannot read or write
  serial directly.
- Original source is reparsed in Rust at preview, preflight, Check, and Start.
  Optional semantics and checksums are bound before authorization.
- A 30-second one-use lease binds intent, source fingerprint, execution options,
  controller session, position, and inspected RX capacity.
- Cutting preflight additionally requires a 15-minute GRBL Check certificate.
  The actor issues it only after every line is acknowledged, `$C` cleanup is
  verified, and the controller reports `Idle`; it is bound to the exact source,
  Optional Stop/Block Delete interpretation, reset count, and reconnect count.
- M2/M30 is parser/policy-validated but host-acknowledged in Check mode, so a
  validation run cannot trigger firmware-specific program-end side effects.
  The current GRBL build emits one reset banner while disabling `$C`; Millo
  accepts and clears only the single new banner created inside that verified
  `Check -> Idle` transition, then requires another clean status before issue.
- FIFO bytes include newline and are released only by the exact oldest `ok` or
  typed failure. Status and push messages never consume a line.
- Physical error, alarm, timeout, reset, invalid state, or disconnect stops at
  the exact source line and requests Hold then Soft Reset; there is no Ignore
  Errors switch.
- Operator Soft Reset validates and atomically consumes its short-lived
  challenge before changing sender state. A stale confirmation cannot cancel
  only the host side while GRBL keeps running, and transport replacement is
  rejected until the controller is disconnected.
- M0, optional M1, M6, and M2/M30 are first-class host barriers rather than UI
  side effects.
- M5/M9 is both a typed preamble and typed shutdown epilogue. Completion requires
  every acknowledgement and fresh physical Idle.
- Timing and progress are backend evidence. Hold/tool-change wall time is
  excluded, terminal values freeze, and incomplete rapid estimates remain
  labelled lower bounds.
- Each sender load receives a monotonic process-local run sequence. A bounded
  `millo-journal` store records start, state transitions, checkpoints at most
  every 250 acknowledgements or two seconds, and every terminal state. Atomic
  temp/backup replacement keeps the preceding valid JSON checkpoint available.
- Journal persistence runs on a dedicated bounded worker outside Tokio. Slow
  storage and `fsync` cannot stall the controller actor; a corrupt primary is
  recovered from its backup, while two corrupt copies produce an explicit load
  error instead of silently presenting an empty history.
- Controller, profile, and validated-setting invariants fail as typed errors at
  their boundary. Unexpected internal state is diagnosable without terminating
  an active desktop process through `expect`.
- Journal checkpoints are diagnostic evidence, never executable continuation
  leases. A failed or cancelled run explicitly records `RestartBlocked`.
- Millo replaces file line numbers with source-line `N` tags on the wire and
  retains GRBL's optional `Ln:` execution report independently from `ok`.
  Recovery therefore never mistakes buffered acceptance for physical progress;
  firmware without `Ln:` cannot produce an automatic execution checkpoint.

## Deliberate boundaries

The current production target is GRBL 1.1 over serial. FluidNC Telnet/WebSocket,
SD-card execution, and controller-specific protocols require separate transport
capabilities and fixtures; this sender does not claim them through GRBL-shaped
guessing. Probe cycles, heightmaps, coolant, reference/machine-coordinate moves,
tool-length offsets, and partial-file restart also remain separate hardware-aware
workflows. Their absence is preferable to exposing raw commands that bypass the
authorization contract.

## Physical certificate evidence

On 2026-08-12, `grbl-cutting-check.nc` completed 27/27 sender steps on GRBL
`1.1f.20230316`. The plan covered 28 source lines, eight motions and 158.083 mm
of cutting geometry. M30 was host-validated, GRBL returned to `Idle`, the
controlled `$C`-exit banner was isolated, and the immediately repeated Cutting
preflight accepted the exact program/session certificate.
