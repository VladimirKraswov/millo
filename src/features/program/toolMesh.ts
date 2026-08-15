import * as THREE from "three";

import type { ToolRenderProfile } from "./toolGeometryModel";

export interface ToolMeshAssembly {
  readonly root: THREE.Group;
  readonly rotor: THREE.Group;
  readonly sweep: THREE.Mesh;
}

const axialMesh = (
  topRadius: number,
  bottomRadius: number,
  height: number,
  baseZ: number,
  material: THREE.Material,
  segments = 32,
): THREE.Mesh => {
  const geometry = new THREE.CylinderGeometry(
    topRadius,
    bottomRadius,
    height,
    segments,
    1,
    false,
  );
  geometry.rotateX(Math.PI / 2);
  const mesh = new THREE.Mesh(geometry, material);
  mesh.position.z = baseZ + height / 2;
  return mesh;
};

const conicalLength = (profile: ToolRenderProfile): number => {
  const radiusDelta = (profile.diameterMm - profile.tipDiameterMm) / 2;
  const halfAngle = THREE.MathUtils.degToRad(
    Math.max(1, profile.includedAngleDegrees ?? (profile.kind === "engraving" ? 30 : 60)) / 2,
  );
  const theoretical = radiusDelta / Math.tan(halfAngle);
  return Math.min(profile.cuttingLengthMm, Math.max(0.2, theoretical));
};

const conicalRadiusAtZ = (
  profile: ToolRenderProfile,
  z: number,
): number => {
  const radius = profile.diameterMm / 2;
  const tipRadius = profile.tipDiameterMm / 2;
  const halfAngle = THREE.MathUtils.degToRad(
    Math.max(1, profile.includedAngleDegrees ?? (profile.kind === "engraving" ? 30 : 60)) / 2,
  );
  return Math.min(radius, tipRadius + Math.max(0, z) * Math.tan(halfAngle));
};

const radiusAtZ = (profile: ToolRenderProfile, z: number): number => {
  const radius = profile.diameterMm / 2;
  if (profile.kind === "vBit" || profile.kind === "engraving") {
    return conicalRadiusAtZ(profile, z);
  }
  if (profile.kind === "drill") {
    const pointLength = Math.min(profile.cuttingLengthMm, Math.max(0.4, radius * 1.8));
    return z >= pointLength ? radius : radius * Math.max(0, z / pointLength);
  }
  if (profile.kind === "ballNose") {
    if (z >= radius * 2) return radius;
    const centered = z - radius;
    return Math.sqrt(Math.max(0, radius * radius - centered * centered));
  }
  return radius;
};

const addFlutes = (
  rotor: THREE.Group,
  profile: ToolRenderProfile,
): void => {
  for (let flute = 0; flute < profile.fluteCount; flute += 1) {
    const points: THREE.Vector3[] = [];
    const baseAngle = flute * Math.PI * 2 / profile.fluteCount;
    for (let index = 0; index <= 24; index += 1) {
      const progress = index / 24;
      const z = Math.max(0.025, profile.cuttingLengthMm * progress);
      const radius = Math.max(0.025, radiusAtZ(profile, z)) + 0.018;
      const angle = baseAngle + progress * Math.PI * 0.72;
      points.push(new THREE.Vector3(
        Math.cos(angle) * radius,
        Math.sin(angle) * radius,
        z,
      ));
    }
    const geometry = new THREE.BufferGeometry().setFromPoints(points);
    const material = new THREE.LineBasicMaterial({
      color: flute % 2 === 0 ? 0x213039 : 0xe4edf0,
      opacity: flute % 2 === 0 ? 0.9 : 0.42,
      transparent: true,
    });
    rotor.add(new THREE.Line(geometry, material));
  }
};

