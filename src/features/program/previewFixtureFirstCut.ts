import type { GcodeProgram } from "../../shared/program";
import type { ProgramGateway } from "../../platform/program/ProgramGateway";
import { idleSenderSnapshot } from "../../shared/dryRun";
import type { SenderSnapshot, SenderStateGateway } from "../../shared/dryRun";
import type {
  FirstCutPreparation,
  RealRunPreflightGateway,
  RunPreflightReport,
} from "../../shared/realRun";
import { previewFixturePreflight } from "./previewFixturePreflight";
import { previewFixtureProgram } from "./previewFixtureProgram";

export const previewFixtureFirstCutProgram: GcodeProgram = {
  ...previewFixtureProgram,
  sourceName: "first-cut-fixture.nc",
  lines: previewFixtureProgram.lines
    .filter((line) => line.sourceLine !== 8)
    .map((line) => line.sourceLine === 4
      ? { ...line, source: "G1 X20 Z-0.2 F120", normalized: "G1 X20 Z-0.2 F120" }
      : line),
  warnings: [],
  features: {
    ...previewFixtureProgram.features,
    hasSpindleActivation: false,
  },
  summary: {
    ...previewFixtureProgram.summary,
    lineCount: previewFixtureProgram.summary.lineCount - 1,
    executableLineCount: previewFixtureProgram.summary.executableLineCount - 1,
    dryRunEligible: true,
    bounds: {
      min: { x: 0, y: 0, z: -0.2 },
      max: { x: 20, y: 15, z: 0 },
      size: { x: 20, y: 15, z: 0.2 },
    },
  },
  toolpath: previewFixtureProgram.toolpath.map((segment) => segment.kind === "rapid"
    ? segment
    : {
        ...segment,
        points: segment.points.map((point) => ({ ...point, z: -0.2 })),
      }),
};

export const previewFixtureProgramGateway: ProgramGateway = {
  parse: async (request, options) => ({
    ...previewFixtureFirstCutProgram,
    sourceName: request.sourceName,
    blockDeleteEnabled: options?.blockDelete ?? false,
  }),
};

export const previewFixtureToolChangeSender: SenderSnapshot = {
  ...idleSenderSnapshot,
  state: "toolChange",
  mode: "cutRun",
  sourceName: previewFixtureFirstCutProgram.sourceName,
  totalLines: 12,
  dispatchedLines: 7,
  acknowledgedLines: 6,
  currentSourceLine: 5,
  currentCommand: "T2 M6",
  requestedTool: 2,
  progress: 0.5,
  elapsedSeconds: 42,
  estimatedCompletedSeconds: 38,
  estimatedRemainingSeconds: 31,
  estimatedTotalSeconds: 69,
  timeEstimateComplete: false,
};

export const previewFixtureCheckCompleteSender: SenderSnapshot = {
  ...idleSenderSnapshot,
  runSequence: 41,
  state: "completed",
  mode: "checkRun",
  sourceName: previewFixtureFirstCutProgram.sourceName,
  totalLines: 10,
  dispatchedLines: 10,
  acknowledgedLines: 10,
  currentSourceLine: 8,
  currentCommand: "M2",
  progress: 1,
  elapsedSeconds: 32,
  estimatedCompletedSeconds: 32,
  estimatedRemainingSeconds: 0,
  estimatedTotalSeconds: 32,
  timeEstimateComplete: true,
};

export const previewFixtureCompletedSender: SenderSnapshot = {
  ...previewFixtureCheckCompleteSender,
  runSequence: 43,
  mode: "cutRun",
};

export const previewFixtureCheckRunningSender: SenderSnapshot = {
  ...previewFixtureCheckCompleteSender,
  runSequence: 42,
  state: "running",
  dispatchedLines: 7,
  acknowledgedLines: 6,
  inFlightLines: 1,
  currentSourceLine: 7,
  currentCommand: "G1 X12 Y8 F240",
  progress: 0.6,
  elapsedSeconds: 18,
  estimatedCompletedSeconds: 20,
  estimatedRemainingSeconds: 14,
  estimatedTotalSeconds: 34,
  timeEstimateComplete: false,
};

export const previewFixtureCutRunningSender: SenderSnapshot = {
  ...previewFixtureCheckRunningSender,
  runSequence: 44,
  mode: "cutRun",
  executingSourceLine: 4,
  lastAcknowledgedSourceLine: 3,
};

