import type { MachineCommandGateway } from "../../platform/machine/MachineCommandGateway";
import type {
  JogAxis,
  JogPadStepOutcome,
  OperatorConfirmation,
} from "../../shared/machine";

export const JOG_PAD_STEPS_MM = [0.01, 0.1] as const;
export const JOG_PAD_FEED_MM_PER_MIN = 10;

export type JogPadStepMm = (typeof JOG_PAD_STEPS_MM)[number];
export type JogDirection = -1 | 1;

export const jogOperatorConfirmation = (
  ready: boolean,
): OperatorConfirmation => ({
  spindleOff: ready,
  toolClear: ready,
  powerControlReachable: ready,
});

export class JogPadInteractor {
  private inFlight = false;

  constructor(private readonly gateway: MachineCommandGateway) {}

  async move(
    confirmation: OperatorConfirmation,
    axis: JogAxis,
    direction: JogDirection,
    stepMm: JogPadStepMm,
  ): Promise<JogPadStepOutcome> {
    if (this.inFlight) {
      throw new Error("jog pad command is already in progress");
    }
    if (!JOG_PAD_STEPS_MM.includes(stepMm)) {
      throw new Error(`unsupported jog pad step: ${stepMm}`);
    }

    this.inFlight = true;
    try {
      return await this.gateway.jogPadStep({
        confirmation,
        axis,
        distanceMm: direction * stepMm,
      });
    } finally {
      this.inFlight = false;
    }
  }
}
