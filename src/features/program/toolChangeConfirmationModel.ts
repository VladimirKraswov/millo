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
