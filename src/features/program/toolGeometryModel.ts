import type { CuttingTool, ToolKind } from "../../shared/tooling";

export interface ToolRenderProfile {
  readonly kind: ToolKind;
  readonly diameterMm: number;
  readonly tipDiameterMm: number;
  readonly shankDiameterMm: number;
  readonly cuttingLengthMm: number;
  readonly shankLengthMm: number;
  readonly fluteCount: number;
  readonly includedAngleDegrees?: number;
  readonly angularSpeedRadPerSecond: number;
}

const clamp = (value: number, minimum: number, maximum: number): number =>
  Math.min(maximum, Math.max(minimum, value));

export function toolRenderProfile(
  tool: CuttingTool | undefined,
  gridSizeMm: number,
): ToolRenderProfile {
  const span = Math.max(10, gridSizeMm);
  const diameter = tool?.diameterMm ?? 3.175;
  const maximumDiameter = Math.max(6, span * 0.45);
  const diameterMm = clamp(diameter, Math.max(0.08, span * 0.002), maximumDiameter);
  const shankDiameterMm = clamp(
    tool?.shankDiameterMm ?? 3.175,
    Math.max(0.1, span * 0.002),
    maximumDiameter,
  );
  const tipRatio = clamp((tool?.tipDiameterMm ?? diameter) / diameter, 0.001, 1);
  const cuttingLengthMm = clamp(
    tool?.cuttingLengthMm ?? 10,
    Math.max(2, diameterMm * 0.35),
    Math.max(8, Math.min(40, span * 0.35)),
  );

  return {
    kind: tool?.kind ?? "flatEndMill",
    diameterMm,
    tipDiameterMm: clamp(diameterMm * tipRatio, 0.02, diameterMm),
    shankDiameterMm,
    cuttingLengthMm,
    shankLengthMm: Math.max(4, Math.min(14, span * 0.12)),
    fluteCount: clamp(Math.round(tool?.fluteCount ?? 2), 1, 8),
    includedAngleDegrees: tool?.includedAngleDegrees,
    angularSpeedRadPerSecond: 6 + Math.min(18, (tool?.spindleRpm ?? 10_000) / 1_000),
  };
}
