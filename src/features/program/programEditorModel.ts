import type { GcodeProgram } from "../../shared/program";

export const PROGRAM_EDITOR_HISTORY_LIMIT = 200;

export interface ProgramEditorHistory {
  readonly source: string;
  readonly past: readonly string[];
  readonly future: readonly string[];
}

export interface TextSelection {
  readonly start: number;
  readonly end: number;
}

export interface TextEdit {
  readonly source: string;
  readonly selection: TextSelection;
}

export type GcodeSyntaxKind =
  | "plain"
  | "comment"
  | "command"
  | "line-number"
  | "axis"
  | "arc"
  | "parameter"
  | "optional"
  | "checksum"
  | "number"
  | "invalid";

export interface GcodeSyntaxToken {
  readonly kind: GcodeSyntaxKind;
  readonly text: string;
}

export const createProgramEditorHistory = (source: string): ProgramEditorHistory => ({
  source,
  past: [],
  future: [],
});

export function commitProgramEditorSource(
  history: ProgramEditorHistory,
  source: string,
): ProgramEditorHistory {
  if (source === history.source) return history;
  return {
    source,
    past: [...history.past, history.source].slice(-PROGRAM_EDITOR_HISTORY_LIMIT),
    future: [],
  };
}

export function undoProgramEditorSource(
  history: ProgramEditorHistory,
): ProgramEditorHistory {
  const source = history.past.at(-1);
  if (source === undefined) return history;
  return {
    source,
    past: history.past.slice(0, -1),
    future: [history.source, ...history.future],
  };
}

export function redoProgramEditorSource(
  history: ProgramEditorHistory,
): ProgramEditorHistory {
  const source = history.future[0];
  if (source === undefined) return history;
  return {
    source,
    past: [...history.past, history.source].slice(-PROGRAM_EDITOR_HISTORY_LIMIT),
    future: history.future.slice(1),
  };
}

const clampOffset = (source: string, offset: number): number =>
  Math.max(0, Math.min(source.length, Number.isFinite(offset) ? offset : 0));

export function normalizeTextSelection(
  source: string,
  selection: TextSelection,
): TextSelection {
  const start = clampOffset(source, selection.start);
  const end = clampOffset(source, selection.end);
  return start <= end ? { start, end } : { start: end, end: start };
}

export function sourceLineAtOffset(source: string, offset: number): number {
  const end = clampOffset(source, offset);
  let line = 1;
  for (let index = 0; index < end; index += 1) {
    if (source[index] === "\n") line += 1;
  }
  return line;
}

export function sourceOffsetAtLine(source: string, sourceLine: number): number {
  const target = Math.max(1, Math.floor(sourceLine));
  if (target === 1) return 0;
  let line = 1;
  for (let index = 0; index < source.length; index += 1) {
    if (source[index] !== "\n") continue;
    line += 1;
    if (line === target) return index + 1;
  }
  return source.length;
}

export function selectedLineSpan(
  source: string,
  selection: TextSelection,
): TextSelection {
  const normalized = normalizeTextSelection(source, selection);
  const startBreak =
    normalized.start === 0 ? -1 : source.lastIndexOf("\n", normalized.start - 1);
  const start = startBreak < 0 ? 0 : startBreak + 1;
  const inclusiveEnd =
    normalized.end > normalized.start && source[normalized.end - 1] === "\n"
      ? normalized.end - 1
      : normalized.end;
  const endBreak = source.indexOf("\n", inclusiveEnd);
  return { start, end: endBreak < 0 ? source.length : endBreak + 1 };
}

export function selectedTextOrLines(
  source: string,
  selection: TextSelection,
): { readonly text: string; readonly selection: TextSelection } {
  const normalized = normalizeTextSelection(source, selection);
  const effective =
    normalized.start === normalized.end
      ? selectedLineSpan(source, normalized)
      : normalized;
  return {
    text: source.slice(effective.start, effective.end),
    selection: effective,
  };
}

