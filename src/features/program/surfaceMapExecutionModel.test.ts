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

  it("exposes the measured Z range for the final run warning", () => {
    const measured: SurfaceSession = {
      ...session,
      active: session.active && {
        ...session.active,
        map: {
          ...session.active.map,
          samples: [
            { point: { sequence: 0, row: 0, column: 0, xMm: 0, yMm: 0 }, zMm: 0.021, triggered: true },
            { point: { sequence: 1, row: 0, column: 1, xMm: 100, yMm: 0 }, zMm: 0.388, triggered: true },
            null,
          ],
        },
      },
    };

    const view = surfaceMapExecutionView(measured, "machine-0001", undefined);

    expect(view?.zRangeMm).toBeCloseTo(0.367);
    expect(view?.detail).toContain("перепад 0.367 mm");
  });

  it("flags a sharp local cliff that is unlike the rest of the map", () => {
    const points = [
      [0, 0, 0.02], [0, 1, 0.05], [0, 2, 0.08],
      [1, 0, -1.98], [1, 1, -1.95], [1, 2, -1.92],
      [2, 0, -1.96], [2, 1, -1.94], [2, 2, -1.91],
    ] as const;
    const measured: SurfaceSession = {
      ...session,
      active: session.active && {
        ...session.active,
        map: {
          ...session.active.map,
          plan: {
            ...session.active.map.plan,
            request: { ...session.active.map.plan.request, columns: 3, rows: 3 },
          },
          samples: points.map(([row, column, zMm], sequence) => ({
            point: { sequence, row, column, xMm: column * 10, yMm: row * 10 },
            zMm,
            triggered: true,
          })),
        },
      },
    };

    const view = surfaceMapExecutionView(measured, "machine-0001", undefined);

    expect(view?.suspiciousNeighborJump).toBe(true);
    expect(view?.maximumNeighborDeltaMm).toBeGreaterThan(1.9);
    expect(view?.detail).toContain("резкий скачок");
  });
});
