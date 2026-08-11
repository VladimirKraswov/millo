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

export const previewFixtureFirstCutGateway: RealRunPreflightGateway = {
  preflight: async (_request, intent) => {
    fixtureIntent = intent;
    return { ...previewFixtureFirstCutReport, intent };
  },
  authorizeFirstCut: async (_request, confirmation) => {
    fixtureIntent = confirmation.intent;
    return {
      report: { ...preparation.report, intent: confirmation.intent },
      authorization: {
        ...preparation.authorization,
        intent: confirmation.intent,
      },
    };
  },
  startProgram: async () => ({
    state: "running",
    mode: fixtureIntent === "airRun" ? "airRun" : "cutRun",
    sourceName: previewFixtureFirstCutProgram.sourceName,
    totalLines: 8,
    acknowledgedLines: 2,
    currentSourceLine: 1,
    currentCommand: "G21 G90 G94 G17",
    progress: 0.25,
  }),
  resumeProgram: async () => ({
    state: "running",
    mode: fixtureIntent === "airRun" ? "airRun" : "cutRun",
    sourceName: previewFixtureFirstCutProgram.sourceName,
    totalLines: 8,
    acknowledgedLines: 4,
    currentSourceLine: 3,
    currentCommand: "G1 X20 F60",
    progress: 0.5,
  }),
};