export function replaceTextSelection(
  source: string,
  selection: TextSelection,
  replacement: string,
): TextEdit {
  const normalized = normalizeTextSelection(source, selection);
  const nextOffset = normalized.start + replacement.length;
  return {
    source:
      source.slice(0, normalized.start) + replacement + source.slice(normalized.end),
    selection: { start: nextOffset, end: nextOffset },
  };
}

export function insertProgramLine(
  source: string,
  selection: TextSelection,
): TextEdit {
  const span = selectedLineSpan(source, selection);
  return {
    source: `${source.slice(0, span.start)}\n${source.slice(span.start)}`,
    selection: { start: span.start, end: span.start },
  };
}

export function deleteProgramLines(
  source: string,
  selection: TextSelection,
): TextEdit {
  const span = selectedLineSpan(source, selection);
  return {
    source: source.slice(0, span.start) + source.slice(span.end),
    selection: { start: span.start, end: span.start },
  };
}

const numberPattern = /^[+-]?(?:(?:\d+(?:\.\d*)?)|(?:\.\d+))/;

const syntaxKindForWord = (letter: string): GcodeSyntaxKind => {
  if (letter === "G" || letter === "M") return "command";
  if (letter === "N" || letter === "O") return "line-number";
  if ("XYZABC".includes(letter)) return "axis";
  if ("IJKR".includes(letter)) return "arc";
  if ("FSTPLHDQ".includes(letter)) return "parameter";
  return "invalid";
};

export function tokenizeGcodeLine(line: string): readonly GcodeSyntaxToken[] {
  const tokens: GcodeSyntaxToken[] = [];
  let index = 0;
  while (index < line.length) {
    const character = line[index];
    if (character === ";") {
      tokens.push({ kind: "comment", text: line.slice(index) });
      break;
    }
    if (character === "(") {
      const close = line.indexOf(")", index + 1);
      const end = close < 0 ? line.length : close + 1;
      tokens.push({ kind: "comment", text: line.slice(index, end) });
      index = end;
      continue;
    }
    if (/\s/.test(character)) {
      let end = index + 1;
      while (end < line.length && /\s/.test(line[end])) end += 1;
      tokens.push({ kind: "plain", text: line.slice(index, end) });
      index = end;
      continue;
    }
    if (character === "/") {
      tokens.push({ kind: "optional", text: character });
      index += 1;
      continue;
    }
    if (character === "%") {
      tokens.push({ kind: "line-number", text: character });
      index += 1;
      continue;
    }
    if (character === "*") {
      const checksum = line.slice(index + 1).match(/^\d+/)?.[0] ?? "";
      tokens.push({ kind: "checksum", text: `*${checksum}` });
      index += checksum.length + 1;
      continue;
    }
    if (/[A-Za-z]/.test(character)) {
      let valueStart = index + 1;
      while (valueStart < line.length && /\s/.test(line[valueStart])) valueStart += 1;
      const value = line.slice(valueStart).match(numberPattern)?.[0];
      const end = value === undefined ? index + 1 : valueStart + value.length;
      tokens.push({
        kind: syntaxKindForWord(character.toUpperCase()),
        text: line.slice(index, end),
      });
      index = end;
      continue;
    }
    const number = line.slice(index).match(numberPattern)?.[0];
    if (number !== undefined) {
      tokens.push({ kind: "number", text: number });
      index += number.length;
      continue;
    }
    tokens.push({ kind: "invalid", text: character });
    index += 1;
  }
  return tokens;
}

export function buildProcessedProgramSource(program: GcodeProgram): string {
  const lines = program.lines
    .filter((line) => !line.blockDeleted && line.normalized.length > 0)
    .map((line) => line.normalized);
  return lines.length === 0 ? "" : `${lines.join("\n")}\n`;
}

export function processedProgramName(sourceName: string): string {
  const dot = sourceName.lastIndexOf(".");
  if (dot <= 0) return `${sourceName || "program"}-transformed.nc`;
  return `${sourceName.slice(0, dot)}-transformed${sourceName.slice(dot)}`;
}
