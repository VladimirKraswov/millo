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
  probeRemoved: false,
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

export const requiredFirstCutConfirmationKeys = (
  confirmation: FirstCutConfirmation,
): ReadonlyArray<Exclude<keyof FirstCutConfirmation, "intent" | "executionOptions">> => [
  ...firstCutConfirmationKeys(confirmation.intent),
  ...(confirmation.intent === "cutting" && confirmation.executionOptions.surfaceMapId !== undefined
    ? (["probeRemoved"] as const)
    : []),
];

export const setFirstCutReadiness = (
  confirmation: FirstCutConfirmation,
  ready: boolean,
): FirstCutConfirmation => {
  const keys = requiredFirstCutConfirmationKeys(confirmation);
  const confirmed = (
    key: Exclude<keyof FirstCutConfirmation, "intent" | "executionOptions">,
  ) => ready && keys.includes(key);
  return {
    ...confirmation,
    stockSecured: confirmed("stockSecured"),
    toolSecured: confirmed("toolSecured"),
    toolRemoved: confirmed("toolRemoved"),
    xyzZeroVerified: confirmed("xyzZeroVerified"),
    safeZVerified: confirmed("safeZVerified"),
    manualSpindleRunning: confirmed("manualSpindleRunning"),
    manualSpindleOff: confirmed("manualSpindleOff"),
    probeRemoved: confirmed("probeRemoved"),
    pathClear: confirmed("pathClear"),
    powerControlReachable: confirmed("powerControlReachable"),
  };
};

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
  const keys = requiredFirstCutConfirmationKeys(confirmation);
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
