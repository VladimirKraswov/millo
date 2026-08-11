import type { GcodeProgram } from "../../shared/program";

export const previewFixtureProgram: GcodeProgram = {
  sourceName: "preview-fixture.nc",
  lines: [],
  warnings: [
    {
      sourceLine: 8,
      severity: "safety",
      code: "spindle-activation",
      message: "M3 spindle activation will be blocked by dry run",
    },
  ],
  features: {
    usesImperialUnits: false,
    usesIncrementalDistance: false,
    hasSpindleActivation: true,
    hasSpindleSpeed: false,
    hasToolChange: false,
    hasProbeCycle: false,
    hasMachineCoordinateMove: false,
  },
  summary: {
    lineCount: 9,
    executableLineCount: 8,
    motionCount: 5,
    rapidDistanceMm: 4,
    cuttingDistanceMm: 60,
    bounds: {
      min: { x: 0, y: 0, z: 0 },
      max: { x: 20, y: 15, z: 0 },
      size: { x: 20, y: 15, z: 0 },
    },
    previewComplete: true,
    dryRunEligible: false,
  },
  toolpath: [
    {
      sourceLine: 3,
      kind: "rapid",
      distanceMm: 4,
      points: [
        { x: 0, y: 0, z: 4 },
        { x: 0, y: 0, z: 0 },
      ],
    },
    {
      sourceLine: 4,
      kind: "linear",
      distanceMm: 20,
      points: [
        { x: 0, y: 0, z: 0 },
        { x: 20, y: 0, z: 0 },
      ],
    },
    {
      sourceLine: 5,
      kind: "linear",
      distanceMm: 15,
      points: [
        { x: 20, y: 0, z: 0 },
        { x: 20, y: 15, z: 0 },
      ],
    },
    {
      sourceLine: 6,
      kind: "linear",
      distanceMm: 20,
      points: [
        { x: 20, y: 15, z: 0 },
        { x: 0, y: 15, z: 0 },
      ],
    },
    {
      sourceLine: 7,
      kind: "arcClockwise",
      distanceMm: 15,
      points: [
        { x: 0, y: 15, z: 0 },
        { x: 4, y: 13, z: 0 },
        { x: 7, y: 9, z: 0 },
        { x: 6, y: 4, z: 0 },
        { x: 0, y: 0, z: 0 },
      ],
    },
  ],
};
