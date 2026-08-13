import type { SenderState } from "../../shared/dryRun";

const senderStateLabels: Readonly<Record<SenderState, string>> = {
  idle: "Не запускалась",
  ready: "Готово",
  running: "Выполняется",
  paused: "Пауза",
  toolChange: "Смена инструмента",
  draining: "Завершение движения",
  completed: "Завершено",
  failed: "Остановлено из-за ошибки",
  cancelled: "Остановлено",
};

export const senderStateLabel = (state: SenderState): string => senderStateLabels[state];
