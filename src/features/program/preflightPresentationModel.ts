import type {
  RunPreflightCheck,
  RunPreflightReport,
  RunProgramBlocker,
} from "../../shared/realRun";

export interface PresentedPreflightCheck extends RunPreflightCheck {
  readonly title: string;
  readonly detail: string;
}

export interface PresentedPreflightReport {
  readonly attention: readonly PresentedPreflightCheck[];
  readonly passed: readonly PresentedPreflightCheck[];
  readonly summary: string;
  readonly title: string;
}

const blockerLabels: Readonly<Record<string, string>> = {
  "spindle-activation": "включение шпинделя",
  "spindle-speed": "ненулевая скорость шпинделя",
  "coolant-activation": "включение охлаждения",
  "probe-cycle": "цикл щупа G38.x",
  "tool-change": "смена инструмента M6",
  "machine-coordinate-motion": "движение в машинных координатах",
  "coordinate-mutation": "изменение рабочих координат",
  "unsupported-program": "неподдерживаемая команда",
  "incomplete-preview": "неполная траектория",
  "command-too-long": "слишком длинная команда",
  "heightmap-compensation": "ошибка компенсации карты высот",
};

function plural(count: number, one: string, few: string, many: string): string {
  const tens = count % 100;
  const units = count % 10;
  if (tens >= 11 && tens <= 14) return many;
  if (units === 1) return one;
  if (units >= 2 && units <= 4) return few;
  return many;
}

function programBlockerDetail(blockers: readonly RunProgramBlocker[]): string {
  const first = blockers[0];
  if (!first) return "Программа содержит команду, которую нельзя безопасно выполнить в выбранном режиме.";
  const label = blockerLabels[first.kind] ?? "неподдерживаемая команда";
  const remainder = blockers.length - 1;
  return remainder > 0
    ? `Найдено: ${label}${first.sourceLine === undefined ? "" : ` в L${first.sourceLine}`} и ещё ${remainder}.`
    : `Найдено: ${label}${first.sourceLine === undefined ? "" : ` в L${first.sourceLine}`}.`;
}

function modalContractDetail(check: RunPreflightCheck): string {
  if (check.level === "pass") {
    return "Единицы, режим координат, подача и рабочая плоскость заданы до первого движения.";
  }
  const missing = check.detail.match(/^Declare (.+) before /)?.[1];
  return missing
    ? `Перед первым движением явно задайте: ${missing}.`
    : "Перед первым движением явно задайте единицы, режим координат, подачу и рабочую плоскость.";
}

function geometryDetail(check: RunPreflightCheck): string {
  if (check.level !== "pass") {
    return "Нужна полностью рассчитанная траектория хотя бы с одним перемещением.";
  }
  const parsed = check.detail.match(/^(\d+) motion\(s\) · (.+)$/);
  if (!parsed) return "Траектория полностью рассчитана и имеет ограниченные габариты.";
  const count = Number(parsed[1]);
  return `${count} ${plural(count, "перемещение", "перемещения", "перемещений")} · ${parsed[2]}`;
}

function workCoordinateDetail(check: RunPreflightCheck): string {
  const active = check.detail.match(/G5[4-9]/)?.[0];
  return check.level === "pass" && active
    ? `${active} активна. Перед запуском останется подтвердить рабочий ноль.`
    : "Контроллер не сообщил активную систему G54-G59.";
}

