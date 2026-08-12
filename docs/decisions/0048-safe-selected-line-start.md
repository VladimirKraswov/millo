# 0048. Safe selected-line start

Status: accepted

## Problem

Long jobs need a way to repeat a damaged or missed region without running the
whole file again. Sending a file literally from an arbitrary source line is not
safe: the controller may be missing units, distance mode, arc mode, plane,
feed, WCS, selected tool, spindle state, and the physical entry move that made
the original cut valid.

Candle's `From Line` workflow is useful, but Millo must retain its stronger
sender invariants and must not turn line selection into an unrestricted stream
offset.

## Decision

Millo treats selected-line execution as compilation of a new immutable program:

1. The operator selects a motion segment in the preview or line table.
2. The native `millo-restart` planner finds the latest preceding rapid segment
   whose entry is at the program clearance height. If no such entry can be
   proven, planning fails closed.
3. The planner emits an explicit safety preamble, raises Z, moves XY at Safe Z,
   restores the anchor Z, WCS, units, distance and arc modes, feed mode, plane,
   motion mode, feed, selected tool, spindle speed, and spindle direction.
4. A selected line inside an active cutting pass is never resumed in place. The
   planner rewinds to the safe entry and reports how many executable lines will
   be repeated.
5. Air run omits spindle activation and host-managed `M6`; cutting retains the
   original tool and spindle contract.
6. The generated remainder is reparsed and passed through the normal run
   policy before it reaches the UI.
7. The generated program has its own source name and fingerprint. It must pass
   a fresh GRBL `$C` run; the resulting certificate is bound to that exact
   generated source and execution options.
8. Normal preflight, one-use operator authorization, sender correlation,
   completion draining, audit logging, and crash-recovery journaling remain in
   force. There is no alternate sender path.

## UX

- Clicking a preview segment selects its source line.
- `С этого участка` opens a compact Safe Z dialog.
- After planning, a persistent banner shows selected line, actual restart line,
  repeated line count, WCS, tool, and the required GRBL Check.
- GRBL Check starts immediately after the generated remainder is parsed. A
  safe-start program requires its exact certificate for Air and Cutting modes.
- `Вся программа` restores the original loaded file before execution begins.
- Air/Cutting intent is fixed after compilation because it controls whether the
  reconstructed setup may contain tool-change and spindle state.

## Consequences

- Partial repetition is slower than blindly seeking to a line, because a safe
  lead-in may be replayed and Check is mandatory.
- It is deterministic and inspectable: the exact generated G-code is parsed,
  previewed, certified, journaled, and recoverable like any other Millo job.
- Programs without a provable clearance rapid cannot use partial start. The
  operator must edit the CAM output or restart the complete program.
- Motion after an earlier `M2`/`M30` is treated as unreachable and cannot be
  selected as a restart target even if the parser can display its geometry.
