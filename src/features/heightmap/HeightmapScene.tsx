import { useEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { CSS2DObject, CSS2DRenderer } from "three/addons/renderers/CSS2DRenderer.js";

import type { Heightmap, HeightmapPlanRequest } from "../../shared/heightmap";
import type { GcodeProgram } from "../../shared/program";
import { buildHeightmapPlan } from "./heightmapModel";
import {
  heightmapCameraScope,
  heightmapSampleLabel,
  heightmapSceneBounds,
  heightmapVisualScale,
  shouldLabelHeightmapSample,
} from "./heightmapSceneModel";

interface HeightmapSceneProps {
  readonly currentSequence?: number;
  readonly map?: Heightmap;
  readonly program?: GcodeProgram;
  readonly request: HeightmapPlanRequest;
  readonly showInterpolation: boolean;
  readonly showInterpolationGrid: boolean;
  readonly showJob: boolean;
  readonly showPerimeter: boolean;
  readonly showProbeGrid: boolean;
  readonly view: "top" | "iso";
  readonly interpolationColumns: number;
  readonly interpolationRows: number;
}

const surfaceColor = new THREE.Color();
const cold = new THREE.Color(0x3ba8d8);
const middle = new THREE.Color(0x72d6b1);
const hot = new THREE.Color(0xffb55c);

interface SavedCameraState {
  readonly position: THREE.Vector3;
  readonly scope: string;
  readonly target: THREE.Vector3;
  readonly up: THREE.Vector3;
  readonly zoom: number;
}

const colorFor = (ratio: number): THREE.Color => {
  const value = Math.max(0, Math.min(1, ratio));
  return value < 0.5
    ? surfaceColor.copy(cold).lerp(middle, value * 2)
    : surfaceColor.copy(middle).lerp(hot, (value - 0.5) * 2);
};

export function HeightmapScene({
  currentSequence,
  map,
  program,
  request,
  showInterpolation,
  showInterpolationGrid,
  showJob,
  showPerimeter,
  showProbeGrid,
  view,
  interpolationColumns,
  interpolationRows,
}: HeightmapSceneProps) {
  const host = useRef<HTMLDivElement>(null);
  const savedCamera = useRef<SavedCameraState | undefined>(undefined);
  const draftPlan = useMemo(() => buildHeightmapPlan(request), [request]);

  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x0b1013);
    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    renderer.domElement.setAttribute("aria-label", "Карта высот и периметр измерения");
    const labels = new CSS2DRenderer();
    labels.domElement.className = "heightmap-scene-labels";
    labels.domElement.setAttribute("aria-hidden", "true");
    element.replaceChildren(renderer.domElement, labels.domElement);

    const bounds = heightmapSceneBounds(request, map?.plan.request);
    const { centerX, centerY, span } = bounds;
    const cameraScope = heightmapCameraScope(view, bounds);
    const camera = new THREE.OrthographicCamera(-span, span, span, -span, 0.01, span * 20);
    camera.up.set(0, 0, 1);
    camera.position.set(
      centerX + (view === "top" ? 0 : span),
      centerY + (view === "top" ? 0 : -span),
      span * (view === "top" ? 3 : 1.25),
    );
    if (view === "top") camera.up.set(0, 1, 0);
    camera.lookAt(centerX, centerY, 0);

    const grid = new THREE.GridHelper(span * 2.4, 24, 0x40515b, 0x202b31);
    grid.position.set(centerX, centerY, -0.03);
    grid.rotation.x = Math.PI / 2;
    scene.add(grid);

    if (showPerimeter) {
      const area = new THREE.Mesh(
        new THREE.PlaneGeometry(request.widthMm, request.heightMm),
        new THREE.MeshBasicMaterial({ color: 0x4cc49a, opacity: 0.08, transparent: true }),
      );
      area.position.set(centerX, centerY, -0.015);
      scene.add(area);
      const shape = new THREE.Shape([
        new THREE.Vector2(request.originXMm, request.originYMm),
        new THREE.Vector2(request.originXMm + request.widthMm, request.originYMm),
        new THREE.Vector2(request.originXMm + request.widthMm, request.originYMm + request.heightMm),
        new THREE.Vector2(request.originXMm, request.originYMm + request.heightMm),
      ]);
      const border = new THREE.LineLoop(
        new THREE.BufferGeometry().setFromPoints(shape.getPoints()),
        new THREE.LineBasicMaterial({ color: 0x67d6ad }),
      );
      border.position.z = 0.01;
      scene.add(border);
    }

    if (showJob && program) {
      const inside: number[] = [];
      const outside: number[] = [];
      const maxX = request.originXMm + request.widthMm;
      const maxY = request.originYMm + request.heightMm;
      for (const segment of program.toolpath) {
        for (let index = 1; index < segment.points.length; index += 1) {
          const pair = [segment.points[index - 1], segment.points[index]];
          const target = pair.every((point) =>
            point.x >= request.originXMm && point.x <= maxX &&
            point.y >= request.originYMm && point.y <= maxY,
          ) ? inside : outside;
          for (const point of pair) target.push(point.x, point.y, 0.04);
        }
      }
      for (const [positions, color] of [[inside, 0xa8b7be], [outside, 0xff6b72]] as const) {
        if (positions.length === 0) continue;
        const geometry = new THREE.BufferGeometry();
        geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
        scene.add(new THREE.LineSegments(geometry, new THREE.LineBasicMaterial({ color })));
      }
    }

    const samples = map?.samples.flatMap((sample) => sample ? [sample] : []) ?? [];
    const scale = heightmapVisualScale(samples.map((sample) => sample.zMm), span);
    const { exaggeration, minimum, range: zRange } = scale;

    if (showInterpolation && map && samples.length >= 4) {
      const sourceRequest = map.plan.request;
      const columns = Math.max(2, Math.min(150, interpolationColumns));
      const rows = Math.max(2, Math.min(150, interpolationRows));
      const positions: number[] = [];
      const colors: number[] = [];
      const byCell = new Map(map.samples.flatMap((sample) => sample ? [[`${sample.point.row}:${sample.point.column}`, sample]] : []));
      const sampleZ = (row: number, column: number): number | undefined => {
        const sourceX = column / (columns - 1) * (map.plan.request.columns - 1);
        const sourceY = row / (rows - 1) * (map.plan.request.rows - 1);
        const left = Math.floor(sourceX);
        const right = Math.min(left + 1, map.plan.request.columns - 1);
        const bottom = Math.floor(sourceY);
        const top = Math.min(bottom + 1, map.plan.request.rows - 1);
        const xMix = sourceX - left;
        const yMix = sourceY - bottom;
        const z00 = byCell.get(`${bottom}:${left}`)?.zMm;
        const z10 = byCell.get(`${bottom}:${right}`)?.zMm;
        const z01 = byCell.get(`${top}:${left}`)?.zMm;
        const z11 = byCell.get(`${top}:${right}`)?.zMm;
        if (z00 === undefined || z10 === undefined || z01 === undefined || z11 === undefined) {
          return undefined;
        }
        return (z00 + (z10 - z00) * xMix) * (1 - yMix) +
          (z01 + (z11 - z01) * xMix) * yMix;
      };
      for (let row = 0; row < rows - 1; row += 1) {
        for (let column = 0; column < columns - 1; column += 1) {
          const corners = [[row, column], [row, column + 1], [row + 1, column + 1], [row, column], [row + 1, column + 1], [row + 1, column]] as const;
          const values = corners.map(([cornerRow, cornerColumn]) => sampleZ(cornerRow, cornerColumn));
          if (values.some((value) => value === undefined)) continue;
          for (let corner = 0; corner < corners.length; corner += 1) {
            const [cornerRow, cornerColumn] = corners[corner];
            const z = values[corner]!;
            const x = sourceRequest.originXMm + cornerColumn / (columns - 1) * sourceRequest.widthMm;
            const y = sourceRequest.originYMm + cornerRow / (rows - 1) * sourceRequest.heightMm;
            positions.push(x, y, (z - minimum) * exaggeration);
            const color = colorFor((z - minimum) / zRange);
            colors.push(color.r, color.g, color.b);
          }
        }
      }
      if (positions.length > 0) {
        const geometry = new THREE.BufferGeometry();
        geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
        geometry.setAttribute("color", new THREE.Float32BufferAttribute(colors, 3));
        geometry.computeVertexNormals();
        const surface = new THREE.Mesh(geometry, new THREE.MeshBasicMaterial({
          opacity: 0.82,
          side: THREE.DoubleSide,
          transparent: true,
          vertexColors: true,
        }));
        scene.add(surface);
        if (showInterpolationGrid) {
          scene.add(new THREE.LineSegments(
            new THREE.WireframeGeometry(geometry),
            new THREE.LineBasicMaterial({ color: 0xdde8ea, opacity: 0.16, transparent: true }),
          ));
        }
      }
    }

    if (showProbeGrid) {
      const samePlan = Boolean(map && [
        "originXMm",
        "originYMm",
        "widthMm",
        "heightMm",
        "columns",
        "rows",
      ].every((key) => map.plan.request[key as keyof HeightmapPlanRequest] === request[key as keyof HeightmapPlanRequest]));
      const addPoints = (positions: number[], colors: number[], size: number) => {
        if (positions.length === 0) return;
        const geometry = new THREE.BufferGeometry();
        geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
        geometry.setAttribute("color", new THREE.Float32BufferAttribute(colors, 3));
        scene.add(new THREE.Points(geometry, new THREE.PointsMaterial({
          size,
          sizeAttenuation: false,
          vertexColors: true,
        })));
      };
      const plannedPositions: number[] = [];
      const plannedColors: number[] = [];
      for (const point of draftPlan.points) {
        const existingSample = samePlan ? map?.samples[point.sequence] : undefined;
        if (!existingSample) {
          plannedPositions.push(point.xMm, point.yMm, 0.08);
          plannedColors.push(0.5, 0.56, 0.6);
        }
      }
      addPoints(plannedPositions, plannedColors, 5);
      const measuredPositions: number[] = [];
      const measuredColors: number[] = [];
      for (const sample of samples) {
        const visualZ = (sample.zMm - minimum) * exaggeration + 0.08;
        measuredPositions.push(sample.point.xMm, sample.point.yMm, visualZ);
        const color = colorFor((sample.zMm - minimum) / zRange);
        measuredColors.push(color.r, color.g, color.b);

        const ring = new THREE.Mesh(
          new THREE.RingGeometry(Math.max(0.18, span * 0.003), Math.max(0.32, span * 0.005), 24),
          new THREE.MeshBasicMaterial({
            color: sample.point.sequence === currentSequence ? 0xffc45e : 0xeaf7f2,
            side: THREE.DoubleSide,
          }),
        );
        ring.position.set(sample.point.xMm, sample.point.yMm, visualZ + 0.03);
        scene.add(ring);

        if (shouldLabelHeightmapSample(sample.point.sequence, map?.plan.points.length ?? samples.length, currentSequence)) {
          const node = document.createElement("span");
          node.className = `heightmap-point-label${sample.point.sequence === currentSequence ? " is-current" : ""}`;
          node.textContent = heightmapSampleLabel(sample.zMm);
          const label = new CSS2DObject(node);
          label.position.set(sample.point.xMm, sample.point.yMm, visualZ + 0.12);
          scene.add(label);
        }
      }
      addPoints(measuredPositions, measuredColors, 7);

      const nextPoint = draftPlan.points.find((point) => point.sequence === currentSequence);
      if (nextPoint && !map?.samples[nextPoint.sequence]) {
        const marker = new THREE.Mesh(
          new THREE.RingGeometry(Math.max(0.25, span * 0.004), Math.max(0.42, span * 0.007), 24),
          new THREE.MeshBasicMaterial({ color: 0xffc45e, side: THREE.DoubleSide }),
        );
        marker.position.set(nextPoint.xMm, nextPoint.yMm, 0.11);
        scene.add(marker);
      }
    }

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.enableRotate = view === "iso";
    controls.target.set(centerX, centerY, 0);
    if (savedCamera.current?.scope === cameraScope) {
      camera.position.copy(savedCamera.current.position);
      camera.up.copy(savedCamera.current.up);
      camera.zoom = savedCamera.current.zoom;
      controls.target.copy(savedCamera.current.target);
      controls.update();
    }

    const resize = () => {
      const width = Math.max(1, element.clientWidth);
      const height = Math.max(1, element.clientHeight);
      const aspect = width / height;
      const half = span * 0.72;
      camera.left = -half * aspect;
      camera.right = half * aspect;
      camera.top = half;
      camera.bottom = -half;
      camera.updateProjectionMatrix();
      renderer.setSize(width, height, false);
      labels.setSize(width, height);
    };
    const observer = new ResizeObserver(resize);
    observer.observe(element);
    resize();
    let frame = 0;
    const render = () => {
      controls.update();
      renderer.render(scene, camera);
      labels.render(scene, camera);
      frame = requestAnimationFrame(render);
    };
    render();
    return () => {
      savedCamera.current = {
        position: camera.position.clone(),
        scope: cameraScope,
        target: controls.target.clone(),
        up: camera.up.clone(),
        zoom: camera.zoom,
      };
      cancelAnimationFrame(frame);
      observer.disconnect();
      controls.dispose();
      scene.traverse((object) => {
        if ("geometry" in object && object.geometry instanceof THREE.BufferGeometry) object.geometry.dispose();
        if ("material" in object) {
          const materials = Array.isArray(object.material) ? object.material : [object.material];
          materials.forEach((material) => material instanceof THREE.Material && material.dispose());
        }
      });
      renderer.dispose();
    };
  }, [currentSequence, draftPlan, interpolationColumns, interpolationRows, map, program, request, showInterpolation, showInterpolationGrid, showJob, showPerimeter, showProbeGrid, view]);

  return <div className="heightmap-scene" ref={host} />;
}
