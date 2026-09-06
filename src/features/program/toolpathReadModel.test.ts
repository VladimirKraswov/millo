import { describe, expect, it } from "vitest";
import type { GcodeProgram } from "../../shared/program";

import { previewFixtureProgram } from "./previewFixtureProgram";
import {
  buildToolpathHighlightReadModel,
  buildRotarySelectionReadModel,
  buildToolPositionReadModel,
  buildToolpathReadModel,
  sourceLineForIntersection,
  formatRotaryDegrees,
} from "./toolpathReadModel";

describe("buildToolpathReadModel", () => {
  it("frames both Z extrema after raising a negative cutting path", () => {
    const program = {
      ...previewFixtureProgram,
      summary: { ...previewFixtureProgram.summary, bounds: {
        min: { x: 0, y: 0, z: -1 }, max: { x: 10, y: 0, z: -1 },
        size: { x: 10, y: 0, z: 0 },
      } },
      toolpath: [{ ...previewFixtureProgram.toolpath[1], points: [
        { x: 0, y: 0, z: -1 }, { x: 10, y: 0, z: -1 },
      ] }],
    };
    const model = buildToolpathReadModel(program, 0.5);
    expect(model.center.z).toBe(-0.5);
    expect(model.gridZ).toBe(0);
    expect([...model.cuttingPositions]).toEqual([-5, 0, 0, 5, 0, 0]);
  });
  it("separates rapid and cutting pairs around a stable program center", () => {
    const model = buildToolpathReadModel(previewFixtureProgram);

    expect([...model.rapidPositions]).toEqual([-10, -7.5, 4, -10, -7.5, 0]);
    expect(model.cuttingPositions.length).toBeGreaterThan(12);
    expect(model.rapidSourceLines).toEqual([3]);
    expect(model.cuttingSourceLines[0]).toBe(4);
    expect(model.center).toEqual({ x: 10, y: 7.5, z: 0 });
    expect(model.gridSize).toBe(30);
    expect(model.gridZ).toBe(0);
    expect(model.pointCount).toBeGreaterThan(4);
  });

  it("maps Three.js line-segment vertex indices back to source lines", () => {
    expect(sourceLineForIntersection([4, 8, 12], 0)).toBe(4);
    expect(sourceLineForIntersection([4, 8, 12], 2)).toBe(8);
    expect(sourceLineForIntersection([4, 8, 12], 4)).toBe(12);
    expect(sourceLineForIntersection([4, 8, 12], undefined)).toBeUndefined();
  });

  it("isolates only the selected source-line geometry around the same center", () => {
    const model = buildToolpathReadModel(previewFixtureProgram);

    const selected = buildToolpathHighlightReadModel(
      previewFixtureProgram,
      4,
      model.center,
    );

    expect([...selected.positions]).toEqual([
      -10, -7.5, 0, 10, -7.5, 0,
    ]);
    expect(selected.segmentCount).toBe(1);
    expect(selected.pointCount).toBe(2);
  });

  it("returns an empty selection for source lines without preview motion", () => {
    const model = buildToolpathReadModel(previewFixtureProgram);

    const selected = buildToolpathHighlightReadModel(
      previewFixtureProgram,
      8,
      model.center,
    );

    expect(selected.positions).toHaveLength(0);
    expect(selected.segmentCount).toBe(0);
  });

  it("places the live tool in the same centered coordinate space as the program", () => {
    const model = buildToolpathReadModel(previewFixtureProgram);

    const tool = buildToolPositionReadModel(
      { x: 15, y: 3, z: 2 },
      model,
      previewFixtureProgram.summary.bounds,
    );

    expect(tool.scenePosition).toEqual({ x: 5, y: -4.5, z: 2 });
    expect(tool.gridProjection).toEqual({ x: 5, y: -4.5, z: 0 });
    expect(tool.overProgram).toBe(true);
  });

  it("reports a tool outside the job without clamping its real position", () => {
    const model = buildToolpathReadModel(previewFixtureProgram);

    const tool = buildToolPositionReadModel(
      { x: 31, y: -4, z: 8 },
      model,
      previewFixtureProgram.summary.bounds,
    );

    expect(tool.scenePosition).toEqual({ x: 21, y: -11.5, z: 8 });
    expect(tool.overProgram).toBe(false);
  });

  it("marks rotary-only motion at its XYZ position without inventing linear travel", () => {
    const anchor = { x: 5, y: 2, z: -1 };
    const program: GcodeProgram = {
      ...previewFixtureProgram,
      toolpath: [{
        sourceLine: 4, kind: "linear", distanceMm: 0,
        points: [anchor, anchor], rotary: { startDegrees: -90, endDegrees: 720 },
      }],
    };
    const model = buildToolpathReadModel(program, 0.5);
    expect([...model.rotaryPositions]).toEqual([-5, -5.5, 0]);
    expect(model.rotarySourceLines).toEqual([4]);
    expect([...model.cuttingPositions]).toEqual([-5, -5.5, 0, -5, -5.5, 0]);
    const selection = buildToolpathHighlightReadModel(program, 4, model.center, 0.5);
    expect(selection.segmentCount).toBe(1);
    expect(selection.pointCount).toBe(2);
    expect([...selection.positions]).toEqual([...model.cuttingPositions]);
  });

  it("does not change XYZ geometry or add stationary markers for simultaneous XYZ+A", () => {
    const xyz = buildToolpathReadModel(previewFixtureProgram);
    const rotary = buildToolpathReadModel({
      ...previewFixtureProgram,
      toolpath: previewFixtureProgram.toolpath.map((segment) => ({
        ...segment, rotary: { startDegrees: 0, endDegrees: 1080 },
      })),
    });
    expect(rotary).toEqual(xyz);
  });

  it("does not mark stationary held A as rotary travel", () => {
    const anchor = { x: 0, y: 0, z: 0 };
    const model = buildToolpathReadModel({
      ...previewFixtureProgram,
      toolpath: [{
        sourceLine: 4, kind: "linear", distanceMm: 0,
        points: [anchor, anchor], rotary: { startDegrees: 90, endDegrees: 90 },
      }],
    });
    expect(model.rotaryPositions).toHaveLength(0);
  });
});

