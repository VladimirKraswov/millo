import { useState } from "react";
import {
  ArrowDown,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  Link2,
  LockKeyhole,
  UnlockKeyhole,
  Unlink,
} from "lucide-react";
import type {
  SketchAnchor,
  SketchAxis,
  SketchAxisConstraint,
  SketchJobRequest,
  SketchShape,
} from "../../shared/sketch";
import { preservePosition } from "./sketchConstraints";
import { SketchNumber } from "./SketchNumber";

function AnchorOptions({
  axis,
  shape,
}: {
  readonly axis: SketchAxis;
  readonly shape?: SketchShape;
}) {
  return (
    <>
      <option value="center">Центр</option>
      <option value="min">{axis === "x" ? "Левый край" : "Нижний край"}</option>
      <option value="max">
        {axis === "x" ? "Правый край" : "Верхний край"}
      </option>
      {shape?.geometry.kind === "polygon" &&
        shape.geometry.points.map((_, i) => (
          <option key={i} value={i}>
            Вершина {i + 1}
          </option>
        ))}
    </>
  );
}
const anchor = (value: string): SketchAnchor =>
  ["min", "center", "max"].includes(value)
    ? (value as SketchAnchor)
    : Number(value);

export function SketchPositionPanel({
  document: doc,
  shape,
  onChange,
}: {
  readonly document: SketchJobRequest;
  readonly shape: SketchShape;
  readonly onChange: (shape: SketchShape) => void;
}) {
  const [negativeDirection, setNegativeDirection] = useState({
    x: false,
    y: false,
  });
  const update = (axis: SketchAxis, constraint?: SketchAxisConstraint) =>
    onChange({
      ...shape,
      constraints: { ...shape.constraints, [axis]: constraint },
    });
  return (
    <section className="sketch-position">
      <header>
        <h3>Положение</h3>
        <button
          type="button"
          title={
            shape.locked
              ? "Разблокировать положение"
              : "Защитить положение от ручных изменений"
          }
          aria-label="Блокировка положения"
          aria-pressed={Boolean(shape.locked)}
          onClick={() => onChange({ ...shape, locked: !shape.locked })}
        >
          {shape.locked ? (
            <LockKeyhole size={16} />
          ) : (
            <UnlockKeyhole size={16} />
          )}
        </button>
      </header>
      <div className="sketch-fields">
        {(["x", "y"] as const).map((axis) => (
          <SketchNumber
            key={axis}
            label={`Центр ${axis.toUpperCase()}`}
            value={axis === "x" ? shape.xMm : shape.yMm}
            min={-10_000}
            disabled={shape.locked || Boolean(shape.constraints?.[axis])}
            onChange={(v) =>
              onChange({ ...shape, [axis === "x" ? "xMm" : "yMm"]: v })
            }
          />
        ))}
      </div>
      <details
        className="sketch-links"
        open={
          Boolean(shape.constraints?.x || shape.constraints?.y) || undefined
        }
      >
        <summary>
          <Link2 size={14} /> Размерные связи
        </summary>
        {(["x", "y"] as const).map((axis) => {
          const c = shape.constraints?.[axis],
            label = axis.toUpperCase();
          const negative = Boolean(
            c &&
            (c.offsetMm < 0 || (c.offsetMm === 0 && negativeDirection[axis])),
          );
          const target = doc.shapes.find((s) => s.id === c?.referenceId);
          const changeAnchor = (fields: Partial<SketchAxisConstraint>) => {
            if (c)
              update(
                axis,
                preservePosition(doc, shape, axis, { ...c, ...fields }),
              );
          };
          const Back = axis === "x" ? ArrowLeft : ArrowDown,
            Forward = axis === "x" ? ArrowRight : ArrowUp;
          return (
            <fieldset
              key={axis}
              className="sketch-axis-link"
              disabled={shape.locked}
            >
              <legend>Ось {label}</legend>
              <label className="sketch-select">
                <span>Отсчёт {label}</span>
                <select
                  aria-label={`Отсчёт ${label}`}
                  value={c ? (c.referenceId ?? "$stock") : "$free"}
                  onChange={(e) => {
                    const value = e.target.value;
                    update(
                      axis,
                      value === "$free"
                        ? undefined
                        : preservePosition(doc, shape, axis, {
                            referenceId: value === "$stock" ? undefined : value,
                            referenceAnchor:
                              value === "$stock" ? "min" : "center",
                            ownAnchor: "center",
                            offsetMm: 0,
                          }),
                    );
                  }}
                >
                  <option value="$free">Свободная координата</option>
                  <option value="$stock">Край / центр листа</option>
                  {doc.shapes
                    .filter((s) => s.id !== shape.id)
                    .map((s) => (
                      <option key={s.id} value={s.id}>
                        {s.name}
                      </option>
                    ))}
                </select>
              </label>
              {c && (
                <>
                  <div className="sketch-fields">
                    <label className="sketch-select">
                      <span>От точки {label}</span>
                      <select
                        aria-label={`Опорная точка ${label}`}
                        value={c.referenceAnchor}
                        onChange={(e) =>
                          changeAnchor({
                            referenceAnchor: anchor(e.target.value),
                          })
                        }
                      >
                        <AnchorOptions axis={axis} shape={target} />
                      </select>
                    </label>
                    <label className="sketch-select">
                      <span>До точки {label}</span>
                      <select
                        aria-label={`Точка фигуры ${label}`}
                        value={c.ownAnchor}
                        onChange={(e) =>
                          changeAnchor({ ownAnchor: anchor(e.target.value) })
                        }
                      >
                        <AnchorOptions axis={axis} shape={shape} />
                      </select>
                    </label>
                  </div>
                  <div className="sketch-link-distance">
                    <SketchNumber
                      label={`Расстояние ${label}`}
                      value={Math.abs(c.offsetMm)}
                      disabled={shape.locked}
                      onChange={(v) =>
                        update(axis, {
                          ...c,
                          offsetMm: (negative ? -1 : 1) * v,
                        })
                      }
                    />
                    <div
                      className="sketch-link-direction"
                      role="group"
                      aria-label={`Направление ${label}`}
                    >
                      <button
                        type="button"
                        aria-label={
                          axis === "x" ? "Влево от опоры" : "Ниже опоры"
                        }
                        title={axis === "x" ? "Влево от опоры" : "Ниже опоры"}
                        aria-pressed={negative}
                        onClick={() => {
                          setNegativeDirection((v) => ({ ...v, [axis]: true }));
                          update(axis, {
                            ...c,
                            offsetMm: -Math.abs(c.offsetMm),
                          });
                        }}
                      >
                        <Back size={17} />
                      </button>
                      <button
                        type="button"
                        aria-label={
                          axis === "x" ? "Вправо от опоры" : "Выше опоры"
                        }
                        title={axis === "x" ? "Вправо от опоры" : "Выше опоры"}
                        aria-pressed={!negative}
                        onClick={() => {
                          setNegativeDirection((v) => ({
                            ...v,
                            [axis]: false,
                          }));
                          update(axis, {
                            ...c,
                            offsetMm: Math.abs(c.offsetMm),
                          });
                        }}
                      >
                        <Forward size={17} />
                      </button>
                    </div>
                    <button
                      type="button"
                      aria-label={`Снять связь ${label}`}
                      title={`Снять связь ${label}`}
                      onClick={() => update(axis)}
                    >
                      <Unlink size={16} />
                    </button>
                  </div>
                </>
              )}
            </fieldset>
          );
        })}
      </details>
    </section>
  );
}
