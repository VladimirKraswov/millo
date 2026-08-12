import type { ConnectionState, MachineMode } from "../../shared/machine";
import type { RealRunPreflightStatus } from "./realRunPreflightReadModel";

export type JobReadinessAction =
  | "connect"
  | "unlock"
  | "acknowledgeReset"
  | "waitForIdle"
  | "setWorkZero"
  | "reviewProgram"
  | "runPreflight"
  | "runGrblCheck"
  | "resolveRecovery"
  | "startProgram";

export type JobReadinessStepId = "machine" | "file" | "origin" | "validation";
export type JobReadinessStepState = "ready" | "action" | "blocked" | "pending";

export interface JobReadinessStep {
  readonly id: JobReadinessStepId;
  readonly state: JobReadinessStepState;
}

export interface JobReadinessView {
  readonly steps: readonly JobReadinessStep[];
  readonly primaryAction: JobReadinessAction;
  readonly primaryLabel: string;
  readonly primaryDisabled: boolean;
}

export interface JobReadinessInput {
  readonly alarm: boolean;
  readonly connection: ConnectionState;
  readonly machineBound: boolean;
  readonly machineMode: MachineMode;
  readonly parserEligible: boolean;
  readonly preflightStatus: RealRunPreflightStatus;
  readonly resetPending: boolean;
  readonly recoveryStatus: "checking" | "clear" | "outstanding";
  readonly requiresGrblCheck: boolean;
  readonly workPositionAvailable: boolean;
}

const step = (
  id: JobReadinessStepId,
  state: JobReadinessStepState,
): JobReadinessStep => ({ id, state });

export function jobReadinessModel(input: JobReadinessInput): JobReadinessView {
  const connected = input.connection === "connected";
  const machineReady =
    connected &&
    input.machineBound &&
    input.machineMode === "idle" &&
    !input.alarm &&
    !input.resetPending;
  const fileReady = input.parserEligible;
  const originReady = input.workPositionAvailable;
  const validationReady = input.preflightStatus === "ready";

  const steps: readonly JobReadinessStep[] = [
    step(
      "machine",
      machineReady
        ? "ready"
        : input.alarm || input.resetPending || connected
          ? "blocked"
          : "action",
    ),
    step("file", fileReady ? "ready" : "blocked"),
    step("origin", originReady ? "ready" : connected ? "action" : "pending"),
    step(
      "validation",
      input.recoveryStatus === "checking"
        ? "pending"
        : input.recoveryStatus === "outstanding"
          ? "blocked"
          : validationReady
            ? "ready"
            : input.preflightStatus === "checking"
              ? "pending"
              : input.preflightStatus === "blocked"
                ? "blocked"
                : "action",
    ),
  ];

  if (!connected) {
    return {
      steps,
      primaryAction: "connect",
      primaryLabel: input.connection === "connecting" ? "Подключение..." : "Подключить станок",
      primaryDisabled: input.connection === "connecting",
    };
  }
  if (input.alarm) {
    return {
      steps,
      primaryAction: "unlock",
      primaryLabel: "Разблокировать станок",
      primaryDisabled: false,
    };
  }
  if (input.resetPending) {
    return {
      steps,
      primaryAction: "acknowledgeReset",
      primaryLabel: "Подтвердить перезапуск",
      primaryDisabled: false,
    };
  }
  if (!input.machineBound) {
    return {
      steps,
      primaryAction: "waitForIdle",
      primaryLabel: "Выберите профиль станка",
      primaryDisabled: true,
    };
  }
  if (input.machineMode !== "idle") {
    return {
      steps,
      primaryAction: "waitForIdle",
      primaryLabel: "Дождитесь состояния Idle",
      primaryDisabled: true,
    };
  }
  if (input.recoveryStatus === "checking") {
    return {
      steps,
      primaryAction: "resolveRecovery",
      primaryLabel: "Проверяем предыдущий запуск...",
      primaryDisabled: true,
    };
  }
  if (input.recoveryStatus === "outstanding") {
    return {
      steps,
      primaryAction: "resolveRecovery",
      primaryLabel: "Разобраться с прошлым запуском",
      primaryDisabled: false,
    };
  }
  if (!fileReady) {
    return {
      steps,
      primaryAction: "reviewProgram",
      primaryLabel: "Проверить программу",
      primaryDisabled: false,
    };
  }
  if (!originReady) {
    return {
      steps,
      primaryAction: "setWorkZero",
      primaryLabel: "Установить рабочий ноль",
      primaryDisabled: false,
    };
  }
  if (input.preflightStatus === "checking") {
    return {
      steps,
      primaryAction: "runPreflight",
      primaryLabel: "Проверяем станок...",
      primaryDisabled: true,
    };
  }
  if (input.requiresGrblCheck) {
    return {
      steps,
      primaryAction: "runGrblCheck",
      primaryLabel: "Проверить G-code через GRBL",
      primaryDisabled: false,
    };
  }
  if (input.preflightStatus === "blocked") {
    return {
      steps,
      primaryAction: "reviewProgram",
      primaryLabel: "Посмотреть замечания",
      primaryDisabled: false,
    };
  }
  if (!validationReady) {
    return {
      steps,
      primaryAction: "runPreflight",
      primaryLabel: "Проверить готовность",
      primaryDisabled: input.preflightStatus === "unavailable",
    };
  }
  return {
    steps,
    primaryAction: "startProgram",
    primaryLabel: "Запустить программу",
    primaryDisabled: false,
  };
}
