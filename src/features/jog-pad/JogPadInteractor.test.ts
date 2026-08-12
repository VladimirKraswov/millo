import { describe, expect, it, vi } from "vitest";

import type { MachineCommandGateway } from "../../platform/machine/MachineCommandGateway";
import type {
  JogPadStepOutcome,
  OperatorConfirmation,
} from "../../shared/machine";
import {
  JogPadInteractor,
  jogOperatorConfirmation,
  type JogPadStepMm,
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

  it("turns one press into one signed fixed-step gateway call", async () => {
    const jogPadStep = vi.fn(async () => outcome);
    const interactor = new JogPadInteractor({ jogPadStep });

    await interactor.move(confirmation, "y", -1, 0.1);

    expect(jogPadStep).toHaveBeenCalledOnce();
    expect(jogPadStep).toHaveBeenCalledWith({
      confirmation,
      axis: "y",
      distanceMm: -0.1,
    });
  });

  it("rejects a value outside the pad presets before the gateway", async () => {
    const jogPadStep = vi.fn(async () => outcome);
    const interactor = new JogPadInteractor({ jogPadStep });

    await expect(
      interactor.move(confirmation, "x", 1, 0.5 as JogPadStepMm),
    ).rejects.toThrow("unsupported jog pad step");
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
    const first = interactor.move(confirmation, "z", 1, 0.01);

    await expect(
      interactor.move(confirmation, "z", -1, 0.01),
    ).rejects.toThrow("already in progress");
    expect(gateway.jogPadStep).toHaveBeenCalledOnce();

    resolveFirst?.(outcome);
    await first;
  });
});
