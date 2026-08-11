import type {
  FirstCutConfirmation,
  RunPreflightReport,
} from "../../shared/realRun";

export const emptyFirstCutConfirmation: FirstCutConfirmation = {
  stockSecured: false,
  toolSecured: false,
  xyzZeroVerified: false,
  safeZVerified: false,
  manualSpindleRunning: false,
  powerControlReachable: false,
};

export const firstCutConfirmationKeys = [
  "stockSecured",
  "toolSecured",
  "xyzZeroVerified",
  "safeZVerified",
  "manualSpindleRunning",
  "powerControlReachable",
] as const;

export interface FirstCutAuthorizationControls {
  readonly completedCount: number;
  readonly complete: boolean;
  readonly canAuthorize: boolean;
}

export function firstCutAuthorizationControls(
  confirmation: FirstCutConfirmation,
  context: {
    readonly report?: RunPreflightReport;
    readonly gatewayAvailable: boolean;
    readonly busy: boolean;
  },
): FirstCutAuthorizationControls {
  const completedCount = firstCutConfirmationKeys.filter(
    (key) => confirmation[key],
  ).length;
  const complete = completedCount === firstCutConfirmationKeys.length;
  return {
    completedCount,
    complete,
    canAuthorize:
      complete &&
      context.report?.ready === true &&
      context.gatewayAvailable &&
      !context.busy,
  };
}
