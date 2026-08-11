import type {
  HardwareProfile,
  ReadinessCheck,
  ReadinessReport,
} from "../shared/machine";

const checkCopy: Record<string, { title: string; pass: string; caution: string; blocker: string }> = {
  "controller-state": {
    title: "Контроллер",
    pass: "Подключен и ожидает команд",
    caution: "Состояние требует внимания",
    blocker: "Нужен стабильный Idle без Alarm и reset",
  },
  "inspection-queries": {
    title: "Чтение профиля",
    pass: "Все read-only запросы завершены",
    caution: "Профиль считан частично",
    blocker: "Не получены обязательные ответы контроллера",
  },
  firmware: {
    title: "Прошивка",
    pass: "GRBL идентифицирован",
    caution: "Версия требует проверки",
    blocker: "Версия прошивки не определена",
  },
  "axis-steps": {
    title: "Калибровка осей",
    pass: "Шаги X/Y/Z заданы",
    caution: "Калибровка требует проверки",
    blocker: "Нет корректных steps/mm для X/Y/Z",
  },
  "axis-rates": {
    title: "Скорости осей",
    pass: "Лимиты скорости X/Y/Z заданы",
    caution: "Скорости требуют проверки",
    blocker: "Нет корректных max rate для X/Y/Z",
  },
  "axis-acceleration": {
    title: "Ускорения",
    pass: "Ускорения X/Y/Z заданы",
    caution: "Ускорения требуют проверки",
    blocker: "Нет корректных acceleration для X/Y/Z",
  },
  "axis-travel": {
    title: "Рабочий ход",
    pass: "Диапазоны X/Y/Z заданы",
    caution: "Диапазоны требуют проверки",
    blocker: "Нет корректного max travel для X/Y/Z",
  },
  "unhomed-operation": {
    title: "Homing и limits",
    pass: "Конфигурация согласована",
    caution: "Координаты не являются проверенной границей станка",
    blocker: "Настройки противоречат профилю без датчиков",
  },
  "milling-mode": {
    title: "Режим станка",
    pass: "Laser mode отключен",
    caution: "Режим требует проверки",
    blocker: "Для фрезеровки требуется $32=0",
  },
  "modal-units": {
    title: "Единицы и modal state",
    pass: "Активен режим миллиметров",
    caution: "Активен G91; test jog задаст режим явно",
    blocker: "Для первого теста требуется G21",
  },
  spindle: {
    title: "Шпиндель",
    pass: "Контроллер сообщает M5",
    caution: "Ручной шпиндель должен оставаться выключенным",
    blocker: "Контроллер не находится в M5",
  },
  "probe-input": {
    title: "Датчик касания",
    pass: "Вход датчика проверен",
    caution: "Нужен отдельный электрический тест датчика",
    blocker: "Конфигурация входа датчика недоступна",
  },
  "emergency-stop": {
    title: "Аварийная остановка",
    pass: "Физическая кнопка присутствует",
    caution: "Физическая аварийная кнопка отсутствует",
    blocker: "Аварийная остановка недоступна",
  },
};

function profileFacts(profile: HardwareProfile): string[] {
  return [
    profile.axes.join("/"),
    profile.spindleControl === "manual" ? "Ручной шпиндель" : "Управляемый шпиндель",
    profile.homingInstalled ? "Homing установлен" : "Без homing",
    profile.limitSwitchesInstalled ? "Limits установлены" : "Без limits",
  ];
}

function localizedCheck(check: ReadinessCheck): { title: string; detail: string } {
  const copy = checkCopy[check.id];
  if (!copy) return { title: check.title, detail: check.detail };
  return { title: copy.title, detail: copy[check.level] };
}

export function ReadinessPanel({ report }: { report: ReadinessReport }) {
  return (
    <section
      className={`readiness-panel ${report.testJogReady ? "is-ready" : "is-blocked"}`}
      aria-labelledby="readiness-title"
    >
      <div className="readiness-summary">
        <div className="readiness-state" aria-hidden="true">
          <i />
        </div>
        <div>
          <span>Hardware readiness</span>
          <h3 id="readiness-title">
            {report.testJogReady
              ? "Готов к безопасному test jog"
              : "Движение заблокировано"}
          </h3>
          <div className="profile-facts">
            {profileFacts(report.profile).map((fact) => (
              <small key={fact}>{fact}</small>
            ))}
          </div>
        </div>
        <dl>
          <div>
            <dt>Blockers</dt>
            <dd>{report.blockerCount}</dd>
          </div>
          <div>
            <dt>Cautions</dt>
            <dd>{report.cautionCount}</dd>
          </div>
          <div>
            <dt>Probe</dt>
            <dd>{report.probeReady ? "Ready" : "Locked"}</dd>
          </div>
        </dl>
      </div>

      <div className="readiness-checks">
        {report.checks.map((check) => {
          const copy = localizedCheck(check);
          return (
            <div className={`readiness-check is-${check.level}`} key={check.id}>
              <i aria-hidden="true" />
              <div>
                <strong>{copy.title}</strong>
                <span>{copy.detail}</span>
                {check.evidence && <code>{check.evidence}</code>}
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}
