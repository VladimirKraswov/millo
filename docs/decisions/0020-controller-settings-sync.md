# ADR 0020: Controller settings are synchronized truth with local revisions

## Status

Accepted.

## Context

GRBL stores controller tuning and machine travel in firmware. Millo also needs
local facts that firmware cannot represent, a stable application profile ID,
and a way to recover from an accidental tuning edit. Treating a local profile as
the settings authority would risk overwriting changes made from another sender.
Treating a USB product name as a unique machine identity would also be false for
controllers without a real USB serial number.

## Decision

- Connect before profile resolution and immediately read `?`, `$I`, `$$`, `$G`,
  and `$#` through the single command actor.
- Build a strong fingerprint from USB VID/PID and a usable serial number. When
  no unique serial exists, build an explicitly `portBound` fallback from
  VID/PID, product, and port. Firmware remains observed metadata rather than
  identity, so an update does not create a second machine. Never write an
  application ID into GRBL.
- Match exactly one fingerprint. Permit one exact-port/firmware legacy match for
  migration. Ambiguous matches do not select a profile.
- Keep serial motion unavailable until the live controller is bound to a
  profile. Unknown controllers open a controller-derived onboarding draft.
- Treat every setting returned by `$$` as controller-owned truth. Keep names and
  physical hardware declarations in `millo-profile`.
- Store a per-profile settings archive only as a duplicate. Its active baseline
  is immutable for one connection. Its current snapshot changes only after a
  verified controller reread. Reconnect starts a new baseline and archives the
  prior baseline when relevant; retain at most 20 revisions.
- Controller edits use a 650 ms UI debounce but execute serially in Rust. Every
  request carries an editing confirmation, source revision, expected old value,
  and target value. The actor rereads before writing, rejects external drift,
  writes one validated `$n=value`, rereads, and compares the stored value
  numerically.
- Rollback targets the active connection baseline even after several edits.
  Previous-session values remain separately restorable and never apply
  automatically.

## Consequences

- Reconnect never pushes a stale local configuration to the controller.
- `500 -> 600 -> 800` during one connection still rolls back to `500`. After
  reconnect, the observed `800` is the new baseline and the earlier `500`
  baseline remains in history.
- A successful GRBL `ok` is insufficient to display Saved; the subsequent
  complete Inspector must report the requested value.
- Controllers without unique USB serials cannot be distinguished perfectly.
  Millo represents that uncertainty and fails closed on ambiguity.
- Jog, work-zero, and serial run preflight remain unavailable during onboarding,
  while Hold, Reset, status, inspection, and disconnect stay available.
