import { describe, expect, it } from "vitest";
import {
  createShape,
  emptySketch,
  fanShapes,
  grilleShapes,
  shapePoints,
  snap,
  validateSketch,
} from "./sketchModel";
import { decodeSketch } from "./sketchStorage";
import { sketchHistory } from "./sketchHistory";

describe("sketch document", () => {
  it("creates editable fan mounting holes and a tabbed outline", () => {
    const shapes = fanShapes(
      { opening: 70, pitch: 71.5, hole: 4.2, plate: 100 },
      { x: 100, y: 70 },
    );
    expect(shapes).toHaveLength(6);
    expect(new Set(shapes.map((s) => s.id)).size).toBe(6);
    expect(shapes[0].xMm).toBe(64.25);
    expect(shapes[0].operation.kind).toBe("pocket");
    expect(shapes[4].operation).toMatchObject({
      kind: "inside",
      tabs: { count: 4 },
    });
    expect(shapes[5].operation).toMatchObject({
      kind: "outside",
      tabs: { count: 4 },
    });
    expect(grilleShapes({ x: 100, y: 70 })).toHaveLength(5);
  });
  it("transforms exact dimensions about the shape centre", () => {
    const shape = createShape(
      { kind: "rectangle", width: 20, height: 10, radius: 0 },
      30,
      40,
    );
    const points = shapePoints({ ...shape, rotationDegrees: 90 });
    expect(Math.min(...points.map((p) => p.x))).toBeCloseTo(25);
    expect(Math.max(...points.map((p) => p.y))).toBeCloseTo(50);
    expect(snap(1.23, 0.1)).toBeCloseTo(1.2);
    expect(snap(1.23, 0)).toBe(1.23);
  });
  it("project save/load preserves all operations, tools and stock without generating code", () => {
    const doc = {
      ...emptySketch(),
      shapes: fanShapes(
        { opening: 70, pitch: 71.5, hole: 4.2, plate: 100 },
        { x: 100, y: 70 },
      ),
    };
    expect(decodeSketch(JSON.stringify({ version: 1, document: doc }))).toEqual(
      doc,
    );
    expect(validateSketch(doc, [])).toContain("выберите подходящий инструмент");
  });
  it("rejects corrupt, future and oversized project files before rendering", () => {
    const doc = emptySketch();
    for (const text of [
      "{",
      JSON.stringify({ version: 2, document: doc }),
      JSON.stringify({
        version: 1,
        document: { ...doc, stock: { ...doc.stock, widthMm: -1 } },
      }),
      " ".repeat(512_001),
    ])
      expect(() => decodeSketch(text)).toThrow();
    const s = createShape({ kind: "circle", diameter: 5 }, 20, 20);
    expect(() =>
      decodeSketch(
        JSON.stringify({ version: 1, document: { ...doc, shapes: [s, s] } }),
      ),
    ).toThrow();
  });
  it("undo/redo keeps whole transactions including project replacement", () => {
    const doc = emptySketch();
    const next = {
      ...doc,
      shapes: [createShape({ kind: "circle", diameter: 5 }, 20, 20)],
    };
    const edited = sketchHistory(
      { past: [], present: doc, future: [] },
      { type: "edit", document: next },
    );
    const undone = sketchHistory(edited, { type: "undo" });
    expect(undone.present).toBe(doc);
    expect(sketchHistory(undone, { type: "redo" }).present).toBe(next);
    expect(
      sketchHistory(undone, {
        type: "edit",
        document: { ...doc, sourceName: "other.nc" },
      }).future,
    ).toEqual([]);
  });
});
