import type { WorkZeroOutcome, WorkZeroRequest } from "../../shared/machine";

export interface WorkCoordinateGateway {
  setZero(request: WorkZeroRequest): Promise<WorkZeroOutcome>;
}
