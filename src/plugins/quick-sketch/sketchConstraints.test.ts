import { describe, expect, it } from "vitest";
import source from "../../../fixtures/sketch/constrained-holes.millo-sketch.json?raw";
import { decodeSketch } from "./sketchStorage";
import {
  arrangeShapes,
  moveSketchShape,
  removeSketchShapes,
  resolveSketch,
  anchorOffset,
} from "./sketchConstraints";
import type { SketchJobRequest } from "../../shared/sketch";

const fixture = () => JSON.parse(source).document as SketchJobRequest;
describe("directed sketch dimensions", () => {
  it("shares exact edge and centre semantics with the native CAM fixture", () => {
    const doc = decodeSketch(source);
    expect(doc.shapes.map((s) => [s.xMm, s.yMm])).toEqual([
      [12, 20],
      [42, 20],
    ]);
    const resized = resolveSketch({
      ...doc,
      shapes: doc.shapes.map((s, i) =>
        i ? s : { ...s, geometry: { kind: "circle", diameter: 10 } },
      ),
    });
    expect(resized.shapes.map((s) => s.xMm)).toEqual([15, 45]);
    expect(
      decodeSketch(JSON.stringify({ version: 2, document: resized })),
    ).toEqual(resized);
  });
  it("rejects cycles and dangling references without mutating the source", () => {
    const doc = fixture();
    expect(() =>
      resolveSketch({
        ...doc,
        shapes: doc.shapes.map((s, i) =>
          i
            ? s
            : {
                ...s,
                constraints: {
                  x: {
                    referenceId: "b",
                    referenceAnchor: "center",
                    ownAnchor: "center",
                    offsetMm: 0,
                  },
                },
              },
        ),
      }),
    ).toThrow("Циклическая");
    expect(doc.shapes[1].xMm).toBe(99);
    for (const bad of ["missing", "b"]) {
      const changed = fixture();
      (
        changed.shapes[1].constraints!.x! as { referenceId: string }
      ).referenceId = bad;
      expect(() =>
        decodeSketch(JSON.stringify({ version: 2, document: changed })),
      ).toThrow();
    }
  });
  it("aligns and spaces centres with persistent bindings, while locks prevent manual edits", () => {
    const doc = decodeSketch(source);
    const row = arrangeShapes(doc, ["a", "b"], "a", "x", 25);
    expect(row.shapes[1]).toMatchObject({ xMm: 37, yMm: 20 });
    const moved = moveSketchShape(row.shapes[0], { x: 70, y: 30 });
    expect(moved).toMatchObject({ xMm: 12, yMm: 30 });
    const next = resolveSketch({ ...row, shapes: [moved, row.shapes[1]] });
    expect(next.shapes[1].yMm).toBe(30);
    const locked = { ...moved, locked: true };
    expect(moveSketchShape(locked, { x: 50, y: 50 })).toBe(locked);
    expect(() =>
      arrangeShapes(
        { ...row, shapes: [row.shapes[0], { ...row.shapes[1], locked: true }] },
        ["a", "b"],
        "a",
        "y",
      ),
    ).toThrow("разблокируйте");
  });
  it("deleting a reference detaches its dimensions at the solved position", () => {
    const doc = removeSketchShapes(fixture(), ["a"]);
    expect(doc.shapes).toHaveLength(1);
    expect(doc.shapes[0]).toMatchObject({
      id: "b",
      xMm: 42,
      yMm: 20,
      constraints: {},
    });
    expect(resolveSketch(doc)).toEqual(doc);
  });
  it("uses analytic rotated rounded bounds and rejects invalid vertex indices", () => {
    const shape = {
      ...fixture().shapes[0],
      geometry: {
        kind: "rectangle" as const,
        width: 20,
        height: 10,
        radius: 2,
      },
      rotationDegrees: 45,
    };
    expect(anchorOffset(shape, "x", "max")).toBeCloseTo(2 + 11 / Math.sqrt(2));
    expect(() => anchorOffset(shape, "x", 1)).toThrow("многоугольника");
    const corrupt = JSON.parse(source);
    corrupt.document.shapes[0].constraints.x.offsetMm = "10";
    expect(() => decodeSketch(JSON.stringify(corrupt))).toThrow("Некорректный");
  });
});
