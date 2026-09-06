import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type PointerEvent,
} from "react";
import { Check, X } from "lucide-react";
import type {
  GeneratedSketchJob,
  SketchGeometry,
  SketchJobRequest,
  SketchPoint,
  SketchShape,
} from "../../shared/sketch";
import { shapePoints, snap, svgPoints, type DrawMode } from "./sketchModel";
import {
  anchorOffset,
  moveSketchShape,
  resolveSketch,
} from "./sketchConstraints";
import { SketchDimensions } from "./SketchDimensions";

interface Props {
  readonly document: SketchJobRequest;
  readonly selection: readonly string[];
  readonly dragEnabled: boolean;
  readonly showDimensions: boolean;
  readonly mode: DrawMode;
  readonly grid: number;
  readonly resetView: number;
  readonly cancelDrawing: number;
  readonly onDrawingChange: (active: boolean) => void;
  readonly generated?: GeneratedSketchJob;
  readonly onSelect: (id?: string, additive?: boolean) => void;
  readonly onMove: (id: string, point: SketchPoint) => void;
  readonly onCreate: (geometry: SketchGeometry, point: SketchPoint) => void;
}
type Gesture = {
  readonly start: SketchPoint;
  readonly current: SketchPoint;
  readonly shape?: SketchShape;
  readonly pan?: { x: number; y: number };
  readonly pointerId: number;
};

