import { describe, expect, it } from "vitest";
import { previewFixtureProgram } from "./previewFixtureProgram";
import { programSourceIndex } from "./programSourceIndex";
import { buildToolpathHighlightReadModel } from "./toolpathReadModel";

describe("programSourceIndex", () => {
  it("indexes a large job once and keeps all segments belonging to a line", () => {
    const segment = previewFixtureProgram.toolpath[0];
    const toolpath = Array.from({ length: 100_000 }, (_, index) => ({
      ...segment, sourceLine: Math.floor(index / 2) + 1,
    }));
    let reads = 0;
    const program = {
      ...previewFixtureProgram,
      get toolpath() { reads += 1; return toolpath; },
    };
    const index = programSourceIndex(program);
    expect(index.motions.size).toBe(50_000);
    expect(programSourceIndex(program)).toBe(index);
    const highlight = buildToolpathHighlightReadModel(program, 49_999, { x: 0, y: 0, z: 0 });
    expect(highlight.segmentCount).toBe(2);
    expect(highlight.pointCount).toBe(4);
    expect(reads).toBe(1);
    expect(programSourceIndex({ ...program })).not.toBe(index);
  });
});
