# Alpha release notes

## v0.1.1-alpha.2

This packaging-only follow-up seals the complete macOS application bundle with
an ad-hoc signature. Alpha 1 contained only the Mach-O linker's partial
signature, so Gatekeeper incorrectly reported the downloaded app as damaged.

The bundle now passes `codesign --verify --deep --strict`. It is still not
Developer ID signed or notarized, so macOS may require one explicit approval in
**System Settings > Privacy & Security > Open Anyway** after the first launch.

## v0.1.1-alpha.1

This alpha consolidates the first complete operator workflow used on real GRBL
hardware. It remains an unsigned hardware-testing release.

### Included since Alpha 1

- probe and heightmap workflows with resumable acquisition, numeric and 3D
  inspection, and opt-in toolpath compensation;
- clearer Check and Machining modes, final start confirmation, depth correction,
  work-zero return, and recovery controls;
- production-gated serial sender with GRBL flow control, Hold, Reset, disconnect
  handling, recovery evidence, selected-line start, and modal reconstruction;
- syntax-highlighted G-code editor, machine profiles, tool library, macros, and
  capability-gated bundled and external plugins;
- image-to-toolpath and spoilboard-surfacing system plugins;
- refactored operator boundaries and controller orchestration with expanded Rust,
  UI, fixture, architecture, security, and website regression coverage.

### Distribution

- macOS 11+ for Apple Silicon: DMG;
- x86_64 Linux: AppImage and Debian/Ubuntu DEB;
- all artifacts are unsigned and accompanied by `SHA256SUMS.txt`.

The AppImage may need its executable bit restored after downloading:
`chmod +x Millo_0.1.1_amd64.AppImage`.

## macOS alpha packaging

Use `npm run bundle:mac:alpha`, not a raw Tauri bundle command. It supplies the
`APPLE_SIGNING_IDENTITY=-` identity so Tauri seals the whole `.app` before
creating the DMG. The release gate is:

```bash
codesign --verify --deep --strict --verbose=4 /path/to/Millo.app
```

A production release must replace ad-hoc signing with an Apple Developer ID
Application identity and notarization credentials.

## v0.1.0-alpha.1

This is the first public hardware-testing build of Millo. It is not a production
release and has not been thoroughly validated across machines, GRBL variants,
wiring, spindle workflows, or failure modes.

### Included

- native serial and deterministic Mock GRBL transports;
- typed controller lifecycle, Inspector, Jog, work-zero, Hold, Reset, and Unlock;
- parser-backed Three.js program preview and syntax-highlighted editor;
- GRBL Check, Air run, cutting preflight, bounded sender, tool-change barriers,
  recovery evidence, and safe selected-line start;
- machine profiles, controller-backed settings archive, tool library, logging,
  macros, and capability-gated external plugins;
- system CAM plugins for image jobs and spoilboard surfacing.

### Safety status

- Begin all evaluation with the tool removed and spindle stopped.
- Keep physical power control immediately reachable.
- Verify work coordinates, units, travel, Safe Z, and the complete preview.
- Never leave a running CNC unattended.
- Homing, limits, probe, spindle control, and emergency-stop availability are
  profile facts; Millo cannot infer disconnected hardware.

### Distribution

The alpha artifacts are not code-signed, notarized, or enrolled in an updater.
macOS may require an explicit Open action in Finder. Linux packages target
x86_64 Debian/Ubuntu and AppImage-compatible distributions. SHA-256 files are
published beside the packages in the GitHub release.
