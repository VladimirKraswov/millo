import { useEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";

import type { Heightmap, HeightmapPlanRequest } from "../../shared/heightmap";
import type { GcodeProgram } from "../../shared/program";
import { buildHeightmapPlan } from "./heightmapModel";

interface HeightmapSceneProps {
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

const colorFor = (ratio: number): THREE.Color => {
  const value = Math.max(0, Math.min(1, ratio));
  return value < 0.5
    ? surfaceColor.copy(cold).lerp(middle, value * 2)
    : surfaceColor.copy(middle).lerp(hot, (value - 0.5) * 2);
};

export function HeightmapScene({
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
    element.replaceChildren(renderer.domElement);

    const centerX = request.originXmm + request.widthMm / 2;
    const centerY = request.originYmm + request.heightMm / 2;
    const span = Math.max(request.widthMm, request.heightMm, 10);
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
        new THREE.Vector2(request.originXmm, request.originYmm),
        new THREE.Vector2(request.originXmm + request.widthMm, request.originYmm),
        new THREE.Vector2(request.originXmm + request.widthMm, request.originYmm + request.heightMm),
        new THREE.Vector2(request.originXmm, request.originYmm + request.heightMm),
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
      const maxX = request.originXmm + request.widthMm;
      const maxY = request.originYmm + request.heightMm;
      for (const segment of program.toolpath) {
        for (let index = 1; index < segment.points.length; index += 1) {
          const pair = [segment.points[index - 1], segment.points[index]];
          const target = pair.every((point) =>
            point.x >= request.originXmm && point.x <= maxX &&
            point.y >= request.originYmm && point.y <= maxY,
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
    const zValues = samples.map((sample) => sample.zMm);
    const minimum = zValues.length > 0 ? Math.min(...zValues) : 0;
    const maximum = zValues.length > 0 ? Math.max(...zValues) : 0;
    const zRange = Math.max(maximum - minimum, 0.001);
    const exaggeration = Math.min(50, Math.max(1, span * 0.08 / zRange));

    if (showInterpolation && map && map.samples.every(Boolean)) {
      const columns = Math.max(2, Math.min(150, interpolationColumns));
      const rows = Math.max(2, Math.min(150, interpolationRows));
      const positions: number[] = [];
      const colors: number[] = [];
      const byCell = new Map(map.samples.flatMap((sample) => sample ? [[`${sample.point.row}:${sample.point.column}`, sample]] : []));
      const sampleZ = (row: number, column: number): number => {
        const sourceX = column / (columns - 1) * (map.plan.request.columns - 1);
        const sourceY = row / (rows - 1) * (map.plan.request.rows - 1);
        const left = Math.floor(sourceX);
        const right = Math.min(left + 1, map.plan.request.columns - 1);
        const bottom = Math.floor(sourceY);
        const top = Math.min(bottom + 1, map.plan.request.rows - 1);
        const xMix = sourceX - left;
        const yMix = sourceY - bottom;
        const z00 = byCell.get(`${bottom}:${left}`)?.zMm ?? 0;
        const z10 = byCell.get(`${bottom}:${right}`)?.zMm ?? z00;
        const z01 = byCell.get(`${top}:${left}`)?.zMm ?? z00;
        const z11 = byCell.get(`${top}:${right}`)?.zMm ?? z01;
        return (z00 + (z10 - z00) * xMix) * (1 - yMix) +
          (z01 + (z11 - z01) * xMix) * yMix;
      };
      for (let row = 0; row < rows - 1; row += 1) {
        for (let column = 0; column < columns - 1; column += 1) {
          const corners = [[row, column], [row, column + 1], [row + 1, column + 1], [row, column], [row + 1, column + 1], [row + 1, column]];
          for (const [cornerRow, cornerColumn] of corners) {
            const z = sampleZ(cornerRow, cornerColumn);
            const x = request.originXmm + cornerColumn / (columns - 1) * request.widthMm;
            const y = request.originYmm + cornerRow / (rows - 1) * request.heightMm;
            positions.push(x, y, (z - minimum) * exaggeration);
            const color = colorFor((z - minimum) / zRange);
            colors.push(color.r, color.g, color.b);
          }
        }
      }
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

    if (showProbeGrid) {
      const positions: number[] = [];
      const colors: number[] = [];
      for (const point of draftPlan.points) {
        const sample = map?.samples[point.sequence];
        positions.push(point.xMm, point.yMm, sample ? (sample.zMm - minimum) * exaggeration + 0.08 : 0.08);
        const color = sample ? colorFor((sample.zMm - minimum) / zRange) : new THREE.Color(0x7f9099);
        colors.push(color.r, color.g, color.b);
      }
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
      geometry.setAttribute("color", new THREE.Float32BufferAttribute(colors, 3));
      scene.add(new THREE.Points(geometry, new THREE.PointsMaterial({
        size: 6,
        sizeAttenuation: false,
        vertexColors: true,
      })));
    }

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.enableRotate = view === "iso";
    controls.target.set(centerX, centerY, 0);

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
    };
    const observer = new ResizeObserver(resize);
    observer.observe(element);
    resize();
    let frame = 0;
    const render = () => {
      controls.update();
      renderer.render(scene, camera);
      frame = requestAnimationFrame(render);
    };
    render();
    return () => {
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
  }, [draftPlan, interpolationColumns, interpolationRows, map, program, request, showInterpolation, showInterpolationGrid, showJob, showPerimeter, showProbeGrid, view]);

  return <div className="heightmap-scene" ref={host} />;
}