export function SketchCanvas({
  document: doc,
  selection,
  dragEnabled,
  showDimensions,
  mode,
  grid,
  resetView,
  cancelDrawing,
  onDrawingChange,
  generated,
  onSelect,
  onMove,
  onCreate,
}: Props) {
  const selectedId = selection[selection.length - 1];
  const svg = useRef<SVGSVGElement>(null);
  const [viewport, setViewport] = useState({ width: 800, height: 500 });
  useLayoutEffect(() => {
    const element = svg.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) =>
      setViewport({
        width: entry.contentRect.width,
        height: entry.contentRect.height,
      }),
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  const [view, setView] = useState({
    x: -15,
    y: -doc.stock.heightMm - 15,
    width: doc.stock.widthMm + 30,
    height: doc.stock.heightMm + 30,
  });
  const [gesture, setGesture] = useState<Gesture>();
  const [polygon, setPolygon] = useState<SketchPoint[]>([]);
  const [cursor, setCursor] = useState<SketchPoint>();
  const drawing = polygon.length > 0 || Boolean(gesture);
  useLayoutEffect(() => {
    onDrawingChange(drawing);
  }, [drawing, onDrawingChange]);
  useEffect(() => {
    setPolygon([]);
    setGesture(undefined);
  }, [cancelDrawing]);
  useEffect(() => {
    setView({
      x: -15,
      y: -doc.stock.heightMm - 15,
      width: doc.stock.widthMm + 30,
      height: doc.stock.heightMm + 30,
    });
  }, [resetView, doc.stock.widthMm, doc.stock.heightMm]);
  useEffect(() => {
    setPolygon([]);
    setGesture(undefined);
  }, [mode]);
  const coordinates = (clientX: number, clientY: number): SketchPoint => {
    const matrix = svg.current?.getScreenCTM();
    if (!matrix) return { x: 0, y: 0 };
    const p = new DOMPoint(clientX, clientY).matrixTransform(matrix.inverse());
    return { x: p.x, y: -p.y };
  };
  useEffect(() => {
    const element = svg.current;
    if (!element) return;
    const zoom = (event: WheelEvent) => {
      event.preventDefault();
      const p = coordinates(event.clientX, event.clientY);
      const scale = event.deltaY > 0 ? 1.12 : 1 / 1.12;
      setView((v) => {
        const factor = Math.max(5, Math.min(30_000, v.width * scale)) / v.width;
        return {
          x: p.x + (v.x - p.x) * factor,
          y: -p.y + (v.y + p.y) * factor,
          width: v.width * factor,
          height: v.height * factor,
        };
      });
    };
    element.addEventListener("wheel", zoom, { passive: false });
    return () => element.removeEventListener("wheel", zoom);
  }, []);
  const pointAt = (e: PointerEvent) => {
    const p = coordinates(e.clientX, e.clientY);
    return { x: snap(p.x, grid), y: snap(p.y, grid) };
  };
  const finishPolygon = () => {
    if (polygon.length < 3) return;
    const minX = Math.min(...polygon.map((p) => p.x)),
      maxX = Math.max(...polygon.map((p) => p.x));
    const minY = Math.min(...polygon.map((p) => p.y)),
      maxY = Math.max(...polygon.map((p) => p.y));
    const center = { x: (minX + maxX) / 2, y: (minY + maxY) / 2 };
    onCreate(
      {
        kind: "polygon",
        points: polygon.map((p) => ({ x: p.x - center.x, y: p.y - center.y })),
      },
      center,
    );
    setPolygon([]);
  };
  const start = (event: PointerEvent<SVGSVGElement>) => {
    if (event.button !== 0 && event.button !== 1) return;
    event.preventDefault();
    svg.current?.focus();
    const p = pointAt(event);
    if (mode === "polygon" && event.button === 0) {
      if (
        polygon.length >= 3 &&
        Math.hypot(p.x - polygon[0].x, p.y - polygon[0].y) < view.width / 60
      )
        finishPolygon();
      else if (polygon.length < 256) setPolygon((prev) => [...prev, p]);
      return;
    }
    svg.current?.setPointerCapture(event.pointerId);
    if (mode === "pan" || event.button === 1) {
      setGesture({
        start: coordinates(event.clientX, event.clientY),
        current: p,
        pan: { x: view.x, y: view.y },
        pointerId: event.pointerId,
      });
      return;
    }
    const target = event.target as Element;
    const id =
      target.closest("[data-shape-id]")?.getAttribute("data-shape-id") ??
      undefined;
    const shape = doc.shapes.find((s) => s.id === id);
    if (mode === "select") {
      const additive = event.shiftKey || event.metaKey || event.ctrlKey;
      onSelect(id, additive);
      if (
        !dragEnabled ||
        additive ||
        !shape ||
        shape.locked ||
        (shape.constraints?.x && shape.constraints?.y)
      )
        return;
    }
    setGesture({
      start: p,
      current: p,
      shape: mode === "select" ? shape : undefined,
      pointerId: event.pointerId,
    });
  };
  const move = (event: PointerEvent<SVGSVGElement>) => {
    const p = pointAt(event);
    setCursor(p);
    if (!gesture || gesture.pointerId !== event.pointerId) return;
    if (gesture.pan) {
      const raw = coordinates(event.clientX, event.clientY);
      setView((v) => ({
        ...v,
        x: v.x + gesture.start.x - raw.x,
        y: v.y - gesture.start.y + raw.y,
      }));
    } else setGesture({ ...gesture, current: p });
  };
  const end = (event: PointerEvent<SVGSVGElement>) => {
    if (!gesture || event.pointerId !== gesture.pointerId) return;
    if (!gesture.pan) {
      const p = pointAt(event),
        dx = p.x - gesture.start.x,
        dy = p.y - gesture.start.y;
      if (gesture.shape && Math.hypot(dx, dy) > 0.001)
        onMove(gesture.shape.id, {
          x: gesture.shape.xMm + dx,
          y: gesture.shape.yMm + dy,
        });
      else if (
        mode === "rectangle" &&
        Math.abs(dx) >= 0.1 &&
        Math.abs(dy) >= 0.1
      )
        onCreate(
          {
            kind: "rectangle",
            width: Math.abs(dx),
            height: Math.abs(dy),
            radius: 0,
          },
          { x: (p.x + gesture.start.x) / 2, y: (p.y + gesture.start.y) / 2 },
        );
      else if (mode === "circle" && Math.hypot(dx, dy) >= 0.1)
        onCreate(
          { kind: "circle", diameter: Math.hypot(dx, dy) * 2 },
          gesture.start,
        );
    }
    setGesture(undefined);
    if (svg.current?.hasPointerCapture(event.pointerId))
      svg.current.releasePointerCapture(event.pointerId);
  };
  const selected = doc.shapes.find((s) => s.id === selectedId);
  let displayDoc = doc;
  if (gesture?.shape) {
    const moved = moveSketchShape(gesture.shape, {
      x: gesture.shape.xMm + gesture.current.x - gesture.start.x,
      y: gesture.shape.yMm + gesture.current.y - gesture.start.y,
    });
    try {
      displayDoc = resolveSketch({
        ...doc,
        shapes: doc.shapes.map((s) => (s.id === moved.id ? moved : s)),
      });
    } catch {
      /* Keep the last valid layout outside the supported coordinate range. */
    }
  }
  const minorGrid = Math.max(
    grid || 1,
    10 ** Math.ceil(Math.log10(view.width / 40)),
  );
  return (
    <div className="sketch-canvas-wrap">
      {generated && (
        <div className="sketch-legend">
          <span>Траектория</span>
          <span>Перемычки</span>
        </div>
      )}
      <svg
        ref={svg}
        className={`sketch-canvas is-${mode}${dragEnabled ? " can-drag" : ""}`}
        viewBox={`${view.x} ${view.y} ${view.width} ${view.height}`}
        role="application"
        aria-label="Чертёж заготовки"
        tabIndex={0}
        onPointerDown={start}
        onPointerMove={move}
        onPointerUp={end}
        onPointerCancel={() => setGesture(undefined)}
        onLostPointerCapture={() => setGesture(undefined)}
        onKeyDown={(e) => {
          if (e.key === "Escape" && (polygon.length || gesture)) {
            e.stopPropagation();
            setPolygon([]);
            setGesture(undefined);
          }
          if (e.key === "Enter" && polygon.length >= 3) {
            e.preventDefault();
            finishPolygon();
          }
        }}
      >
        <defs>
          <pattern
            id="sketch-grid"
            width={minorGrid}
            height={minorGrid}
            patternUnits="userSpaceOnUse"
          >
            <path
              d={`M ${minorGrid} 0 L 0 0 0 ${minorGrid}`}
              fill="none"
              stroke="#334248"
              strokeWidth="0.5"
              vectorEffect="non-scaling-stroke"
            />
          </pattern>
        </defs>
        <rect
          x={0}
          y={-doc.stock.heightMm}
          width={Math.max(1, doc.stock.widthMm)}
          height={Math.max(1, doc.stock.heightMm)}
          fill="#172024"
          stroke="#6a7b82"
          strokeWidth="1"
          vectorEffect="non-scaling-stroke"
        />
        <rect
          x={0}
          y={-doc.stock.heightMm}
          width={Math.max(1, doc.stock.widthMm)}
          height={Math.max(1, doc.stock.heightMm)}
          fill="url(#sketch-grid)"
        />
        {[...displayDoc.shapes]
          .sort((a, b) => {
            const area = (s: SketchShape) =>
              (anchorOffset(s, "x", "max") - anchorOffset(s, "x", "min")) *
              (anchorOffset(s, "y", "max") - anchorOffset(s, "y", "min"));
            return area(b) - area(a);
          })
          .map((shape) => {
            const points = shapePoints(shape);
            return (
              <g
                key={shape.id}
                data-shape-id={shape.id}
                className={`sketch-figure is-${shape.operation.kind}${selection.includes(shape.id) ? " is-selected" : ""}${shape.locked ? " is-locked" : ""}`}
              >
                <title>{shape.name}</title>
                <polygon
                  points={svgPoints(points)}
                  vectorEffect="non-scaling-stroke"
                />
                <polygon
                  className="sketch-hit-outline"
                  points={svgPoints(points)}
                  vectorEffect="non-scaling-stroke"
                />
                {selection.includes(shape.id) && (
                  <g
                    className="sketch-center"
                    transform={`translate(${shape.xMm} ${-shape.yMm})`}
                  >
                    <path
                      d={`M ${-view.width / 130} 0 H ${view.width / 130} M 0 ${-view.width / 130} V ${view.width / 130}`}
                      vectorEffect="non-scaling-stroke"
                    />
                  </g>
                )}
              </g>
            );
          })}
        {generated?.summary.paths.map((path, i) => (
          <polyline
            key={i}
            className="sketch-cam-path"
            points={svgPoints([...path.points, path.points[0]])}
            vectorEffect="non-scaling-stroke"
          />
        ))}
        {generated?.summary.tabPaths.map((path, i) => (
          <polyline
            key={`tab-${i}`}
            className="sketch-tab-path"
            points={svgPoints(path.points)}
            vectorEffect="non-scaling-stroke"
          >
            <title>Перемычка</title>
          </polyline>
        ))}
        {showDimensions && !gesture && (
          <SketchDimensions
            document={displayDoc}
            selection={selection}
            unit={
              3.5 /
              Math.max(
                0.01,
                Math.min(
                  viewport.width / view.width,
                  viewport.height / view.height,
                ),
              )
            }
          />
        )}
        {gesture && mode === "rectangle" && (
          <rect
            className="sketch-drawing"
            x={Math.min(gesture.start.x, gesture.current.x)}
            y={-Math.max(gesture.start.y, gesture.current.y)}
            width={Math.abs(gesture.current.x - gesture.start.x)}
            height={Math.abs(gesture.current.y - gesture.start.y)}
            vectorEffect="non-scaling-stroke"
          />
        )}
        {gesture && mode === "circle" && (
          <circle
            className="sketch-drawing"
            cx={gesture.start.x}
            cy={-gesture.start.y}
            r={Math.hypot(
              gesture.current.x - gesture.start.x,
              gesture.current.y - gesture.start.y,
            )}
            vectorEffect="non-scaling-stroke"
          />
        )}
        {polygon.length > 0 && (
          <polyline
            className="sketch-drawing"
            points={svgPoints([...polygon, ...(cursor ? [cursor] : [])])}
            vectorEffect="non-scaling-stroke"
          />
        )}
        <g className="sketch-origin" fontSize={view.width / 65}>
          <path d="M 0 -8 V 0 H 8" vectorEffect="non-scaling-stroke" />
          <text x={-1} y={view.width / 55} textAnchor="end">
            0
          </text>
          <text
            x={doc.stock.widthMm / 2}
            y={view.width / 45}
            textAnchor="middle"
          >
            X · {doc.stock.widthMm} мм
          </text>
          <text x={0} y={-doc.stock.heightMm - view.width / 65}>
            Y · {doc.stock.heightMm} мм
          </text>
        </g>
      </svg>
      <div className="sketch-canvas-status">
        <span>
          {cursor
            ? `X ${cursor.x.toFixed(2)} · Y ${cursor.y.toFixed(2)}`
            : "X0 Y0 · левый нижний угол"}
        </span>
        <span>{selected?.name ?? `${doc.shapes.length} фигур`}</span>
      </div>
      {polygon.length > 0 && (
        <div className="sketch-polygon-actions">
          <span>{polygon.length} вершин</span>
          <button
            type="button"
            disabled={polygon.length < 3}
            onClick={finishPolygon}
          >
            <Check size={16} />
            Замкнуть
          </button>
          <button
            type="button"
            title="Отменить контур"
            aria-label="Отменить контур"
            onClick={() => setPolygon([])}
          >
            <X size={16} />
          </button>
        </div>
      )}
    </div>
  );
}
