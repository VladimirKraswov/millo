import { useMemo } from "react";
import type { SketchJobRequest } from "../../shared/sketch";
import type { CuttingTool } from "../../shared/tooling";
import { operationLabels } from "./sketchModel";
import { sketchCutterVisual } from "./sketchOperationVisual";

export function SketchOperationOverlay({
  document: doc,
  tools,
  selection,
  unit,
}: {
  readonly document: SketchJobRequest;
  readonly tools: readonly CuttingTool[];
  readonly selection: readonly string[];
  readonly unit: number;
}) {
  const visuals = useMemo(
    () =>
      doc.shapes.map((shape) => ({
        shape,
        tool: tools.find((t) => t.id === shape.operation.toolId),
        marker: sketchCutterVisual(
          shape,
          doc.stock,
          tools.find((t) => t.id === shape.operation.toolId),
        ),
      })),
    [doc, tools],
  );
  return (
    <g className="sketch-operation-overlay" pointerEvents="none">
      {visuals.map(({ shape, marker, tool }) => {
        const selected = selection.includes(shape.id);
        const r = (marker.diameterMm ?? 0) / 2;
        const { x, y } = marker.center;
        return (
          <g
            key={shape.id}
            data-cutter-for={shape.id}
            data-operation={shape.operation.kind}
            data-diameter-mm={marker.diameterMm}
            data-warning={marker.warning ? "true" : undefined}
            className={`sketch-cutter is-${shape.operation.kind}${selected ? " is-selected" : ""}${marker.warning ? " is-warning" : ""}`}
          >
            <title>{`${shape.name} · ${operationLabels[shape.operation.kind]} · ${marker.warning ?? `${tool?.name} · Ø${Number(marker.diameterMm?.toFixed(3))} мм на глубине реза`}`}</title>
            {!marker.warning && marker.diameterMm && (
              <>
                <circle
                  className="sketch-cutter-footprint"
                  cx={x}
                  cy={-y}
                  r={r}
                  vectorEffect="non-scaling-stroke"
                />
                <path
                  d={`M ${x - r * 0.65} ${-y} H ${x + r * 0.65} M ${x} ${-y - r * 0.65} V ${-y + r * 0.65}`}
                  vectorEffect="non-scaling-stroke"
                />
                {marker.contact && shape.operation.kind !== "pocket" && (
                  <path
                    className="sketch-cutter-radius"
                    d={`M ${x} ${-y} L ${marker.contact.x} ${-marker.contact.y}`}
                    vectorEffect="non-scaling-stroke"
                  />
                )}
              </>
            )}
            {marker.warning && (
              <>
                <path
                  d={`M ${x - unit * 2} ${-y - unit * 2} l ${unit * 4} ${unit * 4} M ${x + unit * 2} ${-y - unit * 2} l ${-unit * 4} ${unit * 4}`}
                  vectorEffect="non-scaling-stroke"
                />
              </>
            )}
          </g>
        );
      })}
    </g>
  );
}
