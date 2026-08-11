import type { GcodeProgram } from "../../shared/program";

export const previewFixtureAirSquareProgram: GcodeProgram = {
  sourceName: "air-square-20mm.nc",
  lines: [
    { sourceLine: 1, source: "%", normalized: "", executable: false, warningCount: 0 },
    { sourceLine: 2, source: "(Millo hardware fixture: 20 x 20 mm air-run square)", normalized: "", executable: false, warningCount: 0 },
    { sourceLine: 3, source: "(Start at verified G54-G59 X0 Y0; tool removed; spindle power off)", normalized: "", executable: false, warningCount: 0 },
    { sourceLine: 4, source: "G21 G90 G94 G17", normalized: "G21 G90 G94 G17", executable: true, warningCount: 0 },
    { sourceLine: 5, source: "M5 M9", normalized: "M5 M9", executable: true, warningCount: 0 },
    { sourceLine: 6, source: "G1 X20.000 Y0.000 F100.000", normalized: "G1 X20.000 Y0.000 F100.000", executable: true, warningCount: 0 },
    { sourceLine: 7, source: "G1 X20.000 Y20.000", normalized: "G1 X20.000 Y20.000", executable: true, warningCount: 0 },
    { sourceLine: 8, source: "G1 X0.000 Y20.000", normalized: "G1 X0.000 Y20.000", executable: true, warningCount: 0 },
    { sourceLine: 9, source: "G1 X0.000 Y0.000", normalized: "G1 X0.000 Y0.000", executable: true, warningCount: 0 },
    { sourceLine: 10, source: "M5 M9", normalized: "M5 M9", executable: true, warningCount: 0 },
    { sourceLine: 11, source: "M30", normalized: "M30", executable: true, warningCount: 0 },
    { sourceLine: 12, source: "%", normalized: "", executable: false, warningCount: 0 },
  ],
  warnings: [],
  features: {
    usesImperialUnits: false,
    usesIncrementalDistance: false,
    hasSpindleActivation: false,
    hasSpindleSpeed: false,
    hasToolChange: false,
    hasProbeCycle: false,
    hasMachineCoordinateMove: false,
  },
  summary: {
    lineCount: 12,
    executableLineCount: 8,
    motionCount: 4,
    rapidDistanceMm: 0,
    cuttingDistanceMm: 80,
    bounds: {
      min: { x: 0, y: 0, z: 0 },
      max: { x: 20, y: 20, z: 0 },
      size: { x: 20, y: 20, z: 0 },
    },
    previewComplete: true,
    dryRunEligible: true,
  },
  toolpath: [
    { sourceLine: 6, kind: "linear", distanceMm: 20, points: [{ x: 0, y: 0, z: 0 }, { x: 20, y: 0, z: 0 }] },
    { sourceLine: 7, kind: "linear", distanceMm: 20, points: [{ x: 20, y: 0, z: 0 }, { x: 20, y: 20, z: 0 }] },
    { sourceLine: 8, kind: "linear", distanceMm: 20, points: [{ x: 20, y: 20, z: 0 }, { x: 0, y: 20, z: 0 }] },
    { sourceLine: 9, kind: "linear", distanceMm: 20, points: [{ x: 0, y: 20, z: 0 }, { x: 0, y: 0, z: 0 }] },
  ],
};
