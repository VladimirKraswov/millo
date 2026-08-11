import { setWorkZero } from "../../api/controller";
import type { WorkCoordinateGateway } from "./WorkCoordinateGateway";

export const tauriWorkCoordinateGateway: WorkCoordinateGateway = {
  setZero: setWorkZero,
};