export const previewFixtureCheckControlGateway: SenderStateGateway = {
  snapshot: async () => previewFixtureCheckRunningSender,
  subscribe: async () => () => undefined,
};

export const previewFixtureFirstCutReport: RunPreflightReport = {
  ...previewFixturePreflight,
  sourceName: previewFixtureFirstCutProgram.sourceName,
  programFingerprint: "fixture-first-cut-sha256",
  ready: true,
  blockerCount: 0,
  checks: previewFixturePreflight.checks.map((check) =>
    check.id === "program-policy"
      ? {
          ...check,
          level: "pass",
          detail: "Motion-only program policy passed",
          sourceLine: undefined,
        }
      : check,
  ),
  programBlockers: [],
  totalProgramBlockers: 0,
};

const preparation: FirstCutPreparation = {
  report: { ...previewFixtureFirstCutReport, pollSequence: 43 },
  authorization: {
    id: 7,
    expiresInMs: 30_000,
    sourceName: previewFixtureFirstCutProgram.sourceName,
    programFingerprint: previewFixtureFirstCutReport.programFingerprint,
    pollSequence: 43,
    intent: "airRun",
    executionOptions: { optionalStop: false, blockDelete: false },
  },
};

let fixtureIntent: "airRun" | "cutting" = "airRun";
let fixtureSourceName = previewFixtureFirstCutProgram.sourceName;
const fixtureIsAirSquare = () => fixtureSourceName === "air-square-20mm.nc";

