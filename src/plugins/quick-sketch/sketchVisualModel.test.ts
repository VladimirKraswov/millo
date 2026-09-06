import { describe, expect, it } from "vitest";
import { newToolDraft, type CuttingTool } from "../../shared/tooling";
import { createShape, emptySketch } from "./sketchModel";
import {
  applySketchDimension,
  resizeSketchGeometry,
} from "./sketchDimensionModel";
import { sketchCutterVisual } from "./sketchOperationVisual";

const tool: CuttingTool = {
  ...newToolDraft(),
  id: "endmill",
  name: "End mill",
  kind: "flatEndMill",
  diameterMm: 4,
  factoryPreset: false,
};
const stock = emptySketch().stock;
const circle = () =>
  createShape({ kind: "circle", diameter: 20 }, 30, 40, tool);

describe("machining glyph geometry", () => {
  it("places a real-size cutter inside, outside, on the contour and inside a pocket, including rotation", () => {
    const s = circle();
    for (const [kind, offset] of [
      ["inside", 8],
      ["outside", 12],
      ["engrave", 10],
      ["pocket", 8],
    ] as const) {
      const shape = {
        ...s,
        rotationDegrees: 90,
        operation: { ...s.operation, kind },
      };
      const marker = sketchCutterVisual(shape, stock, tool);
      expect(marker.warning).toBeUndefined();
      expect(marker.diameterMm).toBe(4);
      expect(marker.center.x).toBeCloseTo(30);
      expect(marker.center.y).toBeCloseTo(40 + offset);
      expect(marker.contact!.y).toBeCloseTo(50);
    }
  });
  it("keeps tangency on the correct side for both polygon windings and rotated rectangles", () => {
    const s = circle();
    const points = [
      { x: -10, y: -5 },
      { x: 10, y: -5 },
      { x: 10, y: 5 },
      { x: -10, y: 5 },
    ];
    for (const p of [points, [...points].reverse()]) {
      const shape = {
        ...s,
        geometry: { kind: "polygon" as const, points: p },
        operation: { ...s.operation, kind: "inside" as const },
      };
      const marker = sketchCutterVisual(shape, stock, tool);
      expect(marker.warning).toBeUndefined();
      expect(Math.abs(marker.center.y - 40)).toBe(3);
      expect(
        Math.hypot(
          marker.center.x - marker.contact!.x,
          marker.center.y - marker.contact!.y,
        ),
      ).toBe(2);
    }
    const rect = {
      ...s,
      rotationDegrees: 90,
      geometry: {
        kind: "rectangle" as const,
        width: 20,
        height: 10,
        radius: 0,
      },
    };
    const marker = sketchCutterVisual(rect, stock, tool);
    expect(Math.abs(marker.center.x - 30)).toBeCloseTo(3);
    expect(marker.center.y).toBeCloseTo(40);
  });
  it("does not pretend an oversized or missing tool fits and centers a matching drill", () => {
    const s = circle();
    expect(sketchCutterVisual(s, stock).warning).toBeTruthy();
    expect(
      sketchCutterVisual(s, stock, { ...tool, diameterMm: 30 }).warning,
    ).toBeTruthy();
    const drill = {
      ...s,
      operation: { ...s.operation, kind: "drill" as const },
    };
    expect(
      sketchCutterVisual(drill, stock, {
        ...tool,
        kind: "drill",
        diameterMm: 20,
      }),
    ).toEqual({ center: { x: 30, y: 40 }, diameterMm: 20, warning: undefined });
    expect(
      sketchCutterVisual(drill, stock, { ...tool, kind: "drill" }).warning,
    ).toBeTruthy();
  });
  it("uses the engraving tip width at the actual depth, not the shank width", () => {
    const s = circle();
    const vbit: CuttingTool = {
      ...tool,
      kind: "vBit",
      tipDiameterMm: 0.1,
      includedAngleDegrees: 90,
    };
    const shape = {
      ...s,
      operation: {
        ...s.operation,
        kind: "engrave" as const,
        through: false,
        depthMm: 0.2,
      },
    };
    expect(sketchCutterVisual(shape, stock, vbit).diameterMm).toBeCloseTo(0.5);
    expect(
      sketchCutterVisual(
        { ...shape, operation: { ...shape.operation, depthMm: 0.5 } },
        stock,
        vbit,
      ).diameterMm,
    ).toBeCloseTo(1.1);
  });
});

describe("direct drawing dimensions", () => {
  it("resizes local geometry, clamps rounded corners and scales polygon vertices around their bounds", () => {
    expect(
      resizeSketchGeometry(
        { kind: "rectangle", width: 20, height: 10, radius: 5 },
        "x",
        4,
      ),
    ).toEqual({ kind: "rectangle", width: 4, height: 10, radius: 2 });
    expect(
      resizeSketchGeometry(
        {
          kind: "polygon",
          points: [
            { x: 1, y: 1 },
            { x: 5, y: 1 },
            { x: 5, y: 3 },
          ],
        },
        "x",
        8,
      ),
    ).toEqual({
      kind: "polygon",
      points: [
        { x: -1, y: 1 },
        { x: 7, y: 1 },
        { x: 7, y: 3 },
      ],
    });
    const s = { ...circle(), rotationDegrees: 90 };
    const updated = applySketchDimension(
      { ...emptySketch(), shapes: [s] },
      { shapeId: s.id, kind: "size", axis: "x" },
      6,
    );
    expect(updated.shapes[0]).toMatchObject({
      xMm: 30,
      yMm: 40,
      rotationDegrees: 90,
      geometry: { diameter: 6 },
    });
    for (const value of [NaN, Infinity, -1, 0, 10001])
      expect(() => resizeSketchGeometry(s.geometry, "x", value)).toThrow();
  });
  it("resolves dependent shapes and signed distances without modifying the source document", () => {
    const a = {
      ...circle(),
      id: "a",
      constraints: {
        x: {
          referenceAnchor: "min" as const,
          ownAnchor: "min" as const,
          offsetMm: 10,
        },
      },
    };
    const b = {
      ...circle(),
      id: "b",
      constraints: {
        x: {
          referenceId: "a",
          referenceAnchor: "center" as const,
          ownAnchor: "center" as const,
          offsetMm: 30,
        },
      },
    };
    const original = { ...emptySketch(), shapes: [a, b] };
    const resized = applySketchDimension(
      original,
      { shapeId: "a", kind: "size", axis: "x" },
      10,
    );
    expect(resized.shapes.map((s) => s.xMm)).toEqual([15, 45]);
    const offset = applySketchDimension(
      resized,
      { shapeId: "b", kind: "offset", axis: "x" },
      -5,
    );
    expect(offset.shapes[1].xMm).toBe(10);
    expect(original.shapes[0].geometry).toEqual({
      kind: "circle",
      diameter: 20,
    });
    const locked = { ...original, shapes: [{ ...a, locked: true }, b] };
    expect(() =>
      applySketchDimension(
        locked,
        { shapeId: "a", kind: "offset", axis: "x" },
        5,
      ),
    ).toThrow("разблокируйте");
    expect(() =>
      applySketchDimension(
        original,
        { shapeId: "a", kind: "offset", axis: "y" },
        5,
      ),
    ).toThrow("больше не существует");
    expect(() =>
      applySketchDimension(
        original,
        { shapeId: "missing", kind: "size", axis: "x" },
        5,
      ),
    ).toThrow();
  });
});
