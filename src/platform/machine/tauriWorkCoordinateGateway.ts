import { returnToWorkOrigin, returnToWorkZero, setWorkZero } from "../../api/controller";
import type { WorkCoordinateGateway } from "./WorkCoordinateGateway";

export const tauriWorkCoordinateGateway: WorkCoordinateGateway = {
  setZero: setWorkZero,
  returnToZero: returnToWorkZero,
  returnToOrigin: returnToWorkOrigin,
};
