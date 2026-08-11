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
  },
};

export const previewFixtureFirstCutGateway: RealRunPreflightGateway = {
  preflight: async () => previewFixtureFirstCutReport,
  authorizeFirstCut: async () => preparation,
};
