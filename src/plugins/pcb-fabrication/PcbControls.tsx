import type { ReactNode } from "react";

import type { PcbInspection } from "../../shared/jobs";
import type { CuttingTool } from "../../shared/tooling";

export function OperationRow({
  children,
  enabled,
  label,
  onToggle,
  summary,
}: {
  readonly children: ReactNode;
  readonly enabled: boolean;
  readonly label: string;
  readonly onToggle: (value: boolean) => void;
  readonly summary: string;
}) {
  return <fieldset className={enabled ? "is-enabled" : ""}>
    <label className="pcb-operation-toggle">
      <input checked={enabled} onChange={(event) => onToggle(event.target.checked)} type="checkbox" />
      <span><strong>{label}</strong><small>{summary}</small></span>
    </label>
    {enabled && <div className="pcb-operation-fields">{children}</div>}
  </fieldset>;
}

export function ToolSelect({
  label,
  onChange,
  tools,
  value,
}: {
  readonly label: string;
  readonly onChange: (value: string) => void;
  readonly tools: readonly CuttingTool[];
  readonly value: string;
}) {
  return <label className="pcb-field pcb-tool-select">
    <span>{label}</span>
    <select onChange={(event) => onChange(event.target.value)} value={value}>
      <option value="">Выберите</option>
      {tools.map((tool) => <option key={tool.id} value={tool.id}>{tool.name} · {tool.tipDiameterMm !== undefined ? `кончик ${formatPcbNumber(tool.tipDiameterMm)}` : `Ø${formatPcbNumber(tool.diameterMm)}`}</option>)}
    </select>
  </label>;
}

export function DrillGroupToolSelect({
  group,
  onChange,
  tools,
  value,
}: {
  readonly group: PcbInspection["drillGroups"][number];
  readonly onChange: (value: string) => void;
  readonly tools: readonly CuttingTool[];
  readonly value: string;
}) {
  const selected = tools.find((tool) => tool.id === value);
  const diameterDifference = selected ? Math.abs(selected.diameterMm - group.diameterMm) : undefined;
  const exactMatch = diameterDifference !== undefined && diameterDifference <= 0.01;
  const featureCount = group.hitCount + group.slotCount;
  return <div className="pcb-drill-group">
    <div className="pcb-drill-group-title">
      <strong>Ø{formatPcbNumber(group.diameterMm)} mm</strong>
      <span>T{group.sourceToolNumber} · {group.hitCount} отв.{group.slotCount ? ` · ${group.slotCount} паз.` : ""}</span>
    </div>
    <ToolSelect label={group.slotCount ? "Фреза для отверстий и пазов" : "Сверло"} onChange={onChange} tools={tools} value={value} />
    <small className={exactMatch ? "is-match" : selected ? "is-warning" : ""}>
      {exactMatch
        ? `Диаметр совпадает · ${featureCount} элем.`
        : selected
          ? `Выбрано Ø${formatPcbNumber(selected.diameterMm)} mm · разница ${formatPcbNumber(diameterDifference!)} mm`
          : group.slotCount
            ? `Нужна концевая фреза не шире ${formatPcbNumber(group.diameterMm)} mm`
            : "Сверло не выбрано"}
    </small>
  </div>;
}

export function NumberField({
  label,
  max = 100_000,
  min = -100_000,
  onChange,
  step,
  suffix = "mm",
  value,
}: {
  readonly label: string;
  readonly max?: number;
  readonly min?: number;
  readonly onChange: (value: number) => void;
  readonly step: number;
  readonly suffix?: string;
  readonly value: number;
}) {
  return <label className="pcb-field pcb-number">
    <span>{label}</span>
    <div>
      <input max={max} min={min} onChange={(event) => {
        if (Number.isFinite(event.target.valueAsNumber)) onChange(event.target.valueAsNumber);
      }} step={step} type="number" value={value} />
      <small>{suffix}</small>
    </div>
  </label>;
}

export const formatPcbNumber = (value: number) => Number(value.toFixed(3)).toString();
