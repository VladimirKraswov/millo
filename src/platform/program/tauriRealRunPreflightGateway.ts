import { invoke } from "@tauri-apps/api/core";

import type {
  FirstCutConfirmation,
  FirstCutPreparation,
  RealRunPreflightGateway,
  RunPreflightReport,
} from "../../shared/realRun";

export const tauriRealRunPreflightGateway: RealRunPreflightGateway = {
  preflight: (request) =>
    invoke<RunPreflightReport>("preflight_real_run", { request }),
  authorizeFirstCut: (request, confirmation: FirstCutConfirmation) =>
    invoke<FirstCutPreparation>("authorize_first_cut", {
      request,
      confirmation,
    }),
};
