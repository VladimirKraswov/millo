import { describe, expect, it } from "vitest";
import * as THREE from "three";

import type { ToolRenderProfile } from "./toolGeometryModel";
import { createToolMesh, disposeToolMesh } from "./toolMesh";

const profile = (overrides: Partial<ToolRenderProfile>): ToolRenderProfile => ({
  kind: "engraving",
  diameterMm: 3.175,
  tipDiameterMm: 0.1,
  shankDiameterMm: 3.175,
  cuttingLengthMm: 3,
  shankLengthMm: 4,
  fluteCount: 1,
  includedAngleDegrees: 20,
  angularSpeedRadPerSecond: 18,
  ...overrides,
});

describe("createToolMesh", () => {
  it("keeps a short engraving cone faithful to its included angle", () => {
    const assembly = createToolMesh(profile({}));
    const cutter = assembly.rotor.children.find(
      (child): child is THREE.Mesh => child instanceof THREE.Mesh,
    );
    const geometry = cutter?.geometry as THREE.CylinderGeometry | undefined;

    expect(geometry?.parameters.radiusBottom).toBeCloseTo(0.05, 4);
    expect(geometry?.parameters.radiusTop).toBeCloseTo(
      0.05 + 3 * Math.tan(THREE.MathUtils.degToRad(10)),
      4,
    );
    expect(geometry?.parameters.radiusTop).toBeLessThan(3.175 / 2);

    disposeToolMesh(assembly.root);
  });

  it("models the neck between a surfacing cutter and its shank", () => {
    const assembly = createToolMesh(profile({
      kind: "surfacing",
      diameterMm: 25.4,
      tipDiameterMm: 25.4,
      shankDiameterMm: 6.35,
      cuttingLengthMm: 10,
      fluteCount: 4,
    }));
    const axialMeshes = assembly.rotor.children.filter(
      (child): child is THREE.Mesh => child instanceof THREE.Mesh,
    );

    expect(axialMeshes).toHaveLength(4);
    expect((axialMeshes[1]?.geometry as THREE.CylinderGeometry).parameters.radiusTop)
      .toBeCloseTo(6.35 / 2);

    disposeToolMesh(assembly.root);
  });
});
