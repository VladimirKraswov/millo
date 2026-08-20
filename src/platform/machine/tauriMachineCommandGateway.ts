import { cancelJog, jogPadStep, startContinuousJog } from "../../api/controller";
import type { MachineCommandGateway } from "./MachineCommandGateway";

export const tauriMachineCommandGateway: MachineCommandGateway = {
  jogPadStep,
  startContinuousJog,
  cancelJog,
};
