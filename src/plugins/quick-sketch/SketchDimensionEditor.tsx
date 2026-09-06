import { useLayoutEffect, useRef, useState } from "react";
import { Check, X } from "lucide-react";
import type { SketchDimensionTarget } from "./sketchDimensionModel";

export interface SketchDimensionEditorState {
  readonly target: SketchDimensionTarget;
  readonly label: string;
  readonly value: number;
  readonly left: number;
  readonly top: number;
}

export function SketchDimensionEditor({
  edit,
  onCommit,
  onCancel,
}: {
  readonly edit: SketchDimensionEditorState;
  readonly onCommit: (value: number) => void;
  readonly onCancel: () => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  const [value, setValue] = useState(String(edit.value));
  const [error, setError] = useState<string>();
  useLayoutEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);
  return (
    <form
      className="sketch-dimension-editor"
      aria-label="Размер на чертеже"
      style={{ left: edit.left, top: edit.top }}
      onPointerDown={(e) => e.stopPropagation()}
      onSubmit={(e) => {
        e.preventDefault();
        const text = value.trim().replace(",", "."),
          parsed = Number(text);
        if (!text || !Number.isFinite(parsed)) {
          setError("Введите число в миллиметрах");
          return;
        }
        try {
          onCommit(parsed);
        } catch (reason) {
          setError(String(reason).replace(/^Error:\s*/, ""));
        }
      }}
    >
      <label htmlFor="sketch-inline-size">{edit.label}</label>
      <div>
        <input
          ref={input}
          id="sketch-inline-size"
          aria-label="Значение размера"
          aria-invalid={Boolean(error)}
          aria-describedby={error ? "sketch-inline-error" : undefined}
          inputMode="decimal"
          autoComplete="off"
          value={value}
          onChange={(e) => {
            setValue(e.target.value);
            setError(undefined);
          }}
        />
        <span>мм</span>
        <button
          type="submit"
          title="Применить размер"
          aria-label="Применить размер"
        >
          <Check size={16} />
        </button>
        <button
          type="button"
          title="Отменить размер"
          aria-label="Отменить размер"
          onClick={onCancel}
        >
          <X size={16} />
        </button>
      </div>
      {error && (
        <p id="sketch-inline-error" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}
