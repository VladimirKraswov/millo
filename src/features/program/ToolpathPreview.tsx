import { useEffect, useRef } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";

import type { GcodeProgram } from "../../shared/program";
import { buildToolpathReadModel } from "./toolpathReadModel";

export type PreviewView = "top" | "iso";

interface ToolpathPreviewProps {
  readonly program: GcodeProgram;
  readonly view: PreviewView;
}

export function ToolpathPreview({ program, view }: ToolpathPreviewProps) {
  const hostRef = useRef<HTMLDivElement>(null);

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

    const addPath = (positions: Float32Array, color: number, opacity: number) => {
      if (positions.length === 0) return;
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
      const material = new THREE.LineBasicMaterial({
        color,
        opacity,
        transparent: opacity < 1,
      });
      scene.add(new THREE.LineSegments(geometry, material));
    };
    addPath(model.rapidPositions, 0xffb454, 0.68);
    addPath(model.cuttingPositions, 0x77d6b3, 1);

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
        if (object instanceof THREE.Line || object instanceof THREE.LineSegments) {
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
    };
  }, [program, view]);

  return <div className="toolpath-preview" ref={hostRef} />;
}
