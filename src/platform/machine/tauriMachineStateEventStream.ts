import {
  getControllerSnapshot,
  onMachineState,
} from "../../api/controller";
import type { MachineStateEventStream } from "./MachineStateEventStream";

export const tauriMachineStateEventStream: MachineStateEventStream = {
  readCurrent: getControllerSnapshot,
  listen: onMachineState,
};
