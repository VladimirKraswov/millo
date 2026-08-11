# Millo sender compared with Candle

This document compares execution-core behavior, not UI appearance or total
application feature count. Candle is the compatibility reference; Millo keeps
useful GRBL behavior while replacing unsafe coupling with explicit Rust use
cases. Source references describe the audited Candle checkout in
`/Volumes/Extend/work/Candle`.

## Improvements already implemented

| Area | Candle behavior | Millo behavior | Problem solved | Evidence |
| --- | --- | --- | --- | --- |
| Serial ownership | `frmMain` owns mutable `m_commands` and `m_queue`; UI actions, polling, scripts, and file transfer all call `sendCommand` | One Rust actor owns the transport; every line, query, status byte, Hold, Reset, and override crosses the same queue | Prevents competing readers/writers and response theft | `millo-command` actor tests; ADR 0006 and 0028 |
| RX capacity | Uses the compile-time `BUFFERLENGTH` character window | Reads `[OPT]`, reserves one byte, validates `1..4095`, and accounts for UTF-8 command bytes plus newline | Uses the actual controller buffer without overflow or unnecessary fixed throttling | 127/255-byte sender tests; physical `[OPT:VMZHL,35,254]` selects 253 bytes |
| Response correlation | Removes the first queued command when a terminal-looking response arrives | Keeps a typed oldest-command FIFO and classifies status separately from `ok`, `error`, `ALARM`, and reset | An interleaved `<Status>` cannot acknowledge the wrong G-code line | Delayed `status -> ok` fixtures; physical 25/25 and 24/24 Check runs |
| Realtime responsiveness | Processing is intertwined with the UI response callback | Splits program reads into 10 ms slices and prioritizes actor requests and polling | Hold, Reset, and overrides do not wait for a long command timeout | Hold-preemption and override-preemption tests; ADR 0028 |
| Error policy | Can show a dialog and continue when `Ignore` or global `ignoreErrors` is selected | Physical `error`, `ALARM`, timeout, reset, or link loss fails the exact line and requests Hold then Soft Reset | Buffered commands cannot silently continue after an invalid machine state | Correlated error/alarm/reset/disconnect fixtures |
| Failure contract | UI derives operator behavior from response text and mutable queue context | Terminal snapshots carry a typed failure kind, GRBL code, immutable source line and exact command; `lastError` remains compatibility text only | UI and plugins do not parse strings such as `Some(33)` and cannot lose the failed line after cleanup | Sender/actor fault assertions and TS read-model test |
| Completion | Detects the last row or M2/M30 and may complete from UI/device state | Waits for every FIFO acknowledgement, then a newly observed GRBL `Idle`; M2/M30 is deferred until the planner is already Idle | Prevents “job complete” while motion is still queued and avoids a terminal-command timeout | Physical 20 mm Air run; terminal barrier regression tests |
| Check mode | Toggles `$C` through the general command path and shadows rows | Uses a serial-only typed `Idle -> Check -> Idle` lifecycle, Rust reparse from Program workspace, one outstanding line, exact failure attribution, validation-only M0/M1, and mandatory cleanup | UI cannot inject a prepared plan; syntax validation cannot become motion, stall on an operator pause, or leave the controller accidentally in Check | Complex physical fixtures: 25/25 and 26/26 |
| Cutting-file validation | General sender accepts spindle syntax as part of the file | Typed Check plan accepts `M3/M4/S` under Cutting grammar while Air policy still rejects it | A real engraving file can be firmware-validated without activating motion/spindle | `grbl-cutting-check.nc`; physical 26/26 Check pass |
| Source semantics | Preprocessing is spread across Qt regex helpers and sender code | Parser retains source lines and fails closed on unsupported `/` and `*checksum`; metadata-only `O` headers are omitted, mixed `O` blocks are rejected | Normalization cannot turn an optional/corrupted block into an unconditional command | Parser fixtures for optional block, checksum, and O headers |
| Controller dialect | Broad input acceptance can defer unsupported commands to GRBL | Parser dialect is narrowed by physical Check evidence; `G61` is accepted and `G64` is blocked | A known-incompatible command cannot stop a real job after motion has begun | `grbl-path-control-check.nc` plus parser regression |
| Program authorization | File Send begins directly from the UI action | Serial motion requires fresh status + Inspector, hardware/profile policy, intent-specific checklist, and a short-lived program/position/session-bound one-use lease | A stale preview or plugin/UI call cannot start a different program or machine state | `millo-run` lease tests and successful physical Air run |
| Runtime overrides | UI sends protocol bytes while reconciling slider state | Domain enums map to a closed GRBL byte set inside the controller; applied values return through typed `Ov:` telemetry | UI and future plugins cannot choose arbitrary realtime bytes | Mock preemption tests; physical `110/50/99 -> 100/100/100` smoke |
| Runtime timing | `TimeEstimator` is integrated with Candle's UI/parser and includes planner-oriented estimation | Per-line integer timing is embedded in the immutable Rust plan; snapshots expose active elapsed and explicit complete/lower-bound ETA, exclude Hold, and freeze every terminal state | Timing survives UI replacement and cannot show paused wall time or a silently “exact” rapid estimate | Plan/sender timing tests and TS read-model tests |
| Testability | Sender behavior depends heavily on `frmMain`, Qt widgets, timers, and dialogs | Parser, policy, lease, sender, actor, GRBL parser, Mock transport, and serial adapter are separate crates | Every state transition can be regression-tested without a window or physical machine | `npm run verify`; deterministic Mock fault injection |

## Deliberate non-parity

The following Candle features are not accepted as raw sender behavior in Millo:

- `M6` tool change, `G38.x` probing, `G28/G30/G53` movement, `G10/G92`
  coordinate mutation, and `M7/M8` coolant require dedicated typed workflows
  with hardware/profile checks. Until those workflows exist, parser policy
  blocks them before a plan can be created.
- Candle's global `Ignore error responses` setting is not copied. Continuing a
  buffered physical stream after a rejected line is ambiguous and can move from
  an invalid modal or coordinate state.
- Arbitrary start/end strings and raw console/plugin writes are not part of the
  execution capability. Future customization must compile into a reviewed,
  typed plan and preserve authorization.
- Parser-state restoration after Soft Reset is not attempted. Reset terminates
  the lease and run; restoring only modal words would not prove position,
  planner contents, workholding, or spindle state.

These are capability boundaries, not forgotten parser cases. Add each one as a
small hardware-aware use case with fixtures before allowing it into production
execution.

Candle's planner/acceleration-aware `TimeEstimator` remains a useful reference
and can be more physically accurate than Millo's current feed+dwell lower bound.
Millo does not claim parity there yet; future acceleration modeling belongs in a
separate estimator crate and must preserve the snapshot completeness flag.

## Candle locations audited

- `src/candle/frmmain.cpp:2279-2570`: response queue, errors, pauses, M6.
- `src/candle/frmmain.cpp:4437-4535`: `sendCommand` and RX-window filling.
- `src/candle/frmmain.cpp:949-979`: start, pause, and abort actions.
- `src/candle/frmmain.cpp:5972`: configurable end commands.
- `src/candle/parser/gcodeparser.cpp` and
  `src/candle/parser/gcodepreprocessorutils.cpp`: parser/preprocessor behavior.

The comparison should be updated whenever a new execution capability is
enabled, not only when UI controls are added.
