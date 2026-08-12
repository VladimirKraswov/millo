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
| Response watchdog | Response timeout is coupled to mutable UI/device processing | Treats correlated `ok` as progress and valid realtime status/message as liveness; only controller silence expires the watchdog | Dense planner queues, long moves, and dwell do not become false timeouts, while loss of controller or machine power still fails closed | Delayed-ack liveness fixture plus silent terminal-command timeout fixture; ADR 0028 |
| Error policy | Can show a dialog and continue when `Ignore` or global `ignoreErrors` is selected | Physical `error`, `ALARM`, timeout, reset, or link loss fails the exact line and requests Hold then Soft Reset | Buffered commands cannot silently continue after an invalid machine state | Correlated error/alarm/reset/disconnect fixtures |
| Failure contract | UI derives operator behavior from response text and mutable queue context | Terminal snapshots carry a typed failure kind, GRBL code, immutable source line and exact command; `lastError` remains compatibility text only | UI and plugins do not parse strings such as `Some(33)` and cannot lose the failed line after cleanup | Sender/actor fault assertions and TS read-model test |
| Completion | Detects the last row or M2/M30 and may complete from UI/device state | Waits for every FIFO acknowledgement, then a newly observed GRBL `Idle`; M2/M30 is deferred until the planner is already Idle | Prevents “job complete” while motion is still queued and avoids a terminal-command timeout | Physical 20 mm Air run; terminal barrier regression tests |
| Tool change | Sends `M6`, pauses from response handling, and resumes through mutable UI state | Compiles isolated `M6` into a host-only barrier, sends/acknowledges `Tn` first, drains the FIFO, freezes elapsed time, binds confirmation to source line/tool, then rechecks fresh `Idle`, Inspector and G54-G59 before continuing | GRBL 1.1 does not implement a physical tool changer; raw Resume cannot bypass operator verification and `M6` cannot become a firmware-dependent error | Parser/policy/sender/actor/UI tests; `grbl-tool-change-check.nc` |
| Check mode | Toggles `$C` through the general command path and shadows rows | Uses a serial-only typed `Idle -> Check -> Idle` lifecycle, Rust reparse from Program workspace, one outstanding line, exact failure attribution, validation-only M0/M1, host-only M2/M30, and mandatory cleanup | UI cannot inject a prepared plan; syntax validation cannot become motion, stall on an operator pause, trigger program-end side effects, or leave the controller accidentally in Check | Complex physical fixtures: 25/25 and 27/27 sender steps |
| Check-to-run evidence | Check mode and file Send are separate UI actions without a program/session-bound proof passed into Start | Successful Check mints a 15-minute certificate only after full acknowledgement and verified return to `Idle`; Cutting preflight requires the exact SHA-256, Optional Stop/Block Delete options, reset count, and reconnect count. M2/M30 is host-validated in Check, and a firmware-specific `$C`-exit reset is accepted only inside that exact controlled transition before another clean status | A user cannot Check one interpretation/file, change it or reconnect, then start Cutting under stale validation evidence; Check cannot leave a false reset blocker or trigger program-end side effects | Domain/actor tests; physical 27/27 Check and certificate preflight |
| Cutting-file validation | General sender accepts spindle syntax as part of the file | Typed Check plan accepts `M3/M4/S` under Cutting grammar while Air policy still rejects it | A real engraving file can be firmware-validated without activating motion/spindle | `grbl-cutting-check.nc`; physical 27/27 sender-step Check pass |
| Source semantics | Preprocessing is spread across Qt regex helpers and sender code | Validates decimal XOR checksums over untouched source bytes before normalization; models `/` and `M1` as explicit Block Delete/Optional Stop options; reparses modal state and preview; binds both options to preflight and the one-use lease | A corrupted block cannot execute, and a preview made with one optional-program interpretation cannot authorize another | Parser/policy/lease/actor tests; `grbl-stream-semantics-check.nc` |
| Controller dialect | Broad input acceptance can defer unsupported commands to GRBL | Parser dialect is narrowed by physical Check evidence; `G61` is accepted and `G64` is blocked | A known-incompatible command cannot stop a real job after motion has begun | `grbl-path-control-check.nc` plus parser regression |
| Program authorization | File Send begins directly from the UI action | Serial motion requires fresh status + Inspector, hardware/profile policy, intent-specific checklist, and a short-lived program/position/session-bound one-use lease | A stale preview or plugin/UI call cannot start a different program or machine state | `millo-run` lease tests and successful physical Air run |
| Runtime overrides | UI sends protocol bytes while reconciling slider state | Domain enums map to a closed GRBL byte set inside the controller; applied values return through typed `Ov:` telemetry | UI and future plugins cannot choose arbitrary realtime bytes | Mock preemption tests; physical `110/50/99 -> 100/100/100` smoke |
| Runtime timing | `TimeEstimator` is integrated with Candle's UI/parser and includes planner-oriented estimation | Per-line integer timing is embedded in the immutable Rust plan; snapshots expose active elapsed and explicit complete/lower-bound ETA, exclude Hold, and freeze every terminal state | Timing survives UI replacement and cannot show paused wall time or a silently “exact” rapid estimate | Plan/sender timing tests and TS read-model tests |
| Shutdown contract | End behavior may depend on file content and mutable configured end commands | Every plan injects typed `M5` then `M9` after source lines and before deferred M2/M30; completion still requires fresh `Idle`; snapshot records shutdown acknowledgements | A successful file that omitted output-off commands cannot be reported complete without issuing and correlating both safe-output commands | Plan/sender/actor tests; physical 10/10 Check fixture |
| Large-job progress | Issue reports describe pauses, memory pressure, UI freezes, and silent mid-job stops | Plan storage is bounded, in-flight state is bounded by controller RX bytes, snapshots are O(1), and each `ok` publishes sequence/line/age telemetry; 100,000 lines are a regression fixture | Prevents queue-sized response/UI state and makes a lack of progress observable instead of looking like a running job | 100k stress test; heartbeat read-model test |
| Crash diagnostics | Mutable UI/queue state may be lost with the process, while send-from-line offers insufficient physical proof | `millo-journal` keeps 100 run records, throttled exact-ACK checkpoints, typed failures, shutdown evidence, atomic temp/backup JSON, and an explicit non-executable journal disposition | A crash or unexplained stop leaves bounded forensic evidence without treating buffered acceptance as completed cutting | Journal throttle, failure, bound, persistence and backup-recovery tests |
| Power-loss restart | Send-from-selected-line relies on operator-selected source position and reconstructed parser state | Start is two-phase: exact source/machine/options are synced before dispatch; physical `Ln:` is persisted separately from `ok`; link loss quarantines the sender; recovery explicitly selects bounded checkpoint replay or a full restart for missing/uncertain motion power, then uses ordinary Check/preflight/authorization | Avoids lost crash context, silent reconnect continuation, blind exact-line resume, and false trust in USB-powered GRBL when motor power is absent | `millo-command`, `millo-recovery`, atomic-store fixtures, recovery UI/model checks; ADR 0038 |
| Testability | Sender behavior depends heavily on `frmMain`, Qt widgets, timers, and dialogs | Parser, policy, lease, sender, actor, GRBL parser, Mock transport, and serial adapter are separate crates | Every state transition can be regression-tested without a window or physical machine | `npm run verify`; deterministic Mock fault injection |

## Deliberate non-parity

The following Candle features are not accepted as raw sender behavior in Millo:

- `M6` is supported only as the dedicated host-managed Cutting workflow above.
  It remains forbidden in Air runs and must be isolated from motion, spindle,
  coolant, and coordinate words. `G38.x` probing, `G28/G30/G53` movement,
  `G10/G92` coordinate mutation, and `M7/M8` coolant still require dedicated
  typed workflows with hardware/profile checks.
- Candle's global `Ignore error responses` setting is not copied. Continuing a
  buffered physical stream after a rejected line is ambiguous and can move from
  an invalid modal or coordinate state.
- Arbitrary start/end strings and raw console/plugin writes are not part of the
  execution capability. Future customization must compile into a reviewed,
  typed plan and preserve authorization.
- Candle-style arbitrary send-from-selected-line and `autoLine` modal
  reconstruction are not exposed. Millo offers only the guarded interrupted-job
  workflow: matching source and controller, physical `Ln:` evidence,
  conservative rewind, explicit Safe Z, restored work reference, preview,
  Check, and a new program-bound authorization. Prepending remembered G words
  alone cannot establish those facts.
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
