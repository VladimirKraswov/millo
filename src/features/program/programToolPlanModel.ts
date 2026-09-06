import type { GcodeProgram } from "../../shared/program";

export function initialProgramToolNumber(
  program: Pick<GcodeProgram, "lines" | "document">,
): number | undefined {
  if (program.document) return program.document.initialToolNumber ?? undefined;
  let selectedTool: number | undefined;
  for (const line of program.lines) {
    if (!line.executable || line.blockDeleted) continue;
    const words = line.normalized.split(/\s+/);
    for (const word of words) {
      const match = /^T(\d{1,3})$/i.exec(word);
      if (!match) continue;
      const candidate = Number(match[1]);
      if (Number.isInteger(candidate) && candidate >= 0 && candidate <= 255) {
        selectedTool = candidate;
      }
    }
    if (words.some((word) => /^M0*6(?:\.0*)?$/i.test(word))) return selectedTool;
    if (words.some((word) => /^G0*[123](?:\.0*)?$/i.test(word))) return selectedTool;
  }
  return selectedTool;
}

export function programToolNumberAtSourceLine(
  program: Pick<GcodeProgram, "lines" | "document">,
  sourceLine: number | undefined,
): number | undefined {
  if (sourceLine === undefined) return initialProgramToolNumber(program);
  if (program.document) {
    if (sourceLine > program.document.toolSelectionCoverageLine) return undefined;
    const entries = program.document.toolSelections;
    let low = 0;
    let high = entries.length;
    while (low < high) {
      const mid = (low + high) >>> 1;
      if (entries[mid].sourceLine <= sourceLine) low = mid + 1;
      else high = mid;
    }
    return low > 0 ? entries[low - 1].tool ?? undefined : initialProgramToolNumber(program);
  }
  let selectedTool: number | undefined;
  for (const line of program.lines) {
    if (line.sourceLine > sourceLine) break;
    if (!line.executable || line.blockDeleted) continue;
    for (const word of line.normalized.split(/\s+/)) {
      const match = /^T(\d{1,3})$/i.exec(word);
      if (!match) continue;
      const candidate = Number(match[1]);
      if (Number.isInteger(candidate) && candidate >= 0 && candidate <= 255) {
        selectedTool = candidate;
      }
    }
  }
  return selectedTool ?? initialProgramToolNumber(program);
}