export function createToolMesh(profile: ToolRenderProfile): ToolMeshAssembly {
  const root = new THREE.Group();
  root.name = "millo-tool";
  const rotor = new THREE.Group();
  rotor.name = "millo-tool-rotor";
  root.add(rotor);

  const cuttingMaterial = new THREE.MeshStandardMaterial({
    color: 0xe8eff0,
    emissive: 0x4b5b61,
    emissiveIntensity: 0.52,
    metalness: 0.82,
    roughness: 0.24,
  });
  const shankMaterial = new THREE.MeshStandardMaterial({
    color: 0xcbd5d8,
    emissive: 0x3e4d52,
    emissiveIntensity: 0.48,
    metalness: 0.9,
    roughness: 0.2,
  });
  const collarMaterial = new THREE.MeshStandardMaterial({
    color: 0x71838b,
    emissive: 0x27343a,
    emissiveIntensity: 0.45,
    metalness: 0.62,
    roughness: 0.38,
  });
  const cuttingRadius = profile.diameterMm / 2;
  const tipRadius = profile.tipDiameterMm / 2;

  if (profile.kind === "vBit" || profile.kind === "engraving") {
    const coneLength = conicalLength(profile);
    const coneRadius = conicalRadiusAtZ(profile, coneLength);
    rotor.add(axialMesh(coneRadius, tipRadius, coneLength, 0, cuttingMaterial));
    if (coneRadius >= cuttingRadius - 0.0001 && coneLength < profile.cuttingLengthMm) {
      rotor.add(axialMesh(
        cuttingRadius,
        cuttingRadius,
        profile.cuttingLengthMm - coneLength,
        coneLength,
        cuttingMaterial,
      ));
    }
  } else if (profile.kind === "drill") {
    const pointLength = Math.min(
      profile.cuttingLengthMm,
      Math.max(0.4, cuttingRadius * 1.8),
    );
    rotor.add(axialMesh(cuttingRadius, 0.015, pointLength, 0, cuttingMaterial));
    if (pointLength < profile.cuttingLengthMm) {
      rotor.add(axialMesh(
        cuttingRadius,
        cuttingRadius,
        profile.cuttingLengthMm - pointLength,
        pointLength,
        cuttingMaterial,
      ));
    }
  } else if (profile.kind === "ballNose") {
    const sphere = new THREE.Mesh(
      new THREE.SphereGeometry(cuttingRadius, 32, 16),
      cuttingMaterial,
    );
    sphere.position.z = cuttingRadius;
    rotor.add(sphere);
    const bodyStart = cuttingRadius * 2;
    if (bodyStart < profile.cuttingLengthMm) {
      rotor.add(axialMesh(
        cuttingRadius,
        cuttingRadius,
        profile.cuttingLengthMm - bodyStart,
        bodyStart,
        cuttingMaterial,
      ));
    }
  } else {
    const cuttingLength = profile.kind === "surfacing"
      ? Math.min(profile.cuttingLengthMm, Math.max(1, profile.diameterMm * 0.34))
      : profile.cuttingLengthMm;
    rotor.add(axialMesh(
      cuttingRadius,
      cuttingRadius,
      cuttingLength,
      0,
      cuttingMaterial,
    ));
    if (cuttingLength < profile.cuttingLengthMm) {
      rotor.add(axialMesh(
        profile.shankDiameterMm / 2,
        profile.shankDiameterMm / 2,
        profile.cuttingLengthMm - cuttingLength,
        cuttingLength,
        shankMaterial,
      ));
    }
  }

  addFlutes(rotor, profile);
  rotor.add(axialMesh(
    profile.shankDiameterMm / 2,
    profile.shankDiameterMm / 2,
    profile.shankLengthMm,
    profile.cuttingLengthMm,
    shankMaterial,
  ));
  rotor.add(axialMesh(
    profile.shankDiameterMm * 0.62,
    profile.shankDiameterMm * 0.55,
    Math.max(1.4, profile.shankDiameterMm * 0.6),
    profile.cuttingLengthMm + profile.shankLengthMm,
    collarMaterial,
  ));

  for (const child of [...rotor.children]) {
    if (!(child instanceof THREE.Mesh)) continue;
    const outline = new THREE.LineSegments(
      new THREE.EdgesGeometry(child.geometry, 28),
      new THREE.LineBasicMaterial({
        color: 0x8be0c1,
        opacity: 0.82,
        transparent: true,
      }),
    );
    outline.renderOrder = 2;
    child.add(outline);
  }

  const sweep = new THREE.Mesh(
    new THREE.CircleGeometry(cuttingRadius, 40),
    new THREE.MeshBasicMaterial({
      color: 0x77d6b3,
      depthWrite: false,
      opacity: 0.16,
      side: THREE.DoubleSide,
      transparent: true,
    }),
  );
  sweep.name = "millo-tool-envelope";
  sweep.position.z = 0.025;
  sweep.visible = false;
  root.add(sweep);

  return { root, rotor, sweep };
}

export function disposeToolMesh(root: THREE.Object3D): void {
  root.traverse((object) => {
    const renderable = object as THREE.Object3D & {
      geometry?: THREE.BufferGeometry;
      material?: THREE.Material | THREE.Material[];
    };
    renderable.geometry?.dispose();
    if (Array.isArray(renderable.material)) {
      renderable.material.forEach((material) => material.dispose());
    } else {
      renderable.material?.dispose();
    }
  });
}
