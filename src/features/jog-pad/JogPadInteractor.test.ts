import { describe, expect, it, vi } from "vitest";

import type { MachineCommandGateway } from "../../platform/machine/MachineCommandGateway";
import type {
  JogPadStepOutcome,
  OperatorConfirmation,
} from "../../shared/machine";
import {
  JogPadInteractor,
  MAX_JOG_DISTANCE_MM,
  MAX_JOG_FEED_MM_PER_MIN,
  jogMotionProfiles,
  jogOperatorConfirmation,
} from "./JogPadInteractor";

const confirmation: OperatorConfirmation = {
  spindleOff: true,
  toolClear: true,
  powerControlReachable: true,
};

const outcome: JogPadStepOutcome = {
  inspection: {
    device: {
      settings: {},
      parameters: {},
      modalState: [],
      responses: [],
    },
    readiness: {
      profile: {
        name: "Test",
        axes: ["X", "Y", "Z"],
        spindleControl: "manual",
        homingInstalled: false,
        limitSwitchesInstalled: false,
      probeInstalled: false,
      probeMode: "off",
        emergencyStopInstalled: false,
      },
      testJogReady: true,
      probeReady: false,
      blockerCount: 0,
      cautionCount: 0,
      checks: [],
    },
  },
};

describe("JogPadInteractor", () => {
  it("expands the single operator arm into the typed backend facts", () => {
    expect(jogOperatorConfirmation(true)).toEqual(confirmation);
    expect(jogOperatorConfirmation(false)).toEqual({
      spindleOff: false,
      toolClear: false,
      powerControlReachable: false,
    });
  });

  it("turns one press into one signed bounded gateway call", async () => {
    const jogPadStep = vi.fn(async () => outcome);
    const interactor = new JogPadInteractor({ jogPadStep });

    await interactor.move(confirmation, "y", -1, 10, 800);

    expect(jogPadStep).toHaveBeenCalledOnce();
    expect(jogPadStep).toHaveBeenCalledWith({
      confirmation,
      axis: "y",
      distanceMm: -10,
      feedMmPerMin: 800,
    });
  });

  it("scales motion profiles to the selected machine", () => {
    expect(jogMotionProfiles(50, 1_000)).toEqual([
      { id: "precision", label: "Точно", distanceMm: 0.1, feedMmPerMin: 100 },
      { id: "position", label: "Позиция", distanceMm: 1, feedMmPerMin: 300 },
      { id: "traverse", label: "Быстро", distanceMm: 10, feedMmPerMin: 800 },
    ]);
    expect(jogMotionProfiles(3_000, 6_000)[2]).toEqual({
      id: "traverse",
      label: "Быстро",
      distanceMm: 300,
      feedMmPerMin: 4_800,
    });
  });

  it("rejects values outside the technical envelope before the gateway", async () => {
    const jogPadStep = vi.fn(async () => outcome);
    const interactor = new JogPadInteractor({ jogPadStep });

    await expect(
      interactor.move(confirmation, "x", 1, MAX_JOG_DISTANCE_MM + 0.01, 300),
    ).rejects.toThrow("jog distance");
    await expect(
      interactor.move(confirmation, "x", 1, 1, MAX_JOG_FEED_MM_PER_MIN + 1),
    ).rejects.toThrow("jog feed");
    expect(jogPadStep).not.toHaveBeenCalled();
  });

  it("rejects a concurrent press while the first call is unresolved", async () => {
    let resolveFirst: ((value: JogPadStepOutcome) => void) | undefined;
    const gateway: MachineCommandGateway = {
      jogPadStep: vi.fn(
        () =>
          new Promise<JogPadStepOutcome>((resolve) => {
            resolveFirst = resolve;
          }),
      ),
    };
    const interactor = new JogPadInteractor(gateway);
    const first = interactor.move(confirmation, "z", 1, 1, 300);

    await expect(
      interactor.move(confirmation, "z", -1, 1, 300),
    ).rejects.toThrow("already in progress");
    expect(gateway.jogPadStep).toHaveBeenCalledOnce();

    resolveFirst?.(outcome);
    await first;
  });
});
