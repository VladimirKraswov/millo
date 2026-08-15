import type { PcbCopperAnalysis } from "../../shared/jobs";
import {
  effectiveCuttingDiameterMm,
  type CuttingTool,
} from "../../shared/tooling";

const SUBSTRATE_BITE_MM = 0.015;
const MINIMUM_EDGE_RESERVE_MM = 0.01;
const MINIMUM_USEFUL_DEPTH_MARGIN_MM = 0.005;

export interface IsolationRecommendation {
  readonly tool: CuttingTool;
  readonly depthMm: number;
  readonly copperThicknessMm: number;
  readonly clearanceMm: number;
  readonly passes: number;
  readonly feedMmPerMin: number;
  readonly plungeMmPerMin: number;
  readonly spindleRpm: number;
  readonly effectiveDiameterMm?: number;
  readonly minimumGapMm?: number;
  readonly warning?: string;
}

export const recommendIsolation = (
  tools: readonly CuttingTool[],
  copper: PcbCopperAnalysis,
  copperThicknessMm = 0.035,
): IsolationRecommendation | undefined => {
  const candidates = tools
    .filter(isIsolationTool)
    .map((tool) => recommendIsolationForTool(tool, copper, copperThicknessMm))
    .filter((candidate): candidate is IsolationRecommendation => candidate !== undefined)
    .sort((left, right) =>
      Number(Boolean(left.warning)) - Number(Boolean(right.warning))
      || (left.effectiveDiameterMm ?? Infinity) - (right.effectiveDiameterMm ?? Infinity)
      || toolPreference(left.tool) - toolPreference(right.tool));
  return candidates[0];
};

export const recommendIsolationForTool = (
  tool: CuttingTool,
  copper: PcbCopperAnalysis,
  copperThicknessMm = 0.035,
): IsolationRecommendation | undefined => {
  if (!isIsolationTool(tool)) return undefined;
  if (tool.includedAngleDegrees !== undefined && tool.tipDiameterMm === undefined) return undefined;
  const requiredDepthMm = roundUp(
    Math.max(0.04, copperThicknessMm + SUBSTRATE_BITE_MM),
    0.005,
  );
  const minimumGapMm = copper.minimumIsolationGapMm;
  const maximumDiameterMm = minimumGapMm === undefined
    ? undefined
    : Math.max(0, minimumGapMm - 2 * MINIMUM_EDGE_RESERVE_MM);
  const maximumDepthMm = maximumDiameterMm === undefined
    ? undefined
    : maximumDepthForDiameter(tool, maximumDiameterMm);
  const constrainedDepthMm = maximumDepthMm === undefined
    ? requiredDepthMm
    : Math.min(requiredDepthMm, roundDown(maximumDepthMm, 0.005));
  const cannotCutThrough = constrainedDepthMm < copperThicknessMm + MINIMUM_USEFUL_DEPTH_MARGIN_MM;
  const depthMm = cannotCutThrough ? requiredDepthMm : constrainedDepthMm;
  const effectiveDiameterMm = effectiveCuttingDiameterMm(tool, depthMm);
  if (effectiveDiameterMm === undefined) return undefined;

  const tooWide = maximumDiameterMm !== undefined && effectiveDiameterMm > maximumDiameterMm + 1e-6;
  const remainingGap = minimumGapMm === undefined ? undefined : minimumGapMm - effectiveDiameterMm;
  const clearanceMm = remainingGap === undefined
    ? 0.05
    : clamp(roundDown(remainingGap * 0.4, 0.005), 0.005, 0.05);
  const warning = cannotCutThrough
    ? `Для меди ${format(copperThicknessMm)} мм выбранная фреза не помещается в промежуток ${format(minimumGapMm!)} мм`
    : tooWide
      ? `Расчётная канавка ${format(effectiveDiameterMm)} мм шире доступного промежутка ${format(minimumGapMm!)} мм`
      : undefined;

  return {
    tool,
    depthMm,
    copperThicknessMm,
    clearanceMm,
    passes: 1,
    feedMmPerMin: tool.feedMmPerMin,
    plungeMmPerMin: Math.min(tool.plungeMmPerMin, tool.feedMmPerMin * 0.3),
    spindleRpm: tool.spindleRpm,
    effectiveDiameterMm,
    minimumGapMm,
    warning,
  };
};

export const isolationToolGeometryWarning = (tool: CuttingTool): string | undefined => {
  if (tool.tipDiameterMm !== undefined && tool.includedAngleDegrees === undefined) {
    return "У этой конической фрезы не указан угол. Откройте библиотеку инструментов и уточните маркировку колпачка";
  }
  if (tool.includedAngleDegrees !== undefined && tool.tipDiameterMm === undefined) {
    return "У этой V-фрезы не указан диаметр кончика. Уточните его в библиотеке инструментов";
  }
  return undefined;
};

const isIsolationTool = (tool: CuttingTool): boolean =>
  tool.kind === "engraving" || tool.kind === "vBit" || tool.kind === "flatEndMill";

const maximumDepthForDiameter = (tool: CuttingTool, maximumDiameterMm: number): number | undefined => {
  if (tool.includedAngleDegrees === undefined) {
    return tool.tipDiameterMm === undefined && tool.diameterMm <= maximumDiameterMm
      ? tool.cuttingLengthMm
      : undefined;
  }
  const tip = tool.tipDiameterMm;
  if (tip === undefined || maximumDiameterMm < tip) return 0;
  return (maximumDiameterMm - tip)
    / (2 * Math.tan(tool.includedAngleDegrees * Math.PI / 360));
};

const toolPreference = (tool: CuttingTool): number =>
  tool.kind === "engraving" ? 0 : tool.kind === "vBit" ? 1 : 2;

const roundUp = (value: number, step: number): number => Math.ceil(value / step - 1e-9) * step;
const roundDown = (value: number, step: number): number => Math.max(0, Math.floor(value / step + 1e-9) * step);
const clamp = (value: number, min: number, max: number): number => Math.min(max, Math.max(min, value));
const format = (value: number): string => Number(value.toFixed(3)).toString();
