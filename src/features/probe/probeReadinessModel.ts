export type ProbeReadinessAction = "касанию" | "измерению";

export const describeProbeReadinessFailure = (
  error: unknown,
  action: ProbeReadinessAction,
): string | undefined => {
  const message = String(error);
  if (message.includes("probe start timed out")) {
    return `Контроллер не завершил предыдущее движение за 3 секунды. Дождитесь состояния Idle и повторите попытку ${action}.`;
  }
  if (message.includes("probe start is blocked")) {
    if (message.includes("connection Disconnected") || message.includes("connection Faulted")) {
      return `Связь с контроллером потеряна. Переподключите станок и повторите попытку ${action}.`;
    }
    if (message.includes("alarm true") || message.includes("mode Alarm")) {
      return "Контроллер находится в Alarm. Устраните причину и разблокируйте станок перед продолжением.";
    }
    if (message.includes("reset acknowledgement pending true")) {
      return "Контроллер был перезапущен. Подтвердите восстановление состояния перед продолжением.";
    }
    return `Контроллер пока не готов к ${action}. Дождитесь состояния Idle и повторите запуск.`;
  }
  if (message.includes("another machine operation is active")) {
    return `Сначала завершите или отмените текущее задание, затем повторите попытку ${action}.`;
  }
  if (message.includes("controller is not connected and idle")) {
    return `Контроллер ещё завершает предыдущее действие. Дождитесь состояния Idle и повторите попытку ${action}.`;
  }
  return undefined;
};
