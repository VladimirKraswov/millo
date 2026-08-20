import type { OperatorConsoleCommandKind } from "../../shared/machine";

export interface SafeConsoleCommandDescriptor {
  readonly command: string;
  readonly kind: OperatorConsoleCommandKind;
  readonly label: string;
}

export const safeConsoleCommands: readonly SafeConsoleCommandDescriptor[] = [
  { command: "?", kind: "status", label: "Статус" },
  { command: "$I", kind: "buildInfo", label: "Прошивка" },
  { command: "$$", kind: "settings", label: "Настройки" },
  { command: "$G", kind: "modalState", label: "Режимы" },
  { command: "$#", kind: "parameters", label: "Координаты" },
];

const byCommand = new Map(
  safeConsoleCommands.map((descriptor) => [descriptor.command, descriptor]),
);

export const normalizeConsoleCommand = (input: string): string =>
  input.trim().toUpperCase();

export const normalizeSubmittedConsoleCommand = (
  input: string,
  safeCommandMode: boolean,
): string => {
  const command = input.trim();
  return safeCommandMode || safeConsoleCommand(command)
    ? normalizeConsoleCommand(command)
    : command;
};

export const safeConsoleCommand = (
  input: string,
): SafeConsoleCommandDescriptor | undefined =>
  byCommand.get(normalizeConsoleCommand(input));

export const validExpertConsoleCommand = (input: string): boolean => {
  const command = input.trim();
  return (
    command.length > 0 &&
    command.length <= 255 &&
    /^[\x20-\x7e]+$/.test(command) &&
    command !== "!" &&
    command !== "~"
  );
};

export const consoleCommandAllowed = (
  input: string,
  safeCommandMode: boolean,
): boolean =>
  safeConsoleCommand(input) !== undefined ||
  (!safeCommandMode && validExpertConsoleCommand(input));

export const consolePolicyMessage = (
  input: string,
  safeCommandMode = true,
): string => {
  const command = normalizeConsoleCommand(input);
  if (!command) return "Введите диагностический запрос";
  if (safeConsoleCommand(command)) return "Только чтение";
  if (!safeCommandMode && validExpertConsoleCommand(command)) {
    return "Экспертная команда будет выполнена через Rust actor";
  }
  if (!safeCommandMode && (command === "!" || command === "~")) {
    return "Используйте отдельные кнопки Hold, Resume или Reset";
  }
  return "Команда изменяет состояние или не входит в безопасный список";
};
