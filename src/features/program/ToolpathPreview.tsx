import { useEffect, useRef } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";

import type { GcodeProgram } from "../../shared/program";
import {
  buildToolpathHighlightReadModel,
  buildToolpathReadModel,
  type ToolpathReadModel,
} from "./toolpathReadModel";

export type PreviewView = "top" | "iso";

interface ToolpathPreviewProps {
  readonly program: GcodeProgram;
  readonly selectedSourceLine?: number;
  readonly view: PreviewView;
}

interface PreviewRuntime {
  readonly model: ToolpathReadModel;
  readonly renderer: THREE.WebGLRenderer;
  readonly rapidMaterial?: THREE.LineBasicMaterial;
  readonly cuttingMaterial?: THREE.LineBasicMaterial;
  readonly selectionLine: THREE.LineSegments;
  readonly selectionPoints: THREE.Points;
}

export function ToolpathPreview({
  program,
  selectedSourceLine,
  view,
}: ToolpathPreviewProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const runtimeRef = useRef<PreviewRuntime | undefined>(undefined);

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
    renderer.domElement.setAttribute("aria-label", "G-code toolpath preview");
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
    ): THREE.LineBasicMaterial | undefined => {
      if (positions.length === 0) return undefined;
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
      const material = new THREE.LineBasicMaterial({
        color,
        opacity,
        transparent: true,
      });
      scene.add(new THREE.LineSegments(geometry, material));
      return material;
    };
    const rapidMaterial = addPath(model.rapidPositions, 0xffb454, 0.68);
    const cuttingMaterial = addPath(model.cuttingPositions, 0x77d6b3, 1);

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

    const axes = new THREE.AxesHelper(Math.min(model.gridSize * 0.12, 8));
    scene.add(axes);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.enableRotate = view === "iso";
    controls.screenSpacePanning = true;
    controls.minZoom = 0.3;
    controls.maxZoom = 30;

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
      rapidMaterial,
      cuttingMaterial,
      selectionLine,
      selectionPoints,
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
      renderer.domElement.remove();
      if (runtimeRef.current === runtime) runtimeRef.current = undefined;
    };
  }, [program, view]);

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
        ? "G-code toolpath preview"
        : `G-code toolpath preview, line ${selectedSourceLine} selected`,
    );
  }, [program, selectedSourceLine, view]);

  return <div className="toolpath-preview" ref={hostRef} />;
}
