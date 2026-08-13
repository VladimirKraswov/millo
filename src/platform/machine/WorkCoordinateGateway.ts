import type {
  ReturnToWorkOriginOutcome,
  ReturnToWorkOriginRequest,
  ReturnToWorkZeroOutcome,
  ReturnToWorkZeroRequest,
  WorkZeroOutcome,
  WorkZeroRequest,
} from "../../shared/machine";

export interface WorkCoordinateGateway {
  setZero(request: WorkZeroRequest): Promise<WorkZeroOutcome>;
  returnToZero(request: ReturnToWorkZeroRequest): Promise<ReturnToWorkZeroOutcome>;
  returnToOrigin?(request: ReturnToWorkOriginRequest): Promise<ReturnToWorkOriginOutcome>;
}
