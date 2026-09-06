import type {
  SketchJobRequest,
  SketchPoint,
  SketchShape,
} from "../../shared/sketch";
import { anchorOffset, anchorPoint, referencePoint } from "./sketchConstraints";
import {
  localGeometryBounds,
  type SketchDimensionTarget,
} from "./sketchDimensionModel";
import type { SketchDimensionEditorState } from "./SketchDimensionEditor";

const number = (value: number) => Number(Math.abs(value).toFixed(3)).toString();
export type BeginDimensionEdit = (
  edit: Omit<SketchDimensionEditorState, "left" | "top">,
  bounds: DOMRect,
) => void;

function Dimension({
  a,
  b,
  horizontal,
  level,
  label,
  unit,
  linked,
  target,
  value,
  editLabel,
  onEdit,
  flip = false,
}: {
  readonly a: SketchPoint;
  readonly b: SketchPoint;
  readonly horizontal: boolean;
  readonly level: number;
  readonly label: string;
  readonly unit: number;
  readonly linked?: boolean;
  readonly target?: SketchDimensionTarget;
  readonly value: number;
  readonly editLabel: string;
  readonly onEdit: BeginDimensionEdit;
  readonly flip?: boolean;
}) {
  const d = horizontal
    ? `M ${a.x} ${-a.y} V ${-level} H ${b.x} V ${-b.y}`
    : `M ${a.x} ${-a.y} H ${level} V ${-b.y} H ${b.x}`;
  const x = horizontal ? (a.x + b.x) / 2 : level - unit * 2;
  const y = horizontal ? -level - unit * 1.8 : -(a.y + b.y) / 2;
  const angle = (horizontal ? 0 : -90) + (flip ? 180 : 0);
  const tick = (p: SketchPoint) =>
    horizontal
      ? `M ${p.x - unit} ${-level - unit} l ${unit * 2} ${unit * 2}`
      : `M ${level - unit} ${-p.y - unit} l ${unit * 2} ${unit * 2}`;
  const begin = (element: SVGGElement) =>
    target &&
    onEdit(
      { target, value, label: editLabel },
      element.getBoundingClientRect(),
    );
  return (
    <g
      className={`sketch-dimension${linked ? " is-linked" : ""}`}
      data-dimension={label}
    >
      <path d={d} vectorEffect="non-scaling-stroke" />
      <path d={`${tick(a)} ${tick(b)}`} vectorEffect="non-scaling-stroke" />
      <g
        className={`sketch-dimension-label${target ? " is-editable" : ""}`}
        transform={`translate(${x} ${y}) rotate(${angle})`}
        role={target ? "button" : undefined}
        tabIndex={target ? 0 : undefined}
        aria-label={target ? `Изменить ${editLabel}` : editLabel}
        onPointerDown={(e) => e.stopPropagation()}
        onClick={(e) => e.stopPropagation()}
        onDoubleClick={(e) => {
          e.stopPropagation();
          begin(e.currentTarget);
        }}
        onKeyDown={(e) => {
          if (target && (e.key === "Enter" || e.key === " ")) {
            e.preventDefault();
            e.stopPropagation();
            begin(e.currentTarget);
          }
        }}
      >
        <title>
          {target
            ? `${editLabel} · двойной щелчок для изменения`
            : `${editLabel} · положение защищено`}
        </title>
        <rect
          x={-Math.max(7, label.length * 1.1) * unit}
          y={-unit * 4.2}
          width={Math.max(14, label.length * 2.2) * unit}
          height={unit * 6}
          rx={unit}
        />
        <text textAnchor="middle" fontSize={unit * 3.1}>
          {label}
        </text>
      </g>
    </g>
  );
}

export function SketchDimensions({
  document: doc,
  selection,
  unit,
  onEdit,
}: {
  readonly document: SketchJobRequest;
  readonly selection: readonly string[];
  readonly unit: number;
  readonly onEdit: BeginDimensionEdit;
}) {
  const selected = doc.shapes.filter((s) => selection.includes(s.id));
  const last = doc.shapes.find((s) => s.id === selection[selection.length - 1]);
  const size = last && localGeometryBounds(last.geometry);
  const bounds = (s: SketchShape) => ({
    minX: s.xMm + anchorOffset(s, "x", "min"),
    minY: s.yMm + anchorOffset(s, "y", "min"),
  });
  const flips = (rotation: number) =>
    Math.cos((rotation * Math.PI) / 180) < -1e-9;
  return (
    <g className="sketch-dimensions">
      {last && size && (
        <g
          transform={`translate(${last.xMm} ${-last.yMm}) rotate(${-last.rotationDegrees})`}
        >
          <Dimension
            a={{ x: size.minX, y: size.maxY }}
            b={{ x: size.maxX, y: size.maxY }}
            horizontal
            level={size.maxY + unit * 7}
            label={`${last.geometry.kind === "circle" ? "Ø " : ""}${number(size.maxX - size.minX)} мм`}
            unit={unit}
            target={{ shapeId: last.id, kind: "size", axis: "x" }}
            value={size.maxX - size.minX}
            editLabel={last.geometry.kind === "circle" ? "диаметр" : "ширину"}
            onEdit={onEdit}
            flip={flips(-last.rotationDegrees)}
          />
          {last.geometry.kind !== "circle" && (
            <Dimension
              a={{ x: size.maxX, y: size.minY }}
              b={{ x: size.maxX, y: size.maxY }}
              horizontal={false}
              level={size.maxX + unit * 7}
              label={`${number(size.maxY - size.minY)} мм`}
              unit={unit}
              target={{ shapeId: last.id, kind: "size", axis: "y" }}
              value={size.maxY - size.minY}
              editLabel="высоту"
              onEdit={onEdit}
              flip={flips(-last.rotationDegrees - 90)}
            />
          )}
        </g>
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
                  : `${axis.toUpperCase()} ${c.offsetMm < 0 ? "−" : ""}${number(c.offsetMm)} мм`
              }
              unit={unit}
              target={
                shape.locked
                  ? undefined
                  : { shapeId: shape.id, kind: "offset", axis }
              }
              value={c.offsetMm}
              editLabel={`смещение ${axis.toUpperCase()}`}
              onEdit={onEdit}
            />,
          ];
        }),
      )}
    </g>
  );
}
