import { invoke } from "@tauri-apps/api/core";

import type {
  RealRunPreflightGateway,
  RunPreflightReport,
} from "../../shared/realRun";

export const tauriRealRunPreflightGateway: RealRunPreflightGateway = {
  preflight: (request) =>
    invoke<RunPreflightReport>("preflight_real_run", { request }),
};
