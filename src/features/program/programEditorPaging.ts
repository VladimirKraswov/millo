import type { TextEdit, TextSelection } from "./programEditorModel";

export const PROGRAM_EDITOR_PAGE_LINES = 5_000;

export interface ProgramEditorPage {
  readonly firstLine: number;
  readonly lastLine: number;
  readonly start: number;
  readonly end: number;
  readonly text: string;
  readonly offsets: Uint32Array;
  readonly lineEnding: string;
}

export function buildProgramEditorLineIndex(source: string): Uint32Array {
  let count = 1;
  for (let offset = source.indexOf("\n"); offset !== -1; offset = source.indexOf("\n", offset + 1)) count += 1;
  const offsets = new Uint32Array(count);
  let line = 1;
  for (let offset = source.indexOf("\n"); offset !== -1; offset = source.indexOf("\n", offset + 1)) offsets[line++] = offset + 1;
  return offsets;
}

export function indexedSourceLine(offsets: Uint32Array, offset: number): number {
  let low = 0;
  let high = offsets.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (offsets[middle] <= offset) low = middle + 1;
    else high = middle;
  }
  return Math.max(1, low);
}

export function programEditorPageAtLine(
  source: string, offsets: Uint32Array, sourceLine: number,
): ProgramEditorPage {
  const target = Math.max(1, Math.min(offsets.length, Math.floor(sourceLine) || 1));
  const firstLine = Math.floor((target - 1) / PROGRAM_EDITOR_PAGE_LINES) * PROGRAM_EDITOR_PAGE_LINES + 1;
  const lastLine = Math.min(offsets.length, firstLine + PROGRAM_EDITOR_PAGE_LINES - 1);
  const start = offsets[firstLine - 1];
  let end = lastLine < offsets.length ? offsets[lastLine] - 1 : source.length;
  if (lastLine < offsets.length && source[end - 1] === "\r") end -= 1;
  const raw = source.slice(start, end);
  const text = raw.replace(/\r\n/g, "\n");
  const firstBreak = source.indexOf("\n", start);
  return {
    firstLine, lastLine, start, end, text, offsets: buildProgramEditorLineIndex(text),
    lineEnding: firstBreak > 0 && source[firstBreak - 1] === "\r" ? "\r\n" : "\n",
  };
}

export function pageOffsetToSource(page: ProgramEditorPage, offsets: Uint32Array, offset: number): number {
  const local = Math.max(0, Math.min(page.text.length, offset));
  const line = indexedSourceLine(page.offsets, local) - 1;
  return offsets[page.firstLine - 1 + line] + local - page.offsets[line];
}

export function sourceOffsetToPage(page: ProgramEditorPage, offsets: Uint32Array, offset: number): number {
  const global = Math.max(page.start, Math.min(page.end, offset));
  const line = indexedSourceLine(offsets, global) - page.firstLine;
  return Math.min(page.text.length, page.offsets[line] + global - offsets[page.firstLine - 1 + line]);
}

export function textChange(before: string, after: string) {
  let start = 0;
  const length = Math.min(before.length, after.length);
  while (start < length && before.charCodeAt(start) === after.charCodeAt(start)) start += 1;
  let end = before.length;
  let nextEnd = after.length;
  while (end > start && nextEnd > start && before.charCodeAt(end - 1) === after.charCodeAt(nextEnd - 1)) {
    end -= 1; nextEnd -= 1;
  }
  return { start, end, replacement: after.slice(start, nextEnd) };
}

export function editProgramEditorPage(
  source: string, offsets: Uint32Array, page: ProgramEditorPage,
  nextText: string, selection: TextSelection,
): TextEdit {
  const local = textChange(page.text, nextText);
  const start = pageOffsetToSource(page, offsets, local.start);
  const end = pageOffsetToSource(page, offsets, local.end);
  const replacement = local.replacement.replace(/\n/g, page.lineEnding);
  const selectionOffset = (offset: number) => {
    if (offset <= local.start) return pageOffsetToSource(page, offsets, offset);
    const nextEnd = local.start + local.replacement.length;
    if (offset >= nextEnd) {
      return pageOffsetToSource(page, offsets, offset - local.replacement.length + local.end - local.start)
        + replacement.length - (end - start);
    }
    return start + local.replacement.slice(0, offset - local.start).replace(/\n/g, page.lineEnding).length;
  };
  return {
    source: source.slice(0, start) + replacement + source.slice(end),
    selection: { ...selection, start: selectionOffset(selection.start), end: selectionOffset(selection.end) },
    change: { start, end, replacement },
  };
}