export const previewFixtureFirstCutGateway: RealRunPreflightGateway = {
  prepareSelectedRun: async (request) => ({
    originalSourceName: request.request.sourceName,
    selectedSourceLine: request.selectedSourceLine,
    restartSourceLine: request.selectedSourceLine,
    restartPosition: { x: 0, y: 0, z: request.safeZMm },
    safeZMm: request.safeZMm,
    minimumSafeZMm: 0,
    replayedExecutableLines: 0,
    remainingExecutableLines: 1,
    workCoordinateSystem: "g54",
    spindleMode: "off",
    request: {
      sourceName: `safe-start-L${request.selectedSourceLine}-${request.request.sourceName}`,
      source: request.request.source,
    },
  }),
  recoveryCandidate: async () => null,
  prepareRecovery: async () => {
    throw new Error("Fixture has no interrupted run");
  },
  dismissRecovery: async () => undefined,
  preflight: async (request, intent, executionOptions) => {
    fixtureIntent = intent;
    fixtureSourceName = request.sourceName;
    return {
      ...previewFixtureFirstCutReport,
      sourceName: fixtureSourceName,
      intent,
      executionOptions,
      bounds: fixtureIsAirSquare()
        ? {
            min: { x: 0, y: 0, z: 0 },
            max: { x: 20, y: 20, z: 0 },
            size: { x: 20, y: 20, z: 0 },
          }
        : previewFixtureFirstCutReport.bounds,
    };
  },
  authorizeFirstCut: async (request, confirmation) => {
    fixtureIntent = confirmation.intent;
    fixtureSourceName = request.sourceName;
    return {
      report: {
        ...preparation.report,
        sourceName: fixtureSourceName,
        intent: confirmation.intent,
        executionOptions: confirmation.executionOptions,
      },
      authorization: {
        ...preparation.authorization,
        sourceName: fixtureSourceName,
        intent: confirmation.intent,
        executionOptions: confirmation.executionOptions,
      },
    };
  },
  startProgram: async () => ({
    ...idleSenderSnapshot,
    runSequence: 1,
    state: "running",
    mode: fixtureIntent === "airRun" ? "airRun" : "cutRun",
    sourceName: fixtureSourceName,
    totalLines: fixtureIsAirSquare() ? 10 : 8,
    dispatchedLines: fixtureIsAirSquare() ? 7 : 5,
    acknowledgedLines: 2,
    inFlightLines: 5,
    rxBufferBytes: 64,
    rxBufferCapacity: 127,
    currentSourceLine: fixtureIsAirSquare() ? 4 : 1,
    currentCommand: "G21 G90 G94 G17",
    progress: 0.25,
    elapsedSeconds: 4.5,
    estimatedCompletedSeconds: 8,
    estimatedRemainingSeconds: 40,
    estimatedTotalSeconds: 48,
    timeEstimateComplete: false,
  }),
  startCheck: async (request) => ({
    ...idleSenderSnapshot,
    runSequence: 2,
    state: "running",
    mode: "checkRun",
    sourceName: request.sourceName,
    totalLines: 10,
    dispatchedLines: 1,
    acknowledgedLines: 0,
    inFlightLines: 1,
    rxBufferBytes: 3,
    rxBufferCapacity: 127,
    currentCommand: "M5",
    progress: 0,
    elapsedSeconds: 0.2,
    estimatedCompletedSeconds: 0,
    estimatedRemainingSeconds: 48,
    estimatedTotalSeconds: 48,
    timeEstimateComplete: false,
  }),
  pauseProgram: async () => ({
    ...idleSenderSnapshot,
    runSequence: 1,
    state: "paused",
    mode: fixtureIntent === "airRun" ? "airRun" : "cutRun",
    sourceName: fixtureSourceName,
    totalLines: fixtureIsAirSquare() ? 10 : 8,
    dispatchedLines: fixtureIsAirSquare() ? 8 : 6,
    acknowledgedLines: 4,
    inFlightLines: 4,
    rxBufferBytes: 48,
    rxBufferCapacity: 127,
    currentSourceLine: 3,
    currentCommand: "G1 X20 F60",
    progress: 0.5,
    elapsedSeconds: 12,
    estimatedCompletedSeconds: 20,
    estimatedRemainingSeconds: 28,
    estimatedTotalSeconds: 48,
    timeEstimateComplete: false,
  }),
  resumeProgram: async () => ({
    ...idleSenderSnapshot,
    runSequence: 1,
    state: "running",
    mode: fixtureIntent === "airRun" ? "airRun" : "cutRun",
    sourceName: fixtureSourceName,
    totalLines: fixtureIsAirSquare() ? 10 : 8,
    dispatchedLines: fixtureIsAirSquare() ? 8 : 6,
    acknowledgedLines: 4,
    inFlightLines: 4,
    rxBufferBytes: 48,
    rxBufferCapacity: 127,
    currentSourceLine: 3,
    currentCommand: "G1 X20 F60",
    progress: 0.5,
    elapsedSeconds: 12,
    estimatedCompletedSeconds: 20,
    estimatedRemainingSeconds: 28,
    estimatedTotalSeconds: 48,
    timeEstimateComplete: false,
  }),
  abortProgram: async () => ({
    ...idleSenderSnapshot,
    runSequence: 1,
    state: "cancelled",
    mode: fixtureIntent === "airRun" ? "airRun" : "cutRun",
    sourceName: fixtureSourceName,
    totalLines: fixtureIsAirSquare() ? 10 : 8,
    dispatchedLines: fixtureIsAirSquare() ? 8 : 6,
    acknowledgedLines: 4,
    currentSourceLine: 3,
    currentCommand: "G1 X20 F60",
    progress: 0.5,
    elapsedSeconds: 12,
    estimatedCompletedSeconds: 20,
    estimatedRemainingSeconds: 28,
    estimatedTotalSeconds: 48,
    timeEstimateComplete: false,
  }),
  completeToolChange: async () => ({
    ...idleSenderSnapshot,
    state: "running",
    mode: "cutRun",
    sourceName: fixtureSourceName,
    totalLines: 12,
    dispatchedLines: 8,
    acknowledgedLines: 7,
    inFlightLines: 1,
    rxBufferBytes: 12,
    currentSourceLine: 6,
    currentCommand: "G1 X20 F120",
    progress: 7 / 12,
    elapsedSeconds: 42,
    estimatedCompletedSeconds: 38,
    estimatedRemainingSeconds: 31,
    estimatedTotalSeconds: 69,
  }),
};

export const previewFixtureRecoveryGateway: RealRunPreflightGateway = {
  ...previewFixtureFirstCutGateway,
  recoveryCandidate: async () => ({
    id: 1_786_500_000_000_001,
    sourceName: "engraving-interrupted.nc",
    intent: "cutting",
    state: "running",
    updatedAtUnixMs: 1_786_500_000_000,
    totalLines: 1_420,
    acknowledgedLines: 936,
    executingSourceLine: 911,
    restartSourceLine: 884,
    restartPosition: { x: 48.2, y: 31.7, z: 5 },
    minimumSafeZMm: 5,
    checkpointRestartAvailable: true,
    fullRestartAvailable: true,
    interruption: "controllerDisconnected",
    ready: true,
    detail: "Restart from source line 884 replays 27 line(s) before the interrupted line",
  }),
};
