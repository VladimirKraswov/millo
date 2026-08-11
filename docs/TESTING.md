# Testing and definition of done

Every vertical slice must update tests and documentation in the same commit.
The standard local gate is:

```bash
npm run verify
```

It runs TypeScript type checking, all Rust workspace tests, the production Vite
build, Rust formatting checks, and Clippy with warnings denied.

The test phase also runs `scripts/check-brand.mjs`, which keeps npm, Cargo,
Tauri, UI, and documentation naming consistent.

## Slice checklist

1. Capture new protocol or compatibility behavior as a fixture where possible.
2. Add focused unit tests for state transitions and failure paths.
3. Add adapter or UI tests when behavior exists outside the Rust core.
4. Run `npm run verify` from a clean working tree.
5. Update `README.md`, `docs/ARCHITECTURE.md`, and an ADR when a boundary or
   architectural decision changes.
6. Perform visual verification for changed operator screens.
7. Commit the complete slice atomically.

## Current lifecycle coverage

- GRBL status, reset, alarm, error, and acknowledgement fixtures.
- Reset banner ordering in the mock transport.
- Persistent mock alarm and explicit alarm clearing.
- Unresponsive transport simulation.
- Transient timeout counting and threshold transition to recovery.
- Reconnect plus status synchronization before returning to connected.
- Reset acknowledgement and non-alarm status behavior.

## Current native serial coverage

- Boxed runtime transport preserves the common transport contract.
- Empty port names and zero baud rates are rejected before OS I/O.
- Fragmented serial input is assembled into one CR/LF-trimmed line.
- End-of-stream and I/O before connect are reported as disconnection.
- Tauri serial IDs preserve native port names, including Unix device paths.
- USB device metadata maps to a stable UI descriptor.
- Likely-GRBL discovery accepts explicit controller metadata and common USB-UART
  vendors while rejecting Bluetooth and unidentified USB fixtures.
- macOS callout/TTY alias pairs collapse to `/dev/cu.*`; unpaired and non-macOS
port names remain untouched.

## Current command arbiter and inspector coverage

- One worker serializes polling, realtime bytes, and line queries.
- Actor-owned periodic polling publishes lifecycle snapshots.
- Realtime `?` consumes its status response; `!`, `~`, and `Ctrl-X` use their
  exact one-byte representation.
- `$I`, `$$`, `$G`, and `$#` execute in deterministic order and stop at their
  correlated terminal response.
- `error:n` and `ALARM:n` retain both active command and numeric code.
- Recorded Inspector fixtures parse firmware/build/options, numbered settings,
  modal state, WCS/TLO, and probe parameters.
- Mock Inspector responses cover the full Rust-to-UI happy path without motion.
- The Tauri command surface contains no raw-line or movement endpoint.

## Current hardware readiness coverage

- A representative unhomed XYZ fixture passes the guarded test-jog
  configuration while retaining cautions for G91, manual spindle, missing
  homing/limits, untested probe input, and missing physical emergency stop.
- Missing axis tuning blocks readiness.
- Enabled homing or hard limits block a profile that declares no sensors.
- Laser mode blocks the milling profile.
- Alarm or non-idle controller state blocks readiness even when static settings
  are valid.
- Mock GRBL exposes all required XYZ values and exercises the ready report across
  the command actor and typed Tauri response.
- The Tauri mock smoke test confirms a ready report is invalidated after an
  injected alarm rather than leaving stale green readiness on screen.

CI does not require a physical controller. For a hardware smoke test, launch
`npm run tauri dev`, refresh the device list, connect at the controller's baud
rate, verify that machine coordinates update, unplug the device, and confirm the
state moves through `Recovering`. Reconnect the device and confirm polling
returns to `Connected`.
