import type { HardwareInspection } from "../../shared/machine";
import type {
  RealRunPreflightGateway,
  RunPreflightReport,
} from "../../shared/realRun";
import { previewFixtureProgram } from "./previewFixtureProgram";

const hardware: HardwareInspection = {
  device: {
    firmwareVersion: "1.1f.20230316",
    firmwareOptions: "VMZHL,35,254",
    settings: { $20: "0", $21: "0", $22: "0", $32: "0" },
    modalState: ["G0", "G54", "G17", "G21", "G90", "M5"],
    parameters: { G54: "0.000,0.000,0.000", PRB: "0.000,0.000,0.000:0" },
    responses: [],
  },
  readiness: {
    profile: {
      name: "First XYZ router",
      axes: ["X", "Y", "Z"],
      spindleControl: "manual",
      homingInstalled: false,
      limitSwitchesInstalled: false,
      probeInstalled: false,
      emergencyStopInstalled: false,
    },
    testJogReady: true,
    probeReady: false,
    blockerCount: 0,
    cautionCount: 3,
    checks: [],
  },
};

export const previewFixturePreflight: RunPreflightReport = {
  sourceName: previewFixtureProgram.sourceName,
  programFingerprint: "fixture-preflight-sha256",
  intent: "airRun",
  ready: false,
  blockerCount: 1,
  cautionCount: 3,
  pollSequence: 42,
  bounds: previewFixtureProgram.summary.bounds,
  hardware,
  checks: [
    {
      id: "controller-state",
      level: "pass",
      title: "Fresh controller state",
      detail: "Connected · Idle · status #42",
    },
    {
      id: "motion-hardware",
      level: "pass",
      title: "Motion configuration",
      detail: "Firmware, XYZ tuning, limits profile, units and milling mode passed",
    },
    {
      id: "program-policy",
      level: "blocker",
      title: "Motion-only program policy",
      detail: "1 blocked command; first: M3 spindle activation is forbidden",
      sourceLine: 8,
    },
    {
      id: "program-geometry",
      level: "pass",
      title: "Program geometry",
      detail: "5 motions · 20.000 × 15.000 × 0.000 mm",
    },
    {
      id: "program-modal-contract",
      level: "pass",
      title: "Explicit program modes",
      detail: "G21, G90, G94 and G17 are declared before motion",
    },
    {
      id: "work-coordinate-system",
      level: "pass",
      title: "Work coordinate system",
      detail: "G54 is active; work zero must be verified before authorization",
    },
    {
      id: "unhomed-envelope",
      level: "caution",
      title: "Unverified machine envelope",
      detail: "Preview bounds do not prove physical clearance",
    },
    {
      id: "manual-spindle",
      level: "caution",
      title: "Manual spindle workflow",
      detail: "Automatic spindle commands remain forbidden",
    },
    {
      id: "operator-setup",
      level: "caution",
      title: "Physical setup not authorized",
      detail: "Stock, cutter, work zero and safe Z still require confirmation",
    },
  ],
  programBlockers: [
    {
      kind: "spindle-activation",
      message: "M3 spindle activation is forbidden",
      sourceLine: 8,
    },
  ],
  totalProgramBlockers: 1,
};

export const previewFixturePreflightGateway: RealRunPreflightGateway = {
  preflight: async (_request, intent) => ({ ...previewFixturePreflight, intent }),
  authorizeFirstCut: async () => {
    throw new Error("Blocked fixture cannot be authorized");
  },
  startProgram: async () => {
    throw new Error("Blocked fixture cannot start");
  },
  resumeProgram: async () => {
    throw new Error("Blocked fixture cannot resume");
  },
};
