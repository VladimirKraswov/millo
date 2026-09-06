import type { WorkCoordinateGateway } from "../../platform/machine/WorkCoordinateGateway";
import type {
  ReturnToWorkZeroOutcome,
  WorkAxis,
  WorkZeroOutcome,
} from "../../shared/machine";

export class WorkZeroInteractor {
  constructor(private readonly gateway: WorkCoordinateGateway) {}

  set(axis: WorkAxis, positionConfirmed: boolean): Promise<WorkZeroOutcome> {
    if (!positionConfirmed) {
      throw new Error("work zero requires operator position confirmation");
    }
    return this.gateway.setZero({ axis, positionConfirmed: true });
  }

  returnToZero(axis: WorkAxis, feedMmPerMin: number): Promise<ReturnToWorkZeroOutcome> {
    if (axis === "a") {
      throw new Error("returning A to zero requires explicit rotary clearance");
    }
    if (!Number.isFinite(feedMmPerMin) || feedMmPerMin < 10 || feedMmPerMin > 100_000) {
      throw new Error("return-to-zero feed must be between 10 and 100000 mm/min");
    }
    return this.gateway.returnToZero({ axis, feedMmPerMin });
  }

  async setCartesian(
    positionConfirmed: boolean,
    includeZ = true,
    onOutcome?: (outcome: WorkZeroOutcome) => void,
  ): Promise<WorkZeroOutcome[]> {
    const outcomes: WorkZeroOutcome[] = [];
    const axes: readonly WorkAxis[] = includeZ ? ["x", "y", "z"] : ["x", "y"];
    for (const axis of axes) {
      const outcome = await this.set(axis, positionConfirmed);
      outcomes.push(outcome);
      onOutcome?.(outcome);
    }
    return outcomes;
  }
}
