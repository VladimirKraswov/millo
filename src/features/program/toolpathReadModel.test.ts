import { describe, expect, it } from "vitest";

import { previewFixtureProgram } from "./previewFixtureProgram";
import {
  buildToolpathHighlightReadModel,
  buildToolPositionReadModel,
  buildToolpathReadModel,
  sourceLineForIntersection,
} from "./toolpathReadModel";

describe("buildToolpathReadModel", () => {
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
});
