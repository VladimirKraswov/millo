import type { ProgramLine } from "../../shared/program";

export const PROGRAM_LINE_ROW_HEIGHT = 34;
const PROGRAM_LINE_OVERSCAN = 4;

export interface ProgramLineWindow {
  readonly startIndex: number;
  readonly endIndex: number;
  readonly offsetPx: number;
  readonly totalHeightPx: number;
  readonly lines: readonly ProgramLine[];
}

export function buildProgramLineWindow(
  lines: readonly ProgramLine[],
  scrollTop: number,
  viewportHeight: number,
): ProgramLineWindow {
  const safeScrollTop = Number.isFinite(scrollTop) ? Math.max(0, scrollTop) : 0;
  const safeViewportHeight = Number.isFinite(viewportHeight)
    ? Math.max(PROGRAM_LINE_ROW_HEIGHT, viewportHeight)
    : PROGRAM_LINE_ROW_HEIGHT;
  const firstVisible = Math.min(
    lines.length,
    Math.floor(safeScrollTop / PROGRAM_LINE_ROW_HEIGHT),
  );
  const visibleCount = Math.ceil(safeViewportHeight / PROGRAM_LINE_ROW_HEIGHT);
  const startIndex = Math.max(0, firstVisible - PROGRAM_LINE_OVERSCAN);
  const endIndex = Math.min(
    lines.length,
    firstVisible + visibleCount + PROGRAM_LINE_OVERSCAN,
  );

  return {
    startIndex,
    endIndex,
    offsetPx: startIndex * PROGRAM_LINE_ROW_HEIGHT,
    totalHeightPx: lines.length * PROGRAM_LINE_ROW_HEIGHT,
    lines: lines.slice(startIndex, endIndex),
  };
}

export function findProgramLineIndex(
  lines: readonly ProgramLine[],
  sourceLine: number | undefined,
): number | undefined {
  if (sourceLine === undefined) return undefined;
  let low = 0;
  let high = lines.length - 1;
  while (low <= high) {
    const middle = Math.floor((low + high) / 2);
    const candidate = lines[middle].sourceLine;
    if (candidate === sourceLine) return middle;
    if (candidate < sourceLine) low = middle + 1;
    else high = middle - 1;
  }
  return undefined;
}

