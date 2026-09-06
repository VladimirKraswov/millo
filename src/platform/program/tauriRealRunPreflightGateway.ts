import { invoke } from "@tauri-apps/api/core";
import { invokeProgramDocument } from "./tauriProgramDocument";

import type {
  FirstCutConfirmation,
  FirstCutPreparation,
  RealRunPreflightGateway,
  RunPreflightReport,
  SafeStartPackage,
  SelectedRunPreparationRequest,
  ToolChangeConfirmation,
} from "../../shared/realRun";
import type { SenderSnapshot } from "../../shared/dryRun";

export const tauriRealRunPreflightGateway: RealRunPreflightGateway = {
  prepareSelectedRun: ({ request, ...options }: SelectedRunPreparationRequest) =>
    invokeProgramDocument<SafeStartPackage>("prepare_selected_program_run", request, options, true),
  recoveryCandidate: () =>
    invoke<import("../../shared/recovery").ProgramRecoveryCandidate | null>(
      "program_recovery_candidate",
    ),
  prepareRecovery: (request) =>
    invoke<import("../../shared/recovery").ProgramRecoveryPackage>(
      "prepare_program_recovery",
      { request },
    ),
  dismissRecovery: (recoveryId) =>
    invoke<void>("dismiss_program_recovery", { recoveryId }),
  preflight: (request, intent, executionOptions) =>
    invokeProgramDocument<RunPreflightReport>("preflight_real_run", request, {
      intent,
      executionOptions,
    }),
  authorizeFirstCut: (request, confirmation: FirstCutConfirmation) =>
    invokeProgramDocument<FirstCutPreparation>("authorize_first_cut", request, {
      confirmation,
    }),
  startProgram: (request, authorizationId, executionOptions) =>
    invokeProgramDocument<SenderSnapshot>("start_program_run", request, {
      authorizationId,
      executionOptions,
    }),
  startCheck: (request, executionOptions) =>
    invokeProgramDocument<SenderSnapshot>("start_check_run", request, { executionOptions }),
  pauseProgram: () => invoke<SenderSnapshot>("pause_program_run"),
  resumeProgram: () => invoke<SenderSnapshot>("resume_program_run"),
  abortProgram: () => invoke<SenderSnapshot>("abort_program_run"),
  completeToolChange: (confirmation: ToolChangeConfirmation) =>
    invoke<SenderSnapshot>("complete_tool_change", { confirmation }),
};
