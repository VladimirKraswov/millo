import type { GcodeProgram, ProgramLine } from "../../shared/program";

interface ProgramSourceIndex {
  readonly lines: ReadonlyMap<number, ProgramLine>;
  readonly motions: ReadonlyMap<number, GcodeProgram["toolpath"]>;
}

const indexes = new WeakMap<GcodeProgram, ProgramSourceIndex>();

// Programs are immutable parse results. Live selection must not rescan a whole job.
export function programSourceIndex(program: GcodeProgram): ProgramSourceIndex {
  const cached = indexes.get(program);
  if (cached) return cached;
  const lines = new Map(program.lines.map((line) => [line.sourceLine, line]));
  const motions = new Map<number, GcodeProgram["toolpath"][number][]>();
  for (const segment of program.toolpath) {
    const group = motions.get(segment.sourceLine);
    if (group) group.push(segment);
    else motions.set(segment.sourceLine, [segment]);
  }
  const index = { lines, motions };
  indexes.set(program, index);
  return index;
}
