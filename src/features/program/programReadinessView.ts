import type { RunPreflightReport } from "../../shared/realRun";
import type { ProgramRecoveryCandidate } from "../../shared/recovery";
import { jobReadinessModel } from "./jobReadinessModel";
import {
  formatProgramDiagnostics,
  programCanEnterPreflight,
  programDiagnosticsSummary,
} from "./programDiagnosticsModel";
import type { ProgramWorkspaceProps } from "./programWorkspaceTypes";
import { realRunPreflightControls } from "./realRunPreflightReadModel";

import type { GcodeProgram } from "../../shared/program";
interface ProgramReadinessOptions
  extends Pick<ProgramWorkspaceProps, "machineContext" | "realRunGateway"> {
  realRunAvailable: boolean;
  reportForProgram?: RunPreflightReport;
  preflightLoading: boolean;
  recoveryChecked: boolean;
  recoveryCandidate?: ProgramRecoveryCandidate;
  program?: GcodeProgram;
}
export function programReadinessView({
  machineContext,
  realRunAvailable,
  realRunGateway,
  reportForProgram,
  preflightLoading,
  recoveryChecked,
  recoveryCandidate,
  program,
}: ProgramReadinessOptions) {
  const preflightControls = realRunPreflightControls(reportForProgram, {
    serialAvailable: realRunAvailable,
    gatewayAvailable: realRunGateway !== undefined,
    checking: preflightLoading,
  });
  const requiresGrblCheck =
    reportForProgram?.checks.some(
      (check) =>
        check.id === "grbl-check-certificate" && check.level === "blocker",
    ) ?? false;
  const readiness = jobReadinessModel({
    alarm: machineContext?.snapshot.alarm !== undefined,
    connection: machineContext?.snapshot.connection ?? "disconnected",
    machineBound: machineContext?.machineBound ?? false,
    machineSyncing: machineContext?.machineSyncing ?? false,
    machineMode: machineContext?.snapshot.machine.mode ?? "unknown",
    parserEligible: program ? programCanEnterPreflight(program) : false,
    preflightStatus: preflightControls.status,
    resetPending: machineContext?.snapshot.resetNotice !== undefined,
    recoveryStatus: !recoveryChecked
      ? "checking"
      : recoveryCandidate
        ? "outstanding"
        : "clear",
    requiresGrblCheck,
    workPositionAvailable: machineContext?.workPosition !== undefined,
  });
  const machineDetail = machineContext
    ? machineContext.snapshot.connection !== "connected"
      ? "Не подключен"
      : machineContext.snapshot.alarm
        ? `ALARM${machineContext.snapshot.alarm.code === undefined ? "" : `:${machineContext.snapshot.alarm.code}`}`
        : machineContext.snapshot.resetNotice
          ? "Контроллер перезапущен"
          : machineContext.machineSyncing
            ? "Читаем привязку профиля из контроллера"
            : !machineContext.machineBound
              ? "Профиль не синхронизирован с подключённым контроллером"
              : `${machineContext.machineName} · ${machineContext.snapshot.machine.reportedMode}`
    : "Не подключен";
  const programDiagnostics = program
    ? programDiagnosticsSummary(program)
    : undefined;
  const programDiagnosticsDetail = programDiagnostics
    ? formatProgramDiagnostics(programDiagnostics)
    : "";
  const fileDetail =
    program && programCanEnterPreflight(program)
      ? `${program.summary.lineCount} строк${programDiagnosticsDetail ? ` · ${programDiagnosticsDetail}` : ""}`
      : programDiagnostics?.actionableCount
        ? `${programDiagnostics.actionableCount} замечаний требуют внимания`
        : program
          ? "Не удалось построить полный preview"
          : "Файл не загружен";
  const originDetail = machineContext?.workPosition
    ? `${machineContext.activeCoordinateSystem} · X ${machineContext.workPosition.x.toFixed(3)} · Y ${machineContext.workPosition.y.toFixed(3)} · Z ${machineContext.workPosition.z.toFixed(3)}`
    : `${machineContext?.activeCoordinateSystem ?? "G54"} · не установлен`;
  const validationDetail = !recoveryChecked
    ? "Проверяем историю запусков"
    : recoveryCandidate
      ? `Нужно решение: ${recoveryCandidate.sourceName}`
      : preflightControls.status === "ready"
        ? `Готово · ${reportForProgram?.cautionCount ?? 0} замечаний`
        : preflightControls.status === "blocked"
          ? requiresGrblCheck
            ? "Нужна проверка контроллером"
            : `${reportForProgram?.blockerCount ?? 0} блокирующих замечаний`
          : preflightControls.status === "checking"
            ? "Читаем состояние GRBL"
            : "Еще не выполнялась";

  return {
    preflightControls,
    readiness,
    machineDetail,
    fileDetail,
    originDetail,
    validationDetail,
  };
}
