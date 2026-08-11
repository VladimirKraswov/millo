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
  preflight: (request, intent, executionOptions) =>
    invoke<RunPreflightReport>("preflight_real_run", {
      request,
      intent,
      executionOptions,
    }),
  authorizeFirstCut: (request, confirmation: FirstCutConfirmation) =>
    invoke<FirstCutPreparation>("authorize_first_cut", {
      request,
      confirmation,
    }),
  startProgram: (request, authorizationId, executionOptions) =>
    invoke<SenderSnapshot>("start_program_run", {
      request,
      authorizationId,
      executionOptions,
    }),
  startCheck: (request, executionOptions) =>
    invoke<SenderSnapshot>("start_check_run", { request, executionOptions }),
  resumeProgram: () => invoke<SenderSnapshot>("resume_program_run"),
  completeToolChange: (confirmation: ToolChangeConfirmation) =>
    invoke<SenderSnapshot>("complete_tool_change", { confirmation }),
};
