import { runZProbe } from "../../api/controller";
import type { ZProbeGateway } from "./ZProbeGateway";

export const tauriZProbeGateway: ZProbeGateway = {
  run: runZProbe,
};
