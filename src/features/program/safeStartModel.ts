export function suggestedSafeZ(maximumProgramZ: number | undefined): number {
  const baseline = Number.isFinite(maximumProgramZ) ? (maximumProgramZ ?? 0) : 0;
  return Math.ceil((baseline + 2) * 10) / 10;
}

export function canPrepareSafeStart(input: {
  readonly busy: boolean;
  readonly minimumSafeZ: number;
  readonly motionCount: number;
  readonly safeZ: number;
  readonly sourceLine?: number;
}): boolean {
  return (
    !input.busy &&
    input.sourceLine !== undefined &&
    input.motionCount > 0 &&
    Number.isFinite(input.safeZ) &&
    input.safeZ >= input.minimumSafeZ
  );
}
