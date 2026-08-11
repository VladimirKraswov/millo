import { invoke } from "@tauri-apps/api/core";

import type {
  FirstCutConfirmation,
  FirstCutPreparation,
  RealRunPreflightGateway,
  RunPreflightReport,
  ToolChangeConfirmation,
} from "../../shared/realRun";
import type { SenderSnapshot } from "../../shared/dryRun";

export const tauriRealRunPreflightGateway: RealRunPreflightGateway = {
  preflight: (request, intent) =>
    invoke<RunPreflightReport>("preflight_real_run", { request, intent }),
  authorizeFirstCut: (request, confirmation: FirstCutConfirmation) =>
    invoke<FirstCutPreparation>("authorize_first_cut", {
      request,
      confirmation,
    }),
  startProgram: (request, authorizationId) =>
    invoke<SenderSnapshot>("start_program_run", { request, authorizationId }),
  startCheck: (request) =>
    invoke<SenderSnapshot>("start_check_run", { request }),
  resumeProgram: () => invoke<SenderSnapshot>("resume_program_run"),
  completeToolChange: (confirmation: ToolChangeConfirmation) =>
    invoke<SenderSnapshot>("complete_tool_change", { confirmation }),
};
