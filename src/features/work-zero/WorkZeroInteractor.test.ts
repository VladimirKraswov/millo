import { describe, expect, it, vi } from "vitest";

import type { WorkCoordinateGateway } from "../../platform/machine/WorkCoordinateGateway";
import type { ReturnToWorkZeroOutcome, WorkZeroOutcome } from "../../shared/machine";
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
const returnOutcome: ReturnToWorkZeroOutcome = {
  axis: "z",
  coordinateSystem: "g54",
  command: "$J=G90 G21 Z0.000 F100.000",
  snapshot: emptySnapshot,
};

describe("WorkZeroInteractor", () => {
  it("requires confirmation for A and never includes A in Cartesian zero", async () => {
    const setZero = vi.fn(async () => outcome);
    const interactor = new WorkZeroInteractor({ setZero, returnToZero: vi.fn() });
    expect(() => interactor.set("a", false)).toThrow("confirmation");
    await interactor.set("a", true);
    expect(setZero).toHaveBeenLastCalledWith({ axis: "a", positionConfirmed: true });
    setZero.mockClear();
    await interactor.setCartesian(true);
    expect(setZero).toHaveBeenCalledTimes(3);
    expect(setZero).toHaveBeenNthCalledWith(1, { axis: "x", positionConfirmed: true });
    expect(setZero).toHaveBeenNthCalledWith(2, { axis: "y", positionConfirmed: true });
    expect(setZero).toHaveBeenNthCalledWith(3, { axis: "z", positionConfirmed: true });
  });

  it("rejects rotary return before sending a motion request", () => {
    const returnToZero = vi.fn();
    const interactor = new WorkZeroInteractor({ setZero: vi.fn(), returnToZero });
    expect(() => interactor.returnToZero("a", 360)).toThrow("rotary clearance");
    expect(returnToZero).not.toHaveBeenCalled();
  });

  it("publishes completed Cartesian zeros even when a later axis fails", async () => {
    const setZero = vi.fn().mockResolvedValueOnce(outcome).mockRejectedValueOnce(new Error("Y failed"));
    const onOutcome = vi.fn();
    const interactor = new WorkZeroInteractor({ setZero, returnToZero: vi.fn() });
    await expect(interactor.setCartesian(true, true, onOutcome)).rejects.toThrow("Y failed");
    expect(onOutcome).toHaveBeenCalledOnce();
    expect(onOutcome).toHaveBeenCalledWith(outcome);
    expect(setZero).toHaveBeenCalledTimes(2);
  });
  it("rejects missing confirmation before reaching the gateway", () => {
    const setZero = vi.fn(async () => outcome);
    const interactor = new WorkZeroInteractor({ setZero, returnToZero: vi.fn() });

    expect(() => interactor.set("x", false)).toThrow(
      "work zero requires operator position confirmation",
    );
    expect(setZero).not.toHaveBeenCalled();
  });

  it("delegates one typed, confirmed axis request", async () => {
    const setZero = vi.fn(async () => outcome);
    const gateway: WorkCoordinateGateway = { setZero, returnToZero: vi.fn() };
    const interactor = new WorkZeroInteractor(gateway);

    await expect(interactor.set("x", true)).resolves.toBe(outcome);
    expect(setZero).toHaveBeenCalledOnce();
    expect(setZero).toHaveBeenCalledWith({
      axis: "x",
      positionConfirmed: true,
    });
  });

  it("delegates one typed absolute return and validates its feed", async () => {
    const returnToZero = vi.fn(async () => returnOutcome);
    const interactor = new WorkZeroInteractor({ setZero: vi.fn(), returnToZero });

    await expect(interactor.returnToZero("z", 100)).resolves.toBe(returnOutcome);
    expect(returnToZero).toHaveBeenCalledWith({ axis: "z", feedMmPerMin: 100 });
    expect(() => interactor.returnToZero("z", 0)).toThrow("between 10 and 100000");
  });
});
