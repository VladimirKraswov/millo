import { describe, expect, it, vi } from "vitest";

import type { WorkCoordinateGateway } from "../../platform/machine/WorkCoordinateGateway";
import type { WorkZeroOutcome } from "../../shared/machine";
import { emptySnapshot } from "../../shared/machine";
import { WorkZeroInteractor } from "./WorkZeroInteractor";

const outcome: WorkZeroOutcome = {
  axis: "x",
  coordinateSystem: "g54",
  command: "G10 L20 P1 X0",
  parameterValue: "10.000,0.000,0.000",
  workPosition: 0,
  snapshot: emptySnapshot,
};

describe("WorkZeroInteractor", () => {
  it("rejects missing confirmation before reaching the gateway", () => {
    const setZero = vi.fn(async () => outcome);
    const interactor = new WorkZeroInteractor({ setZero });

    expect(() => interactor.set("x", false)).toThrow(
      "work zero requires operator position confirmation",
    );
    expect(setZero).not.toHaveBeenCalled();
  });

  it("delegates one typed, confirmed axis request", async () => {
    const setZero = vi.fn(async () => outcome);
    const gateway: WorkCoordinateGateway = { setZero };
    const interactor = new WorkZeroInteractor(gateway);

    await expect(interactor.set("x", true)).resolves.toBe(outcome);
    expect(setZero).toHaveBeenCalledOnce();
    expect(setZero).toHaveBeenCalledWith({
      axis: "x",
      positionConfirmed: true,
    });
  });
});
