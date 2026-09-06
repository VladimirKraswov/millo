import { useEffect, useState } from "react";

export function SketchNumber({
  label,
  value,
  onChange,
  min = 0,
  max = 10_000,
  step = 0.1,
  unit = "мм",
  disabled = false,
}: {
  readonly label: string;
  readonly value: number;
  readonly onChange: (v: number) => void;
  readonly min?: number;
  readonly max?: number;
  readonly step?: number;
  readonly unit?: string;
  readonly disabled?: boolean;
}) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  const commit = () => {
    if (disabled) return;
    const parsed = Number(text);
    if (text.trim() && Number.isFinite(parsed)) {
      const next = Math.min(max, Math.max(min, parsed));
      setText(String(next));
      if (next !== value) onChange(next);
    } else setText(String(value));
  };
  return (
    <label className="sketch-number">
      <span>{label}</span>
      <div>
        <input
          aria-label={label}
          type="number"
          value={text}
          min={min}
          max={max}
          step={step}
          disabled={disabled}
          onChange={(e) => setText(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.currentTarget.blur();
            }
            if (e.key === "Escape") {
              setText(String(value));
              e.stopPropagation();
            }
          }}
        />
        <small>{unit}</small>
      </div>
    </label>
  );
}