describe("rotary selection", () => {
  it("uses exact line details outside the sampled path without scanning the program", () => {
    const program: GcodeProgram = {
      ...previewFixtureProgram,
      get toolpath(): GcodeProgram["toolpath"] { throw new Error("Unexpected full-path scan"); },
    };
    const detail: GcodeProgram["toolpath"] = [{
      sourceLine: 100, kind: "linear", distanceMm: 10,
      points: [{ x: 1, y: 2, z: -1 }, { x: 7, y: 8, z: -2 }],
      rotary: { startDegrees: 720, endDegrees: 1080 },
    }];
    const center = { x: 1, y: 1, z: 1 };
    const selection = buildToolpathHighlightReadModel(program, 100, center, 0.5, detail);
    expect([...selection.positions]).toEqual([0, 1, -1.5, 6, 7, -2.5]);
    expect(selection.segmentCount).toBe(1);
    expect(buildRotarySelectionReadModel(program, 100, detail)).toEqual([detail[0].rotary]);
    expect(buildToolpathHighlightReadModel(program, 99, center, 0, detail).segmentCount).toBe(0);
    expect(buildRotarySelectionReadModel(program, 99, detail)).toEqual([]);
    expect(buildToolpathHighlightReadModel(program, 100, center, 0, []).positions).toHaveLength(0);
    expect(buildRotarySelectionReadModel(program, 100, [])).toEqual([]);
  });

  it("uses the cached source index across live selections, preserving segment boundaries", () => {
    let scans = 0;
    const segments: GcodeProgram["toolpath"] = [
      { ...previewFixtureProgram.toolpath[0], sourceLine: 4, rotary: { startDegrees: -90, endDegrees: 720 } },
      { ...previewFixtureProgram.toolpath[1], sourceLine: 4, rotary: { startDegrees: 720, endDegrees: 450 } },
      { ...previewFixtureProgram.toolpath[1], sourceLine: 5, rotary: { startDegrees: 450, endDegrees: 450 } },
    ];
    const program: GcodeProgram = {
      ...previewFixtureProgram,
      get toolpath() { scans += 1; return segments; },
    };
    expect(buildRotarySelectionReadModel(program, undefined)).toEqual([]);
    expect(scans).toBe(0);
    expect(buildRotarySelectionReadModel(program, 4)).toEqual([
      { startDegrees: -90, endDegrees: 720 }, { startDegrees: 720, endDegrees: 450 },
    ]);
    for (let status = 0; status < 1000; status += 1) {
      expect(buildRotarySelectionReadModel(program, 5)).toEqual([{ startDegrees: 450, endDegrees: 450 }]);
    }
    expect(scans).toBe(1);
    expect(buildRotarySelectionReadModel(program, 99)).toEqual([]);
    expect(buildRotarySelectionReadModel(previewFixtureProgram, 4)).toEqual([]);
  });

  it("formats degrees without wrapping or treating absent telemetry as zero", () => {
    expect(formatRotaryDegrees(-810.25)).toBe("-810.250°");
    expect(formatRotaryDegrees(0)).toBe("0.000°");
    expect(formatRotaryDegrees(undefined)).toBe("--");
    expect(formatRotaryDegrees(NaN)).toBe("--");
    expect(formatRotaryDegrees(Infinity)).toBe("--");
  });
});
