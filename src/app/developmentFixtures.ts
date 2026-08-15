import { previewFixtureAirSquareProgram } from "../features/program/previewFixtureAirSquare";
import { previewFixtureFirstCutProgram } from "../features/program/previewFixtureFirstCut";
import { previewFixtureProgram } from "../features/program/previewFixtureProgram";
import type { AuditLogSnapshot } from "../shared/audit";
import { emptySnapshot, type ControllerSnapshot } from "../shared/machine";
import type { MachineProfileState } from "../shared/profile";
import type { ControllerSettingsState } from "../shared/settings";

export const developmentFixture = import.meta.env.DEV
  ? new URLSearchParams(window.location.search).get("fixture")
  : undefined;

const physicalRunFixtures = new Set([
  "air-square",
  "check-complete",
  "check-running",
  "first-cut",
  "heightmap",
  "preflight",
  "recovery",
  "run-complete",
  "tool-change",
  "tool-motion",
]);

const firstCutFixtures = new Set([
  "air-square",
  "check-complete",
  "check-running",
  "first-cut",
  "recovery",
  "run-complete",
  "tool-change",
  "tool-motion",
]);

const firstCutProgramFixtures = new Set([
  "check-complete",
  "check-running",
  "first-cut",
  "recovery",
  "run-complete",
  "tool-change",
  "tool-motion",
]);

const jogFixtures = new Set(["alarm", "jog", "jog-active", "logs", "reset"]);

export const developmentPreflightFixture = physicalRunFixtures.has(developmentFixture ?? "");
export const developmentFirstCutFixture = firstCutFixtures.has(developmentFixture ?? "");
export const developmentJogFixture = jogFixtures.has(developmentFixture ?? "");
export const developmentProbeFixture = ["heightmap", "probe"].includes(
  developmentFixture ?? "",
);
export const developmentMachineFixture =
  developmentJogFixture || developmentProbeFixture || developmentPreflightFixture;

export const developmentPreviewFixture =
  developmentFixture === "air-square"
    ? previewFixtureAirSquareProgram
    : firstCutProgramFixtures.has(developmentFixture ?? "")
      ? previewFixtureFirstCutProgram
      : ["heightmap", "preflight", "program"].includes(developmentFixture ?? "")
        ? previewFixtureProgram
        : undefined;

const developmentMachineMode =
  developmentFixture === "jog-active"
    ? "jog"
    : developmentFixture === "alarm"
      ? "alarm"
      : "idle";

export const developmentJogSnapshot: ControllerSnapshot = {
  ...emptySnapshot,
  connection: "connected",
  machine: {
    ...emptySnapshot.machine,
    mode: developmentMachineMode,
    reportedMode:
      developmentMachineMode === "jog"
        ? "Jog"
        : developmentMachineMode === "alarm"
          ? "Alarm"
          : "Idle",
    machinePosition: { x: 152.4, y: 91.2, z: -4.75 },
    workPosition: { x: 12.4, y: 8.2, z: 5.25 },
    workCoordinateOffset: { x: 140, y: 83, z: -10 },
    feedRate: 0,
    spindleSpeed: 0,
    pins: developmentProbeFixture
      ? {
          raw: "P",
          xLimit: false,
          yLimit: false,
          zLimit: false,
          aLimit: false,
          bLimit: false,
          cLimit: false,
          probe: developmentFixture === "probe",
          door: false,
          hold: false,
          softReset: false,
          cycleStart: false,
        }
      : undefined,
  },
  pollSequence: 42,
  pollIntervalMs: 250,
  statusTimeoutMs: 500,
  failureThreshold: 2,
  alarm:
    developmentFixture === "alarm"
      ? { code: 3, message: "Reset while in motion" }
      : undefined,
  resetNotice:
    developmentFixture === "reset"
      ? { banner: "Grbl 1.1f ['$' for help]", version: "1.1f", sequence: 4 }
      : undefined,
};

