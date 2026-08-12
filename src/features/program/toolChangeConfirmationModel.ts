import type { ToolChangeConfirmation } from "../../shared/realRun";

export type ToolChangeChecklistKey = Exclude<
  keyof ToolChangeConfirmation,
  "sourceLine" | "requestedTool"
>;

export const toolChangeChecklistKeys: readonly ToolChangeChecklistKey[] = [
  "toolSecured",
  "zZeroVerified",
  "safeZVerified",
  "pathClear",
  "manualSpindleRunning",
  "powerControlReachable",
];

export const emptyToolChangeConfirmation = (
  sourceLine: number,
  requestedTool?: number,
): ToolChangeConfirmation => ({
  sourceLine,
  requestedTool,
  toolSecured: false,
  zZeroVerified: false,
  safeZVerified: false,
  pathClear: false,
  manualSpindleRunning: false,
  powerControlReachable: false,
});

export const setToolChangeReadiness = (
  confirmation: ToolChangeConfirmation,
  ready: boolean,
): ToolChangeConfirmation => {
  return {
    ...confirmation,
    toolSecured: ready,
    zZeroVerified: ready,
    safeZVerified: ready,
    pathClear: ready,
    manualSpindleRunning: ready,
    powerControlReachable: ready,
  };
};

export const toolChangeConfirmationProgress = (
  confirmation: ToolChangeConfirmation,
): { readonly completed: number; readonly total: number; readonly complete: boolean } => {
  const completed = toolChangeChecklistKeys.filter(
    (key) => confirmation[key],
  ).length;
  return {
    completed,
    total: toolChangeChecklistKeys.length,
    complete: completed === toolChangeChecklistKeys.length,
  };
};
