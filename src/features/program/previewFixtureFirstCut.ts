import type { GcodeProgram } from "../../shared/program";
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
  lines: previewFixtureProgram.lines.filter((line) => line.sourceLine !== 8),
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
  },
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
  },
};

let fixtureIntent: "airRun" | "cutting" = "airRun";
let fixtureSourceName = previewFixtureFirstCutProgram.sourceName;
const fixtureIsAirSquare = () => fixtureSourceName === "air-square-20mm.nc";

export const previewFixtureFirstCutGateway: RealRunPreflightGateway = {
  preflight: async (request, intent) => {
    fixtureIntent = intent;
    fixtureSourceName = request.sourceName;
    return {
      ...previewFixtureFirstCutReport,
      sourceName: fixtureSourceName,
      intent,
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
      },
      authorization: {
        ...preparation.authorization,
        sourceName: fixtureSourceName,
        intent: confirmation.intent,
      },
    };
  },
  startProgram: async () => ({
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
  }),
  startCheck: async (request) => ({
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
  }),
  resumeProgram: async () => ({
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
  }),
};