export const developmentProfileFixture: MachineProfileState = {
  profiles: [
    {
      id: "machine-0001",
      name: "LUNYEE CNC",
      travelMm: { x: 500, y: 500, z: 200 },
      maxJogDistanceMm: 50,
      spindleControl: "manual",
      homingInstalled: false,
      limitSwitchesInstalled: false,
      probeInstalled: developmentProbeFixture,
      probeSettings: {
        mode: developmentFixture === "heightmap" ? "heightmap" : "workZero",
        plateThicknessMm: 19.1,
        maxTravelMm: 10,
        probeFeedMmPerMin: 25,
        retractMm: 3,
        retractFeedMmPerMin: 100,
      },
      emergencyStopInstalled: false,
      connection: { transportId: "serial:/dev/cu.fixture-grbl", baudRate: 115_200 },
      detectedController: { firmwareVersion: "1.1f.20230316" },
    },
  ],
  selectedProfileId: "machine-0001",
};

export const developmentSettingsFixture: ControllerSettingsState = {
  snapshot: {
    revision: 4,
    firmwareVersion: "1.1f.20230316",
    firmwareBuildInfo: "LUNYEE_4axis_Control",
    values: [
      { key: "$21", value: "0", title: "Hard limits", group: "safety", kind: "boolean", known: true },
      { key: "$22", value: "0", title: "Homing cycle", group: "homing", kind: "boolean", known: true },
      { key: "$100", value: "1600.000", title: "X steps per millimeter", group: "calibration", kind: "decimal", unit: "step/mm", known: true },
      { key: "$110", value: "1000.000", title: "X maximum rate", group: "motion", kind: "decimal", unit: "mm/min", known: true },
      { key: "$120", value: "600.000", title: "X acceleration", group: "motion", kind: "decimal", unit: "mm/s^2", known: true },
      { key: "$130", value: "500.000", title: "X maximum travel", group: "travel", kind: "decimal", unit: "mm", known: true },
      { key: "$131", value: "500.000", title: "Y maximum travel", group: "travel", kind: "decimal", unit: "mm", known: true },
      { key: "$132", value: "200.000", title: "Z maximum travel", group: "travel", kind: "decimal", unit: "mm", known: true },
      { key: "$200", value: "7.5", title: "Firmware setting 200", group: "advanced", kind: "decimal", known: false },
    ],
  },
  sessionBaseline: {
    "$21": "0",
    "$22": "0",
    "$100": "1600.000",
    "$110": "1000.000",
    "$120": "500.000",
    "$130": "500.000",
    "$131": "500.000",
    "$132": "200.000",
    "$200": "7.5",
  },
  previousBaseline: { "$120": "400.000" },
  revisionCount: 2,
  profileId: "machine-0001",
  fingerprint: {
    key: "port:0483:5740:lunyee_4axis_control:devcuusbmodem11101",
    confidence: "portBound",
    label: "LUNYEE_4axis_Control · 1.1f.20230316 · /dev/cu.usbmodem11101",
  },
};

export const developmentAuditFixture: AuditLogSnapshot = {
  sessionId: "preview-2048",
  activePath: "/Users/operator/Library/Application Support/Millo/logs/millo-audit.jsonl",
  droppedEntries: 0,
  writeFailures: 0,
  entries: [
    {
      schemaVersion: 1,
      sequence: 201,
      sessionId: "preview-2048",
      timestampMs: Date.now() - 8_500,
      level: "info",
      category: "transport",
      event: "transport.connect.completed",
      message: "Controller connected and synchronized",
      data: { port: "/dev/cu.usbmodem11101", firmware: "Grbl 1.1f" },
    },
    {
      schemaVersion: 1,
      sequence: 202,
      sessionId: "preview-2048",
      timestampMs: Date.now() - 5_200,
      level: "warning",
      category: "program",
      event: "program.preflight.report",
      message: "Program preflight is blocked",
      data: { sourceName: "millo-solar-guilloche.nc", blocker: "Work zero not verified" },
    },
    {
      schemaVersion: 1,
      sequence: 203,
      sessionId: "preview-2048",
      timestampMs: Date.now() - 2_100,
      level: "error",
      category: "sender",
      event: "sender.snapshot",
      message: "ALARM:2 at source line 18",
      data: { sourceLine: 18, command: "G1 Z-0.200 F80", state: "failed" },
    },
  ],
};
