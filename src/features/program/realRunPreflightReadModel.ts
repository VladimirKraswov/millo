import type { RunPreflightReport } from "../../shared/realRun";

export type RealRunPreflightStatus =
  | "unavailable"
  | "unchecked"
  | "checking"
  | "ready"
  | "blocked";

export interface RealRunPreflightControls {
  readonly canCheck: boolean;
  readonly status: RealRunPreflightStatus;
  readonly statusLabel: string;
}
export function realRunPreflightControls(
  report: RunPreflightReport | undefined,
  context: {
    readonly serialAvailable: boolean;
    readonly gatewayAvailable: boolean;
    readonly checking: boolean;
  },
): RealRunPreflightControls {
  const canCheck =
    context.serialAvailable && context.gatewayAvailable && !context.checking;
  if (!context.serialAvailable || !context.gatewayAvailable) {
    return { canCheck: false, status: "unavailable", statusLabel: "Unavailable" };
  }
  if (context.checking) {
    return { canCheck: false, status: "checking", statusLabel: "Reading controller" };
  }
  if (!report) {
    return { canCheck, status: "unchecked", statusLabel: "Not checked" };
  }
  return report.ready
    ? { canCheck, status: "ready", statusLabel: "Preflight clear" }
    : { canCheck, status: "blocked", statusLabel: "Blocked" };
}
