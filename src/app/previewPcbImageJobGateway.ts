import { previewFixtureProgram } from "../features/program/previewFixtureProgram";
import type { ImageJobGateway } from "../platform/jobs/ImageJobGateway";
import { tauriImageJobGateway } from "../platform/jobs/tauriImageJobGateway";
import type {
  GeneratedPcbJob,
  PcbInspectRequest,
  PcbInspection,
} from "../shared/jobs";

const copperPath = [
  { xMm: 8, yMm: 8 },
  { xMm: 30, yMm: 8 },
  { xMm: 30, yMm: 10 },
  { xMm: 10, yMm: 10 },
];

const outlinePath = [
  { xMm: 0, yMm: 0 },
  { xMm: 38, yMm: 0 },
  { xMm: 38, yMm: 56 },
  { xMm: 0, yMm: 56 },
];

const inspectFixture = (request: PcbInspectRequest): PcbInspection => {
  const drilling = request.files.find((file) => file.role === "drill");
  const paths = request.files.flatMap((file) => {
    if (file.role === "ignore" || file.role === "drill") return [];
    return [{
      role: file.role,
      closed: file.role === "outline",
      points: file.role === "outline" ? outlinePath : copperPath,
    }];
  });
  return {
    bounds: {
      minXMm: request.transform.offsetXMm,
      minYMm: request.transform.offsetYMm,
      maxXMm: request.transform.offsetXMm + 38,
      maxYMm: request.transform.offsetYMm + 56,
      widthMm: 38,
      heightMm: 56,
    },
    paths,
    drillHits: drilling ? [
      { groupKey: `${drilling.sourceName}::T1`, point: { xMm: 4, yMm: 4 } },
      { groupKey: `${drilling.sourceName}::T1`, point: { xMm: 34, yMm: 4 } },
    ] : [],
    drillSlots: drilling ? [{
      groupKey: `${drilling.sourceName}::T2`,
      start: { xMm: 16, yMm: 48 },
      end: { xMm: 22, yMm: 48 },
    }] : [],
    drillGroups: drilling ? [
      {
        key: `${drilling.sourceName}::T1`,
        sourceName: drilling.sourceName,
        sourceToolNumber: 1,
        diameterMm: 0.8,
        hitCount: 2,
        slotCount: 0,
      },
      {
        key: `${drilling.sourceName}::T2`,
        sourceName: drilling.sourceName,
        sourceToolNumber: 2,
        diameterMm: 1,
        hitCount: 0,
        slotCount: 1,
      },
    ] : [],
    files: request.files.map((file) => ({
      sourceName: file.sourceName,
      role: file.role,
      primitiveCount: file.role === "ignore" ? 0 : 1,
    })),
    warnings: [],
  };
};

const generatedFixture = (request: Parameters<ImageJobGateway["generatePcb"]>[0]): GeneratedPcbJob => {
  const inspection = inspectFixture(request.board);
  return {
    sourceName: request.sourceName,
    source: previewFixtureProgram.lines.map((line) => line.source).join("\n"),
    program: { ...previewFixtureProgram, sourceName: request.sourceName },
    inspection,
    summary: {
      bounds: inspection.bounds,
      operations: [{
        kind: "isolation",
        toolId: request.settings.isolation.toolId,
        toolName: "Preview tool",
        motionCount: previewFixtureProgram.summary.motionCount,
      }],
      toolCount: 1,
      toolChangeCount: 0,
      warningCount: 0,
    },
  };
};

export const previewPcbImageJobGateway: ImageJobGateway = {
  ...tauriImageJobGateway,
  inspectPcb: async (request) => inspectFixture(request),
  generatePcb: async (request) => generatedFixture(request),
  save: async (job) => ({ path: `/tmp/${job.sourceName}`, bytesWritten: job.source.length }),
};
