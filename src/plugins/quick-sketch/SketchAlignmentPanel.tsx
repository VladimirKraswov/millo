import { useEffect, useState } from "react";
import {
  AlignHorizontalJustifyCenter,
  AlignVerticalJustifyCenter,
  ArrowRight,
  ArrowUp,
} from "lucide-react";
import type { SketchAxis, SketchShape } from "../../shared/sketch";
import { SketchNumber } from "./SketchNumber";

export function SketchAlignmentPanel({
  shapes,
  onAlign,
}: {
  readonly shapes: readonly SketchShape[];
  readonly onAlign: (
    referenceId: string,
    axis: SketchAxis,
    step?: number,
  ) => void;
}) {
  const [reference, setReference] = useState(shapes[0]?.id ?? ""),
    [step, setStep] = useState(20);
  useEffect(() => {
    if (!shapes.some((s) => s.id === reference))
      setReference(shapes[0]?.id ?? "");
  }, [shapes, reference]);
  return (
    <section className="sketch-alignment">
      <h3>Выбрано фигур: {shapes.length}</h3>
      <label className="sketch-select">
        <span>Неподвижная опора</span>
        <select
          aria-label="Опорная фигура"
          value={reference}
          onChange={(e) => setReference(e.target.value)}
        >
          {shapes.map((s) => (
            <option key={s.id} value={s.id}>
              {s.name}
            </option>
          ))}
        </select>
      </label>
      <div className="sketch-fields">
        <button type="button" onClick={() => onAlign(reference, "y")}>
          <AlignVerticalJustifyCenter size={17} />
          По горизонтали
        </button>
        <button type="button" onClick={() => onAlign(reference, "x")}>
          <AlignHorizontalJustifyCenter size={17} />
          По вертикали
        </button>
      </div>
      <SketchNumber
        label="Шаг между центрами"
        value={step}
        min={0.1}
        onChange={setStep}
      />
      <div className="sketch-fields">
        <button type="button" onClick={() => onAlign(reference, "x", step)}>
          <ArrowRight size={17} />
          Разместить по X
        </button>
        <button type="button" onClick={() => onAlign(reference, "y", step)}>
          <ArrowUp size={17} />
          Разместить по Y
        </button>
      </div>
    </section>
  );
}
