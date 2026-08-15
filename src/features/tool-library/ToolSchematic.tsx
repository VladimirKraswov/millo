import type { CSSProperties } from "react";

import type { CuttingToolDraft } from "../../shared/tooling";
import { toolKindLabels } from "../../shared/tooling";

interface ToolSchematicProps {
  readonly tool: Pick<
    CuttingToolDraft,
    "kind" | "diameterMm" | "tipDiameterMm" | "shankDiameterMm" | "fluteCount" | "includedAngleDegrees"
  >;
  readonly compact?: boolean;
}

export function ToolSchematic({ tool, compact = false }: ToolSchematicProps) {
  const shankWidth = compact ? 10 : 18;
  const cutterWidth = Math.max(
    compact ? 8 : 14,
    Math.min(compact ? 34 : 58, shankWidth * tool.diameterMm / tool.shankDiameterMm),
  );
  const style = {
    "--tool-shank-width": `${shankWidth}px`,
    "--tool-cutter-width": `${cutterWidth}px`,
    "--tool-flute-count": tool.fluteCount,
  } as CSSProperties;
  const angle = tool.includedAngleDegrees
    ? `, угол ${tool.includedAngleDegrees}°`
    : "";
  const tip = tool.tipDiameterMm !== undefined
    ? `, кончик ${tool.tipDiameterMm} мм`
    : "";
  return (
    <div
      aria-label={`Схема: ${toolKindLabels[tool.kind]}, диаметр ${tool.diameterMm} мм${tip}${angle}`}
      className={`tool-schematic is-${tool.kind}${compact ? " is-compact" : ""}`}
      role="img"
      style={style}
    >
      <span className="tool-schematic-shank" />
      <span className="tool-schematic-neck" />
      <span className="tool-schematic-cutter" />
      <span className="tool-schematic-axis" />
    </div>
  );
}
