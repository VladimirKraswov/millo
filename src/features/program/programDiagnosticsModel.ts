import type {
  GcodeProgram,
  ProgramWarning,
  ProgramWarningCode,
  ProgramWarningSeverity,
} from "../../shared/program";

export interface ProgramDiagnosticsSummary {
  readonly actionableCount: number;
  readonly managedToolChangeCount: number;
  readonly totalCount: number;
}

export interface ProgramWarningPresentation {
  readonly detail: string;
  readonly kind: ProgramWarningSeverity | "managed";
  readonly title: string;
}

const warningTitles: Readonly<Record<ProgramWarningCode, string>> = {
  "unclosed-comment": "Незакрытый комментарий",
  "unexpected-comment-close": "Лишнее закрытие комментария",
  "invalid-token": "Некорректная команда",
  "duplicate-word": "Повтор параметра",
  "optional-block": "Опциональная строка",
  "optional-block-unsupported": "Неподдерживаемая опциональная строка",
  "checksum-validated": "Контрольная сумма",
  "checksum-invalid": "Ошибка контрольной суммы",
  "checksum-unsupported": "Неподдерживаемая контрольная сумма",
  "unsupported-g-code": "Неподдерживаемая G-команда",
  "unsupported-m-code": "Неподдерживаемая M-команда",
  "unsupported-word": "Неподдерживаемый параметр",
  "unsupported-plane": "Неподдерживаемая плоскость",
  "coordinate-system-ignored": "Система координат не учтена",
  "unsafe-machine-command": "Опасная команда станка",
  "spindle-activation": "Запуск шпинделя",
  "spindle-speed": "Обороты шпинделя",
  "tool-change": "Смена инструмента",
  "arc-definition": "Некорректная дуга",
  "dwell-definition": "Некорректная выдержка",
  "feed-rate": "Некорректная подача",
  "modal-group-conflict": "Конфликт режимов",
  "preview-limit": "Лимит предпросмотра",
  "rotary-timing-unavailable": "Время движения A приблизительное",
};

export function isManagedToolChange(warning: ProgramWarning): boolean {
  return warning.code === "tool-change";
}

export function hasActionableProgramWarnings(program: GcodeProgram): boolean {
  if (program.document) return program.document.warningCount > program.document.managedToolChangeCount;
  return program.warnings.some((warning) => !isManagedToolChange(warning));
}

export function programCanEnterPreflight(program: GcodeProgram): boolean {
  return program.summary.previewComplete &&
    (program.document?.errorCount ?? 0) === 0 &&
    !program.warnings.some((warning) => warning.severity === "error");
}

export function programDiagnosticsSummary(
  program: GcodeProgram,
): ProgramDiagnosticsSummary {
  const managedToolChangeCount = program.document?.managedToolChangeCount ?? program.warnings.filter(isManagedToolChange).length;
  const totalCount = program.document?.warningCount ?? program.warnings.length;
  return {
    actionableCount: totalCount - managedToolChangeCount,
    managedToolChangeCount,
    totalCount,
  };
}

export function formatProgramDiagnostics(
  summary: ProgramDiagnosticsSummary,
): string {
  const parts: string[] = [];
  if (summary.actionableCount > 0) {
    parts.push(`${summary.actionableCount} ${plural(
      summary.actionableCount,
      "замечание",
      "замечания",
      "замечаний",
    )}`);
  }
  if (summary.managedToolChangeCount > 0) {
    parts.push(`${summary.managedToolChangeCount} ${plural(
      summary.managedToolChangeCount,
      "смена инструмента",
      "смены инструмента",
      "смен инструмента",
    )}`);
  }
  return parts.join(" · ");
}

export function programWarningPresentation(
  warning: ProgramWarning,
): ProgramWarningPresentation {
  if (isManagedToolChange(warning)) {
    return {
      detail: "Millo остановится перед этой строкой, предложит заменить инструмент и продолжит только после подтверждения. M6 не отправляется в GRBL.",
      kind: "managed",
      title: warningTitles[warning.code],
    };
  }
  return {
    detail: warning.message,
    kind: warning.severity,
    title: warningTitles[warning.code],
  };
}

function plural(
  value: number,
  one: string,
  few: string,
  many: string,
): string {
  const modulo100 = Math.abs(value) % 100;
  const modulo10 = modulo100 % 10;
  if (modulo100 >= 11 && modulo100 <= 19) return many;
  if (modulo10 === 1) return one;
  if (modulo10 >= 2 && modulo10 <= 4) return few;
  return many;
}
