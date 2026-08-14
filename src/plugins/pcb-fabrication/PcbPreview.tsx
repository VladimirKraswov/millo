import { useRef } from "react";

import type { PcbInspection } from "../../shared/jobs";

export function PcbPreview({
  inspection,
  onMove,
}: {
  readonly inspection?: PcbInspection;
  readonly onMove: (deltaX: number, deltaY: number) => void;
}) {
  const drag = useRef<{ x: number; y: number; scale: number } | undefined>(undefined);
  if (!inspection) {
    return <div className="pcb-preview-empty"><span>Gerber preview</span><small>Перетащите файлы сюда</small></div>;
  }
  const bounds = inspection.bounds;
  const pad = Math.max(bounds.widthMm, bounds.heightMm) * 0.08 + 1;
  const viewBox = `${bounds.minXMm - pad} ${-(bounds.maxYMm + pad)} ${bounds.widthMm + pad * 2} ${bounds.heightMm + pad * 2}`;
  return (
    <svg
      aria-label="Предпросмотр печатной платы"
      className="pcb-preview-canvas"
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture(event.pointerId);
        const rect = event.currentTarget.getBoundingClientRect();
        drag.current = { x: event.clientX, y: event.clientY, scale: (bounds.widthMm + pad * 2) / Math.max(rect.width, 1) };
      }}
      onPointerMove={(event) => {
        if (!drag.current || !event.currentTarget.hasPointerCapture(event.pointerId)) return;
        const current = drag.current;
        const dx = (event.clientX - current.x) * current.scale;
        const dy = -(event.clientY - current.y) * current.scale;
        if (Math.abs(dx) + Math.abs(dy) < 0.02) return;
        drag.current = { ...current, x: event.clientX, y: event.clientY };
        onMove(dx, dy);
      }}
      onPointerUp={(event) => {
        drag.current = undefined;
        if (event.currentTarget.hasPointerCapture(event.pointerId)) {
          event.currentTarget.releasePointerCapture(event.pointerId);
        }
      }}
      viewBox={viewBox}
    >
      <g className="pcb-preview-grid">
        <path d={`M ${bounds.minXMm - pad} 0 H ${bounds.maxXMm + pad}`} />
        <path d={`M 0 ${-(bounds.minYMm - pad)} V ${-(bounds.maxYMm + pad)}`} />
      </g>
      {inspection.paths.map((path, index) => (
        <polygon
          className={`is-${path.role}`}
          key={`${path.role}-${index}`}
          points={path.points.map((point) => `${point.xMm},${-point.yMm}`).join(" ")}
        />
      ))}
      <g className="pcb-preview-drills">
        {inspection.drillHits.map((hit, index) => (
          <circle cx={hit.point.xMm} cy={-hit.point.yMm} key={`${hit.groupKey}-${index}`} r={0.22} />
        ))}
      </g>
    </svg>
  );
}
