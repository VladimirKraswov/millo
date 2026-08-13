import { describe, expect, it } from "vitest";

import type { SurfaceSession } from "../../shared/heightmap";
import { surfaceMapExecutionView } from "./surfaceMapExecutionModel";

const session: SurfaceSession = {
  schemaVersion: 1,
  revision: 3,
  nextMapId: 5,
  applicationEnabled: false,
  requiresSetupConfirmation: true,
  coordinateBindingStale: false,
  active: {
    mapId: 4,
    machineProfileId: "machine-0001",
    createdAtUnixMs: 1,
    map: {
      schemaVersion: 1,
      plan: {
        schemaVersion: 1,
        request: {
          originXMm: 0,
          originYMm: 0,
          widthMm: 100,
          heightMm: 50,
          columns: 2,
          rows: 2,
          clearanceZMm: 2,
          maxProbeDepthMm: 3,
          probeFeedMmPerMin: 25,
          travelFeedMmPerMin: 300,
          retractFeedMmPerMin: 100,
          contactMode: "directSurface",
          contactOffsetMm: 0,
        },
        spacing: { xMm: 100, yMm: 50 },
        points: [],
      },
      samples: [],
    },
  },
};

describe("surfaceMapExecutionView", () => {
  it("shows a saved map only for its machine and validates program coverage", () => {
    const covered = surfaceMapExecutionView(session, "machine-0001", {
      min: { x: 2, y: 3, z: -0.2 },
      max: { x: 80, y: 40, z: 2 },
      size: { x: 78, y: 37, z: 2.2 },
    });
    const outside = surfaceMapExecutionView(session, "machine-0001", {
      min: { x: -1, y: 3, z: -0.2 },
      max: { x: 80, y: 40, z: 2 },
      size: { x: 81, y: 37, z: 2.2 },
    });

    expect(covered?.coversProgram).toBe(true);
    expect(covered?.usable).toBe(true);
    expect(covered?.detail).toContain("Карта #4");
    expect(outside?.coversProgram).toBe(false);
    expect(surfaceMapExecutionView(session, "another-machine", undefined)).toBeUndefined();
  });

  it("keeps a map visible but unusable after work-zero mutation", () => {
    const stale = surfaceMapExecutionView(
      { ...session, coordinateBindingStale: true },
      "machine-0001",
      undefined,
    );
    expect(stale?.usable).toBe(false);
    expect(stale?.detail).toContain("снимите новую карту");
  });
});
