import type { WorkCoordinateGateway } from "../../platform/machine/WorkCoordinateGateway";
import type { WorkAxis, WorkZeroOutcome } from "../../shared/machine";

export class WorkZeroInteractor {
  constructor(private readonly gateway: WorkCoordinateGateway) {}

  set(axis: WorkAxis, positionConfirmed: boolean): Promise<WorkZeroOutcome> {
    if (!positionConfirmed) {
      throw new Error("work zero requires operator position confirmation");
    }
    return this.gateway.setZero({ axis, positionConfirmed: true });
  }
}
