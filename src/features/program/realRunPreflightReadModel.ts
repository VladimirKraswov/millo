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
    return { canCheck: false, status: "unavailable", statusLabel: "Недоступно" };
  }
  if (context.checking) {
    return { canCheck: false, status: "checking", statusLabel: "Читаем контроллер" };
  }
  if (!report) {
    return { canCheck, status: "unchecked", statusLabel: "Не проверено" };
  }
  return report.ready
    ? { canCheck, status: "ready", statusLabel: "Проверка пройдена" }
    : { canCheck, status: "blocked", statusLabel: "Нужно действие" };
}
