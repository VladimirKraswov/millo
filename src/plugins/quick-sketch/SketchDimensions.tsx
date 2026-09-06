import type {
  SketchJobRequest,
  SketchPoint,
  SketchShape,
} from "../../shared/sketch";
import { anchorOffset, anchorPoint, referencePoint } from "./sketchConstraints";

const number = (value: number) => Number(Math.abs(value).toFixed(3)).toString();
function Dimension({
  a,
  b,
  horizontal,
  level,
  label,
  unit,
  linked,
}: {
  readonly a: SketchPoint;
  readonly b: SketchPoint;
  readonly horizontal: boolean;
  readonly level: number;
  readonly label: string;
  readonly unit: number;
  readonly linked?: boolean;
}) {
  const d = horizontal
    ? `M ${a.x} ${-a.y} V ${-level} H ${b.x} V ${-b.y}`
    : `M ${a.x} ${-a.y} H ${level} V ${-b.y} H ${b.x}`;
  const x = horizontal ? (a.x + b.x) / 2 : level - unit * 2;
  const y = horizontal ? -level - unit * 1.3 : -(a.y + b.y) / 2;
  const tick = (p: SketchPoint) =>
    horizontal
      ? `M ${p.x - unit} ${-level - unit} l ${unit * 2} ${unit * 2}`
      : `M ${level - unit} ${-p.y - unit} l ${unit * 2} ${unit * 2}`;
  return (
    <g
      className={`sketch-dimension${linked ? " is-linked" : ""}`}
      data-dimension={label}
    >
      <path d={d} vectorEffect="non-scaling-stroke" />
      <path d={`${tick(a)} ${tick(b)}`} vectorEffect="non-scaling-stroke" />
      <text
        x={x}
        y={y}
        textAnchor="middle"
        fontSize={unit * 3.1}
        transform={horizontal ? undefined : `rotate(-90 ${x} ${y})`}
      >
        {label}
      </text>
    </g>
  );
}
export function SketchDimensions({
  document: doc,
  selection,
  unit,
}: {
  readonly document: SketchJobRequest;
  readonly selection: readonly string[];
  readonly unit: number;
}) {
  const selected = doc.shapes.filter((s) => selection.includes(s.id));
  const last = doc.shapes.find((s) => s.id === selection[selection.length - 1]);
  const bounds = (s: SketchShape) => ({
    minX: s.xMm + anchorOffset(s, "x", "min"),
    maxX: s.xMm + anchorOffset(s, "x", "max"),
    minY: s.yMm + anchorOffset(s, "y", "min"),
    maxY: s.yMm + anchorOffset(s, "y", "max"),
  });
  const size = last && bounds(last);
  return (
    <g className="sketch-dimensions">
      {last && size && (
        <>
          <Dimension
            a={{ x: size.minX, y: size.maxY }}
            b={{ x: size.maxX, y: size.maxY }}
            horizontal
            level={size.maxY + unit * 7}
            label={`${last.geometry.kind === "circle" ? "Ø " : ""}${number(size.maxX - size.minX)} мм`}
            unit={unit}
          />
          {last.geometry.kind !== "circle" && (
            <Dimension
              a={{ x: size.maxX, y: size.minY }}
              b={{ x: size.maxX, y: size.maxY }}
              horizontal={false}
              level={size.maxX + unit * 7}
              label={`${number(size.maxY - size.minY)} мм`}
              unit={unit}
            />
          )}
        </>
      )}
      {selected.flatMap((shape, index) =>
        (["x", "y"] as const).flatMap((axis) => {
          const c = shape.constraints?.[axis];
          if (!c) return [];
          const a = referencePoint(doc, c),
            b = anchorPoint(shape, c.ownAnchor),
            box = bounds(shape);
          return [
            <Dimension
              key={`${shape.id}-${axis}`}
              a={a}
              b={b}
              horizontal={axis === "x"}
              linked
              level={
                axis === "x"
                  ? Math.min(a.y, box.minY) - unit * (8 + index * 5)
                  : Math.min(a.x, box.minX) - unit * (8 + index * 5)
              }
              label={
                c.offsetMm === 0
                  ? `${axis.toUpperCase()} =`
                  : `${axis.toUpperCase()} ${number(c.offsetMm)} мм`
              }
              unit={unit}
            />,
          ];
        }),
      )}
    </g>
  );
}
