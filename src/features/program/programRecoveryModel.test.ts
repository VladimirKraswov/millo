import { describe, expect, it } from "vitest";

import type { ProgramRecoveryCandidate } from "../../shared/recovery";
import {
  canPrepareRecovery,
  emptyRecoveryPreparation,
  recoverySafeZDefault,
} from "./programRecoveryModel";

const candidate: ProgramRecoveryCandidate = {
  id: 7,
  sourceName: "job.nc",
  intent: "cutting",
  state: "running",
  updatedAtUnixMs: 1,
  totalLines: 100,
  acknowledgedLines: 80,
  executingSourceLine: 74,
  restartSourceLine: 68,
  restartPosition: { x: 10, y: 20, z: 5 },
  minimumSafeZMm: 5,
  ready: true,
  detail: "rewind",
};

describe("program recovery model", () => {
  it("defaults clearance above the proven program envelope", () => {
    expect(recoverySafeZDefault(candidate)).toBe(7);
  });

  it("requires every recovery-specific operator confirmation", () => {
    const empty = emptyRecoveryPreparation(candidate);
    expect(canPrepareRecovery(candidate, empty, false)).toBe(false);
    expect(
      canPrepareRecovery(
        candidate,
        {
          ...empty,
          machineReferenceRestored: true,
          workZeroRestored: true,
          restartPointInspected: true,
          pathClear: true,
          powerControlReachable: true,
        },
        false,
      ),
    ).toBe(true);
  });

  it("keeps missing Ln telemetry and low Safe Z fail-closed", () => {
    const blocked = { ...candidate, ready: false, executingSourceLine: undefined };
    const request = {
      ...emptyRecoveryPreparation(candidate),
      machineReferenceRestored: true,
      workZeroRestored: true,
      restartPointInspected: true,
      pathClear: true,
      powerControlReachable: true,
      safeZMm: 4.9,
    };
    expect(canPrepareRecovery(candidate, request, false)).toBe(false);
    expect(canPrepareRecovery(blocked, { ...request, safeZMm: 7 }, false)).toBe(false);
  });

  it("rejects non-finite clearance and a duplicate submit while busy", () => {
    const confirmed = {
      ...emptyRecoveryPreparation(candidate),
      machineReferenceRestored: true,
      workZeroRestored: true,
      restartPointInspected: true,
      pathClear: true,
      powerControlReachable: true,
    };

    expect(canPrepareRecovery(candidate, { ...confirmed, safeZMm: Number.NaN }, false)).toBe(false);
    expect(canPrepareRecovery(candidate, confirmed, true)).toBe(false);
  });
});
