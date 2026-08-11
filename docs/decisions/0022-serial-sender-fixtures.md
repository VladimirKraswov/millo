# ADR 0022: Promote the sender through test-only serial fixtures

## Status

Accepted.

## Context

The bounded sender already executes strict motion-only plans on Mock GRBL. The
first-cut gate now issues a short-lived single-use lease, but exposing a real
serial Start command before testing their combined transaction would make UI
state or duplicated orchestration part of the safety boundary.

## Decision

- Reuse one `millo-sender` state machine and identify jobs as `mockDryRun` or
  `firstCut`; do not fork sender behavior by transport.
- Add a command-actor start coordinator that rebuilds the strict opaque plan,
  reads a fresh status, requires stable Idle, consumes the matching lease, then
  loads and starts the sender in one serialized request.
- Compile the coordinator request and its deterministic dispatch barrier only
  for Rust tests. Do not register a Tauri command or add a React/plugin API.
- Exercise a serial-class actor over `MockTransport` for complete `ok`,
  correlated `error`, `ALARM`, Hold/resume, reset-banner, and link-drop cases.
- Keep one line in flight. Every `ok` advances exactly once; terminal responses
  retain the source line and prevent further dispatch.
- Treat authorization consumption as destructive. Reuse and failed mismatches
  require a new complete first-cut authorization.

## Consequences

- The sender/lease integration can mature without making physical motion
  reachable from the application.
- The next slice can expose a narrowly gated air-cut endpoint instead of
  inventing another sender path.
- Hold is processed by the actor's prioritized request queue between GRBL line
  acknowledgements. Hardware smoke testing must measure this latency before a
  cutting run is allowed.
- Reset banners and transport loss fail the sender even when no UI is present.
