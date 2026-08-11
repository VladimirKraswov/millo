import { describe, expect, it } from "vitest";

import { previewFixtureProgram } from "./previewFixtureProgram";
import {
  buildToolpathHighlightReadModel,
  buildToolpathReadModel,
} from "./toolpathReadModel";

describe("buildToolpathReadModel", () => {
  it("separates rapid and cutting pairs around a stable program center", () => {
    const model = buildToolpathReadModel(previewFixtureProgram);

    expect([...model.rapidPositions]).toEqual([-10, -7.5, 4, -10, -7.5, 0]);
    expect(model.cuttingPositions.length).toBeGreaterThan(12);
    expect(model.center).toEqual({ x: 10, y: 7.5, z: 0 });
    expect(model.gridSize).toBe(30);
    expect(model.gridZ).toBe(0);
    expect(model.pointCount).toBeGreaterThan(4);
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
});
