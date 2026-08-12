import { Crosshair, ScanSearch } from "lucide-react";
import { useEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";

import type { GcodeProgram, ProgramPoint } from "../../shared/program";
import {
  buildToolpathHighlightReadModel,
  buildToolpathReadModel,
  buildToolPositionReadModel,
  sourceLineForIntersection,
  type ToolpathReadModel,
} from "./toolpathReadModel";

export type PreviewView = "top" | "iso";

interface ToolpathPreviewProps {
  readonly onSelectSourceLine?: (sourceLine: number) => void;
  readonly program: GcodeProgram;
  readonly selectedSourceLine?: number;
  readonly toolCoordinateSystem?: string;
  readonly toolPosition?: ProgramPoint;
  readonly view: PreviewView;
}

interface PreviewRuntime {
  readonly model: ToolpathReadModel;
  readonly renderer: THREE.WebGLRenderer;
  readonly rapidLine?: THREE.LineSegments;
  readonly rapidMaterial?: THREE.LineBasicMaterial;
  readonly cuttingLine?: THREE.LineSegments;
  readonly cuttingMaterial?: THREE.LineBasicMaterial;
  readonly focusProgram: () => void;
  readonly focusTool: () => void;
  readonly selectionLine: THREE.LineSegments;
  readonly selectionPoints: THREE.Points;
  readonly toolMarker: THREE.Points;
  readonly toolMarkerTexture: THREE.CanvasTexture;
  readonly toolProjection: THREE.Points;
  readonly toolProjectionLine: THREE.Line;
}

const formatCoordinate = (value: number): string =>
  `${value < 0 ? "−" : ""}${Math.abs(value).toFixed(3)}`;

const createToolMarkerTexture = (): THREE.CanvasTexture => {
  const canvas = document.createElement("canvas");
  canvas.width = 64;
  canvas.height = 64;
  const context = canvas.getContext("2d");
  if (context) {
    context.clearRect(0, 0, 64, 64);
    context.strokeStyle = "#0b1013";
    context.lineWidth = 10;
    context.beginPath();
    context.arc(32, 32, 18, 0, Math.PI * 2);
    context.stroke();
    context.strokeStyle = "#ffca73";
    context.lineWidth = 5;
    context.beginPath();
    context.arc(32, 32, 18, 0, Math.PI * 2);
    context.moveTo(32, 4);
    context.lineTo(32, 20);
    context.moveTo(32, 44);
    context.lineTo(32, 60);
    context.moveTo(4, 32);
    context.lineTo(20, 32);
    context.moveTo(44, 32);
    context.lineTo(60, 32);
    context.stroke();
    context.fillStyle = "#f4f7f8";
    context.beginPath();
    context.arc(32, 32, 4, 0, Math.PI * 2);
    context.fill();
  }
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  return texture;
};

export function ToolpathPreview({
  onSelectSourceLine,
  program,
  selectedSourceLine,
  toolCoordinateSystem = "G54",
  toolPosition,
  view,
}: ToolpathPreviewProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const runtimeRef = useRef<PreviewRuntime | undefined>(undefined);
  const toolOverProgram = useMemo(() => {
    const bounds = program.summary.bounds;
    return Boolean(
      toolPosition &&
      bounds &&
      toolPosition.x >= bounds.min.x &&
      toolPosition.x <= bounds.max.x &&
      toolPosition.y >= bounds.min.y &&
      toolPosition.y <= bounds.max.y,
    );
  }, [program.summary.bounds, toolPosition]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const model = buildToolpathReadModel(program);
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x0b1013);

    const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.01, 10_000);
    camera.up.set(0, 0, 1);
    if (view === "top") {
      camera.position.set(0, 0, model.frameRadius * 3);
      camera.up.set(0, 1, 0);
    } else {
      camera.position.set(
        model.frameRadius * 1.25,
        -model.frameRadius * 1.45,
        model.frameRadius * 1.2,
      );
    }
    camera.lookAt(0, 0, 0);

    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.domElement.setAttribute("aria-label", "Предпросмотр траектории G-code");
    host.replaceChildren(renderer.domElement);

    const grid = new THREE.GridHelper(
      model.gridSize,
      Math.min(40, Math.max(10, Math.round(model.gridSize / 2))),
      0x52616a,
      0x253038,
    );
    grid.rotation.x = Math.PI / 2;
    grid.position.z = model.gridZ - model.gridSize * 0.001;
    scene.add(grid);

    const addPath = (
      positions: Float32Array,
      color: number,
      opacity: number,
    ):
      | { readonly line: THREE.LineSegments; readonly material: THREE.LineBasicMaterial }
      | undefined => {
      if (positions.length === 0) return undefined;
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
      const material = new THREE.LineBasicMaterial({
        color,
        opacity,
        transparent: true,
      });
      const line = new THREE.LineSegments(geometry, material);
      scene.add(line);
      return { line, material };
    };
    const rapidPath = addPath(model.rapidPositions, 0xffb454, 0.68);
    const cuttingPath = addPath(model.cuttingPositions, 0x77d6b3, 1);

    const selectionLine = new THREE.LineSegments(
      new THREE.BufferGeometry(),
      new THREE.LineBasicMaterial({
        color: 0xf4f7f8,
        depthTest: false,
        transparent: true,
      }),
    );
    selectionLine.renderOrder = 10;
    const selectionPoints = new THREE.Points(
      new THREE.BufferGeometry(),
      new THREE.PointsMaterial({
        color: 0xffc76a,
        depthTest: false,
        size: 4,
        sizeAttenuation: false,
      }),
    );
    selectionPoints.renderOrder = 11;
    selectionLine.visible = false;
    selectionPoints.visible = false;
    scene.add(selectionLine, selectionPoints);

    const toolMarkerTexture = createToolMarkerTexture();
    const createToolPoint = (size: number, opacity: number) => {
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute(
        "position",
        new THREE.BufferAttribute(new Float32Array(3), 3),
      );
      const point = new THREE.Points(
        geometry,
        new THREE.PointsMaterial({
          alphaTest: 0.05,
          color: 0xffffff,
          depthTest: false,
          depthWrite: false,
          map: toolMarkerTexture,
          opacity,
          size,
          sizeAttenuation: false,
          transparent: true,
        }),
      );
      point.visible = false;
      return point;
    };
    const toolProjection = createToolPoint(18, 0.38);
    toolProjection.renderOrder = 20;
    const toolProjectionLine = new THREE.Line(
      new THREE.BufferGeometry().setAttribute(
        "position",
        new THREE.BufferAttribute(new Float32Array(6), 3),
      ),
      new THREE.LineDashedMaterial({
        color: 0xffca73,
        dashSize: Math.max(model.gridSize * 0.018, 0.3),
        depthTest: false,
        gapSize: Math.max(model.gridSize * 0.012, 0.2),
        opacity: 0.55,
        transparent: true,
      }),
    );
    toolProjectionLine.visible = false;
    toolProjectionLine.renderOrder = 21;
    const toolMarker = createToolPoint(34, 1);
    toolMarker.renderOrder = 22;
    scene.add(toolProjection, toolProjectionLine, toolMarker);

    const axes = new THREE.AxesHelper(Math.min(model.gridSize * 0.12, 8));
    scene.add(axes);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.enableRotate = view === "iso";
    controls.screenSpacePanning = true;
    controls.minZoom = 0.3;
    controls.maxZoom = 30;

    const raycaster = new THREE.Raycaster();
    let pointerStart: { x: number; y: number } | undefined;
    const onPointerDown = (event: PointerEvent) => {
      if (event.button === 0) {
        pointerStart = { x: event.clientX, y: event.clientY };
      }
    };
    const onPointerUp = (event: PointerEvent) => {
      if (!pointerStart || event.button !== 0) return;
      const distance = Math.hypot(
        event.clientX - pointerStart.x,
        event.clientY - pointerStart.y,
      );
      pointerStart = undefined;
      if (distance > 4) return;
      const rectangle = renderer.domElement.getBoundingClientRect();
      if (rectangle.width <= 0 || rectangle.height <= 0) return;
      const pointer = new THREE.Vector2(
        ((event.clientX - rectangle.left) / rectangle.width) * 2 - 1,
        -((event.clientY - rectangle.top) / rectangle.height) * 2 + 1,
      );
      raycaster.params.Line.threshold =
        Math.max(model.gridSize * 0.008, 0.12) / camera.zoom;
      raycaster.setFromCamera(pointer, camera);
      const paths = [rapidPath?.line, cuttingPath?.line].filter(
        (line): line is THREE.LineSegments => line !== undefined,
      );
      const hit = raycaster.intersectObjects(paths, false)[0];
      if (!hit) return;
      const sourceLines =
        hit.object === rapidPath?.line
          ? model.rapidSourceLines
          : model.cuttingSourceLines;
      const sourceLine = sourceLineForIntersection(sourceLines, hit.index);
      if (sourceLine !== undefined) onSelectSourceLine?.(sourceLine);
    };
    renderer.domElement.addEventListener("pointerdown", onPointerDown);
    renderer.domElement.addEventListener("pointerup", onPointerUp);

    const frameAt = (target: THREE.Vector3) => {
      camera.up.set(0, 0, 1);
      if (view === "top") {
        camera.position.set(
          target.x,
          target.y,
          target.z + model.frameRadius * 3,
        );
        camera.up.set(0, 1, 0);
      } else {
        camera.position.set(
          target.x + model.frameRadius * 1.25,
          target.y - model.frameRadius * 1.45,
          target.z + model.frameRadius * 1.2,
        );
      }
      camera.zoom = 1;
      camera.lookAt(target);
      camera.updateProjectionMatrix();
      controls.target.copy(target);
      controls.update();
    };

    const resize = () => {
      const width = Math.max(host.clientWidth, 1);
      const height = Math.max(host.clientHeight, 1);
      const aspect = width / height;
      if (aspect >= 1) {
        camera.left = -model.frameRadius * aspect;
        camera.right = model.frameRadius * aspect;
        camera.top = model.frameRadius;
        camera.bottom = -model.frameRadius;
      } else {
        camera.left = -model.frameRadius;
        camera.right = model.frameRadius;
        camera.top = model.frameRadius / aspect;
        camera.bottom = -model.frameRadius / aspect;
      }
      camera.updateProjectionMatrix();
      renderer.setSize(width, height, false);
    };
    const observer = new ResizeObserver(resize);
    observer.observe(host);
    resize();

    const runtime: PreviewRuntime = {
      model,
      renderer,
      rapidLine: rapidPath?.line,
      rapidMaterial: rapidPath?.material,
      cuttingLine: cuttingPath?.line,
      cuttingMaterial: cuttingPath?.material,
      focusProgram: () => frameAt(new THREE.Vector3()),
      focusTool: () => {
        if (!toolMarker.visible) return;
        frameAt(toolMarker.position);
      },
      selectionLine,
      selectionPoints,
      toolMarker,
      toolMarkerTexture,
      toolProjection,
      toolProjectionLine,
    };
    runtimeRef.current = runtime;

    let frame = 0;
    const animate = () => {
      frame = requestAnimationFrame(animate);
      controls.update();
      renderer.render(scene, camera);
    };
    animate();

    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      renderer.domElement.removeEventListener("pointerdown", onPointerDown);
      renderer.domElement.removeEventListener("pointerup", onPointerUp);
      controls.dispose();
      scene.traverse((object) => {
        if (
          object instanceof THREE.Line ||
          object instanceof THREE.LineSegments ||
          object instanceof THREE.Points
        ) {
          object.geometry.dispose();
          if (Array.isArray(object.material)) {
            object.material.forEach((material) => material.dispose());
          } else {
            object.material.dispose();
          }
        }
      });
      renderer.dispose();
      renderer.forceContextLoss();
      toolMarkerTexture.dispose();
      renderer.domElement.remove();
      if (runtimeRef.current === runtime) runtimeRef.current = undefined;
    };
  }, [onSelectSourceLine, program, view]);

  useEffect(() => {
    const runtime = runtimeRef.current;
    if (!runtime) return;
    const selection = buildToolpathHighlightReadModel(
      program,
      selectedSourceLine,
      runtime.model.center,
    );
    const hasSelection = selection.positions.length > 0;
    const replaceGeometry = (object: THREE.LineSegments | THREE.Points) => {
      object.geometry.dispose();
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute(
        "position",
        new THREE.BufferAttribute(selection.positions, 3),
      );
      object.geometry = geometry;
      object.visible = hasSelection;
    };
    replaceGeometry(runtime.selectionLine);
    replaceGeometry(runtime.selectionPoints);
    if (runtime.rapidMaterial) {
      runtime.rapidMaterial.opacity = hasSelection ? 0.2 : 0.68;
    }
    if (runtime.cuttingMaterial) {
      runtime.cuttingMaterial.opacity = hasSelection ? 0.28 : 1;
    }
    runtime.renderer.domElement.setAttribute(
      "aria-label",
      selectedSourceLine === undefined
        ? "Предпросмотр траектории G-code"
        : `Предпросмотр траектории G-code, выбрана строка ${selectedSourceLine}`,
    );
  }, [program, selectedSourceLine, view]);

  useEffect(() => {
    const runtime = runtimeRef.current;
    if (!runtime) return;
    if (!toolPosition) {
      runtime.toolMarker.visible = false;
      runtime.toolProjection.visible = false;
      runtime.toolProjectionLine.visible = false;
      return;
    }

    const position = buildToolPositionReadModel(
      toolPosition,
      runtime.model,
      program.summary.bounds,
    );
    const visibleMarkerPosition = view === "top"
      ? {
          ...position.scenePosition,
          z: runtime.model.gridZ + runtime.model.gridSize * 0.004,
        }
      : position.scenePosition;
    runtime.toolMarker.position.set(
      visibleMarkerPosition.x,
      visibleMarkerPosition.y,
      visibleMarkerPosition.z,
    );
    runtime.toolProjection.position.set(
      position.gridProjection.x,
      position.gridProjection.y,
      position.gridProjection.z,
    );
    const lineAttribute = runtime.toolProjectionLine.geometry.getAttribute(
      "position",
    ) as THREE.BufferAttribute;
    lineAttribute.setXYZ(
      0,
      position.scenePosition.x,
      position.scenePosition.y,
      position.scenePosition.z,
    );
    lineAttribute.setXYZ(
      1,
      position.gridProjection.x,
      position.gridProjection.y,
      position.gridProjection.z,
    );
    lineAttribute.needsUpdate = true;
    runtime.toolProjectionLine.computeLineDistances();
    runtime.toolMarker.visible = true;
    runtime.toolProjection.visible = view === "iso";
    runtime.toolProjectionLine.visible =
      view === "iso" &&
      Math.abs(position.scenePosition.z - position.gridProjection.z) > 0.001;
    runtime.renderer.domElement.setAttribute(
      "aria-label",
      `Предпросмотр траектории G-code${selectedSourceLine === undefined ? "" : `, выбрана строка ${selectedSourceLine}`}, фреза ${toolCoordinateSystem} X ${toolPosition.x.toFixed(3)}, Y ${toolPosition.y.toFixed(3)}, Z ${toolPosition.z.toFixed(3)}`,
    );
  }, [program, selectedSourceLine, toolCoordinateSystem, toolPosition, view]);

  return (
    <div className="toolpath-preview">
      <div className="toolpath-canvas" ref={hostRef} />
      {toolPosition && (
        <aside
          className={`tool-position-hud${toolOverProgram ? "" : " is-outside"}`}
          aria-label="Текущее положение фрезы"
        >
          <Crosshair aria-hidden="true" size={18} />
          <div className="tool-position-title">
            <strong>Фреза · {toolCoordinateSystem}</strong>
            <small>{toolOverProgram ? "Над заданием" : "Вне границ задания"}</small>
          </div>
          <code>
            <span>X {formatCoordinate(toolPosition.x)}</span>
            <span>Y {formatCoordinate(toolPosition.y)}</span>
            <span>Z {formatCoordinate(toolPosition.z)}</span>
          </code>
          <div className="tool-position-actions">
            <button
              aria-label="Показать всю программу"
              onClick={() => runtimeRef.current?.focusProgram()}
              title="Показать всю программу"
              type="button"
            >
              <ScanSearch aria-hidden="true" size={15} />
            </button>
            <button
              aria-label="Центрировать на фрезе"
              onClick={() => runtimeRef.current?.focusTool()}
              title="Центрировать на фрезе"
              type="button"
            >
              <Crosshair aria-hidden="true" size={15} />
            </button>
          </div>
        </aside>
      )}
    </div>
  );
}
