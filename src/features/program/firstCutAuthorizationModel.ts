import type {
  FirstCutConfirmation,
  RunPreflightReport,
} from "../../shared/realRun";

export const emptyFirstCutConfirmation: FirstCutConfirmation = {
  intent: "airRun",
  executionOptions: { optionalStop: false, blockDelete: false },
  stockSecured: false,
  toolSecured: false,
  toolRemoved: false,
  xyzZeroVerified: false,
  safeZVerified: false,
  manualSpindleRunning: false,
  manualSpindleOff: false,
  pathClear: false,
  powerControlReachable: false,
};

const commonConfirmationKeys = [
  "xyzZeroVerified",
  "safeZVerified",
  "pathClear",
  "powerControlReachable",
] as const;

const airRunConfirmationKeys = ["toolRemoved", "manualSpindleOff"] as const;
const cuttingConfirmationKeys = [
  "stockSecured",
  "toolSecured",
  "manualSpindleRunning",
] as const;

export const firstCutConfirmationKeys = (
  intent: FirstCutConfirmation["intent"],
): ReadonlyArray<
  Exclude<keyof FirstCutConfirmation, "intent" | "executionOptions">
> => [
  ...commonConfirmationKeys,
  ...(intent === "airRun" ? airRunConfirmationKeys : cuttingConfirmationKeys),
];

export interface FirstCutAuthorizationControls {
  readonly completedCount: number;
  readonly totalCount: number;
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
  const keys = firstCutConfirmationKeys(confirmation.intent);
  const completedCount = keys.filter(
    (key) => confirmation[key],
  ).length;
  const complete = completedCount === keys.length;
  return {
    completedCount,
    totalCount: keys.length,
    complete,
    canAuthorize:
      complete &&
      context.report?.ready === true &&
      context.gatewayAvailable &&
      !context.busy,
  };
}
