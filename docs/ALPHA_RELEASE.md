# Alpha release notes

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
