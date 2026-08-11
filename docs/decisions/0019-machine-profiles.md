# ADR 0019: Machine profiles are persistent safety context

## Status

Accepted.

## Context

Millo previously constructed one hardcoded first-machine profile inside the
Tauri adapter. That supported early guarded jog tests but could not represent
multiple machines, a work envelope, or an operator-visible link between the
selected controller and its physical hardware assumptions.

Firmware provides useful but incomplete facts. GRBL `$130/$131/$132` are
configured maximum travel. `$21/$22` reveal whether hard limits and homing are
enabled. `$6` only configures probe input polarity and does not prove that a
physical probe is installed.

## Decision

- Add the Tauri-independent `millo-profile` crate.
- Require a profile name and finite positive X/Y/Z travel. Keep the first axis
  set fixed to XYZ instead of exposing premature arbitrary-axis configuration.
- Record spindle control, homing, limits, probe, and physical emergency stop
  explicitly. Every unverified hardware flag defaults to false.
- Store a schema-versioned bounded JSON document with stable IDs and one selected
  profile in the application configuration directory.
- Keep an optional connection preset and detected firmware identity as
  convenience metadata.
- Allow create/select only while the controller is disconnected. Load the
  selected profile into the command actor before connection and reject
  connection when no profile is selected.
- Provide a temporary disconnected detection session that performs only status
  and Inspector reads. It may derive XYZ and configured limits/homing, but never
  infer probe or emergency-stop hardware.
- Leave edit, delete, and stronger profile-device identity for later slices.

## Consequences

- Readiness, jog, work zero, and real-run preflight share the same selected
  hardware assumptions instead of a React-only preference.
- A first-time operator can enter four required values manually or prefill them
  from GRBL without granting motion authority.
- Selecting another profile cannot silently change safety policy while a serial
  connection is active.
- Configured travel is still not a homed physical boundary. Machines without
  homing and limits retain the unverified-envelope caution.
