import type { GcodeProgram } from "../../shared/program";

export const maximumDepthAdjustmentMm = 10;

export interface DepthCorrectionView {
  readonly available: boolean;
  readonly enabled: boolean;
  readonly adjustmentMm: number;
  readonly minimumAdjustmentMm: number;
  readonly maximumAdjustmentMm: number;
}

export function deepestCuttingZ(program: GcodeProgram | undefined): number | undefined {
  if (!program) return undefined;
  if (program.document) return program.document.deepestCuttingZ ?? undefined;
  let deepest: number | undefined;
  for (const segment of program.toolpath) {
    if (segment.kind === "rapid") continue;
    for (const point of segment.points) {
      if (point.z >= -1e-9) continue;
      deepest = deepest === undefined ? point.z : Math.min(deepest, point.z);
    }
  }
  return deepest;
}

export function depthCorrectionView(
  program: GcodeProgram | undefined,
  adjustmentUm: number | undefined,
): DepthCorrectionView {
  const available = deepestCuttingZ(program) !== undefined;
  const adjustmentMm = adjustmentUm === undefined ? 0 : adjustmentUm / 1_000;
  return {
    available,
    enabled: adjustmentUm !== undefined,
    adjustmentMm,
    minimumAdjustmentMm: -maximumDepthAdjustmentMm,
    maximumAdjustmentMm: maximumDepthAdjustmentMm,
  };
}

export function depthAdjustmentUm(adjustmentMm: number): number {
  if (!Number.isFinite(adjustmentMm)) {
    throw new Error("Смещение глубины должно быть числом");
  }
  const adjustmentUm = Math.round(adjustmentMm * 1_000);
  if (Math.abs(adjustmentUm) > maximumDepthAdjustmentMm * 1_000) {
    throw new Error("Коррекция глубины ограничена диапазоном ±10 мм");
  }
  return adjustmentUm;
}

export function depthAdjustmentFromDraft(draft: string): number | undefined {
  if (draft.trim() === "") return undefined;
  const adjustmentMm = Number(draft);
  return Number.isFinite(adjustmentMm) ? adjustmentMm : undefined;
}

export function adjustCuttingZ(
  z: number,
  kind: GcodeProgram["toolpath"][number]["kind"],
  adjustmentMm: number,
): number {
  return kind !== "rapid" && z < -1e-9 ? z + adjustmentMm : z;
}
