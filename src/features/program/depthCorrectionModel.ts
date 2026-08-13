import type { GcodeProgram } from "../../shared/program";

export const maximumDepthAdjustmentMm = 10;

export interface DepthCorrectionView {
  readonly available: boolean;
  readonly enabled: boolean;
  readonly fileDepthMm?: number;
  readonly targetDepthMm?: number;
  readonly adjustmentMm: number;
  readonly minimumTargetMm?: number;
  readonly maximumTargetMm: number;
}

export function deepestCuttingZ(program: GcodeProgram | undefined): number | undefined {
  if (!program) return undefined;
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
  const fileDepthMm = deepestCuttingZ(program);
  const adjustmentMm = adjustmentUm === undefined ? 0 : adjustmentUm / 1_000;
  return {
    available: fileDepthMm !== undefined,
    enabled: adjustmentUm !== undefined,
    fileDepthMm,
    targetDepthMm: fileDepthMm === undefined ? undefined : fileDepthMm + adjustmentMm,
    adjustmentMm,
    minimumTargetMm:
      fileDepthMm === undefined ? undefined : fileDepthMm - maximumDepthAdjustmentMm,
    maximumTargetMm: 0,
  };
}

export function depthAdjustmentUmForTarget(
  fileDepthMm: number,
  targetDepthMm: number,
): number {
  if (!Number.isFinite(fileDepthMm) || fileDepthMm >= 0) {
    throw new Error("В файле нет рабочей глубины ниже Z0");
  }
  if (!Number.isFinite(targetDepthMm) || targetDepthMm > 0) {
    throw new Error("Итоговая глубина должна быть не выше Z0");
  }
  const adjustmentUm = Math.round((targetDepthMm - fileDepthMm) * 1_000);
  if (Math.abs(adjustmentUm) > maximumDepthAdjustmentMm * 1_000) {
    throw new Error("Коррекция глубины ограничена диапазоном ±10 мм");
  }
  return adjustmentUm;
}

export function adjustCuttingZ(
  z: number,
  kind: GcodeProgram["toolpath"][number]["kind"],
  adjustmentMm: number,
): number {
  return kind !== "rapid" && z < -1e-9 ? Math.min(0, z + adjustmentMm) : z;
}
