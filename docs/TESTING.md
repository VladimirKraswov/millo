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

CI does not require a physical controller. For a hardware smoke test, launch
`npm run tauri dev`, refresh the device list, connect at the controller's baud
rate, verify that machine coordinates update, unplug the device, and confirm the
state moves through `Recovering`. Reconnect the device and confirm polling
returns to `Connected`.