function checkCertificateDetail(check: RunPreflightCheck): string {
  if (check.level === "pass") {
    const sequence = check.detail.match(/Check #(\d+)/)?.[1];
    return sequence
      ? `Проверка GRBL #${sequence} подтверждает этот файл и текущие параметры запуска.`
      : "GRBL подтвердил этот файл и текущие параметры запуска.";
  }
  if (check.detail.includes("expired")) {
    return "Предыдущая проверка устарела. Запустите проверку GRBL ещё раз.";
  }
  if (check.detail.includes("changed")) {
    return "После проверки изменился файл, режим запуска или сеанс контроллера. Повторите проверку.";
  }
  return "Нажмите «Проверить G-code через GRBL». Проверка проходит без движения станка.";
}

function heightmapDetail(check: RunPreflightCheck, report: RunPreflightReport): string {
  const mapId = report.executionOptions.surfaceMapId;
  if (check.level === "pass") {
    const blocks = check.detail.match(/; (\d+) compensated/)?.[1];
    return `Карта #${mapId ?? "?"} покрывает траекторию${blocks ? ` · подготовлено ${blocks} блоков` : ""}.`;
  }
  return "Карта высот не может быть безопасно применена к этой траектории. Проверьте её периметр и завершённость.";
}

export function presentPreflightCheck(
  check: RunPreflightCheck,
  report: RunPreflightReport,
): PresentedPreflightCheck {
  let title: string;
  let detail: string;

  switch (check.id) {
    case "controller-state":
      title = "Состояние контроллера";
      detail = check.level === "pass"
        ? `Подключён · Idle · актуальный статус #${report.pollSequence}`
        : "Для запуска контроллер должен быть подключён, не иметь Alarm и находиться в Idle.";
      break;
    case "motion-hardware":
      title = "Настройки движения";
      detail = check.level === "pass"
        ? "Прошивка, оси XYZ, единицы и режим фрезеровки согласованы."
        : "Проверка критичных настроек движения не пройдена. Откройте инспектор станка.";
      break;
    case "program-policy":
      title = report.intent === "cutting" ? "Допустимость гравировки" : "Безопасность прогона";
      detail = check.level === "pass"
        ? report.intent === "cutting"
          ? "Команды резания допустимы; опасные служебные команды остаются под контролем Millo."
          : "В файле нет команд, запрещённых для прогона без резания."
        : programBlockerDetail(report.programBlockers);
      break;
    case "program-modal-contract":
      title = "Режимы G-code";
      detail = modalContractDetail(check);
      break;
    case "program-geometry":
      title = "Траектория";
      detail = geometryDetail(check);
      break;
    case "work-coordinate-system":
      title = "Рабочая система координат";
      detail = workCoordinateDetail(check);
      break;
    case "unhomed-envelope":
      title = "Границы станка не подтверждены";
      detail = "Без homing и концевиков preview не гарантирует физический запас. Проверьте путь и крепёж.";
      break;
    case "manual-spindle":
      title = "Шпиндель включается вручную";
      detail = "Перед движением Millo отдельно попросит подтвердить, что шпиндель уже запущен.";
      break;
    case "operator-setup":
      title = "Физическая подготовка";
      detail = "Заготовка, фреза, ноль, безопасная Z и свободная траектория подтверждаются при запуске.";
      break;
    case "grbl-check-certificate":
      title = check.level === "pass" ? "Проверка GRBL пройдена" : "Нужна проверка GRBL";
      detail = checkCertificateDetail(check);
      break;
    case "heightmap-compensation":
      title = check.level === "pass" ? "Карта высот готова" : "Карта высот не применима";
      detail = heightmapDetail(check, report);
      break;
    default:
      title = check.title;
      detail = check.detail;
  }

  return { ...check, title, detail };
}

export function presentPreflightReport(report: RunPreflightReport): PresentedPreflightReport {
  const checks = report.checks.map((check) => presentPreflightCheck(check, report));
  const attention = checks.filter((check) => check.level !== "pass");
  const passed = checks.filter((check) => check.level === "pass");
  if (report.blockerCount > 0) {
    return {
      attention,
      passed,
      title: report.blockerCount === 1 ? "Остался один шаг" : "Нужны действия перед запуском",
      summary: `${report.blockerCount} ${plural(report.blockerCount, "препятствие", "препятствия", "препятствий")} · выполните отмеченные действия`,
    };
  }
  return {
    attention,
    passed,
    title: "Проверка пройдена",
    summary: report.cautionCount > 0
      ? `${report.cautionCount} ${plural(report.cautionCount, "напоминание", "напоминания", "напоминаний")} перед движением`
      : "Контроллер и программа готовы к подтверждению запуска",
  };
}
