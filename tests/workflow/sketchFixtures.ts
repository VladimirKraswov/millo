import {
  createShape,
  emptySketch,
} from "../../src/plugins/quick-sketch/sketchModel";

export function panelProject() {
  const shape = (
    name: string,
    geometry: Parameters<typeof createShape>[0],
    x: number,
    y: number,
  ) => {
    const s = createShape(geometry, x, y);
    return {
      ...s,
      name,
      operation: { ...s.operation, toolId: "preset-carbide3d-102" },
    };
  };
  const holes = [
    [70, 40],
    [70, 100],
    [130, 40],
    [130, 100],
  ].map(([x, y], i) =>
    shape(`Крепление ${i + 1}`, { kind: "circle", diameter: 4.2 }, x, y),
  );
  const opening = shape("Проём", { kind: "circle", diameter: 70 }, 100, 70);
  const plate = shape(
    "Панель",
    { kind: "rectangle", width: 100, height: 100, radius: 4 },
    100,
    70,
  );
  return {
    version: 2,
    document: {
      ...emptySketch(),
      shapes: [
        ...holes,
        {
          ...opening,
          operation: {
            ...opening.operation,
            kind: "inside",
            tabs: { count: 4, widthMm: 3, heightMm: 0.6 },
          },
        },
        plate,
      ],
    },
  };
}
