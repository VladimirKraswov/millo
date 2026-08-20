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

export const safeConsoleCommand = (
  input: string,
): SafeConsoleCommandDescriptor | undefined =>
  byCommand.get(normalizeConsoleCommand(input));

export const consolePolicyMessage = (input: string): string => {
  const command = normalizeConsoleCommand(input);
  if (!command) return "Введите диагностический запрос";
  if (safeConsoleCommand(command)) return "Только чтение";
  return "Команда изменяет состояние или не входит в безопасный список";
};
