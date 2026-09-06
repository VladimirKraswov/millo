import { useEffect, useState } from "react";
import { Copy, Trash2 } from "lucide-react";
import type { CuttingTool } from "../../shared/tooling";
import type {
  SketchOperation,
  SketchShape,
  SketchStock,
} from "../../shared/sketch";
import { ToolSchematic } from "../../features/tool-library/ToolSchematic";
import {
  changeOperation,
  compatibleTools,
  operationLabels,
  toolSettings,
} from "./sketchModel";

export function SketchNumber({
  label,
  value,
  onChange,
  min = 0,
  max = 10_000,
  step = 0.1,
  unit = "мм",
}: {
  readonly label: string;
  readonly value: number;
  readonly onChange: (v: number) => void;
  readonly min?: number;
  readonly max?: number;
  readonly step?: number;
  readonly unit?: string;
}) {
  const [text, setText] = useState(String(value));
  useEffect(() => {
    setText(String(value));
  }, [value]);
  const commit = () => {
    const parsed = Number(text);
    if (text.trim() && Number.isFinite(parsed)) {
      const value = Math.min(max, Math.max(min, parsed));
      setText(String(value));
      onChange(value);
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
          onChange={(e) => setText(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              commit();
              e.currentTarget.blur();
            }
          }}
        />
        <small>{unit}</small>
      </div>
    </label>
  );
}

export function SketchInspector({
  shape,
  stock,
  tools,
  onChange,
  onDelete,
  onDuplicate,
  onArray,
}: {
  readonly shape?: SketchShape;
  readonly stock: SketchStock;
  readonly tools: readonly CuttingTool[];
  readonly onChange: (s: SketchShape) => void;
  readonly onDelete: () => void;
  readonly onDuplicate: () => void;
  readonly onArray: (count: number, dx: number, dy: number) => void;
}) {
  const [count, setCount] = useState(3),
    [dx, setDx] = useState(15),
    [dy, setDy] = useState(0);
  if (!shape)
    return (
      <section className="sketch-properties is-empty">
        <h3>Свойства фигуры</h3>
        <p>Фигура не выбрана</p>
      </section>
    );
  const op = shape.operation,
    geometry = shape.geometry;
  const tool = tools.find((t) => t.id === op.toolId);
  const operation = (update: Partial<SketchOperation>) =>
    onChange({ ...shape, operation: { ...op, ...update } });
  const actualDepth = op.through
    ? stock.thicknessMm + stock.breakthroughMm
    : op.depthMm;
  return (
    <section className="sketch-properties">
      <header>
        <input
          aria-label="Название фигуры"
          value={shape.name}
          maxLength={80}
          onChange={(e) => onChange({ ...shape, name: e.target.value })}
        />
        <button
          title="Дублировать фигуру"
          aria-label="Дублировать фигуру"
          onClick={onDuplicate}
          type="button"
        >
          <Copy size={16} />
        </button>
        <button
          title="Удалить фигуру"
          aria-label="Удалить фигуру"
          onClick={onDelete}
          type="button"
        >
          <Trash2 size={16} />
        </button>
      </header>
      <div className="sketch-fields">
        <SketchNumber
          label="Центр X"
          value={shape.xMm}
          max={stock.widthMm}
          onChange={(v) => onChange({ ...shape, xMm: v })}
        />
        <SketchNumber
          label="Центр Y"
          value={shape.yMm}
          max={stock.heightMm}
          onChange={(v) => onChange({ ...shape, yMm: v })}
        />
        {geometry.kind === "rectangle" && (
          <>
            <SketchNumber
              label="Ширина фигуры"
              min={0.1}
              value={geometry.width}
              onChange={(width) =>
                onChange({
                  ...shape,
                  geometry: {
                    ...geometry,
                    width,
                    radius: Math.min(geometry.radius, width / 2),
                  },
                })
              }
            />
            <SketchNumber
              label="Высота фигуры"
              min={0.1}
              value={geometry.height}
              onChange={(height) =>
                onChange({
                  ...shape,
                  geometry: {
                    ...geometry,
                    height,
                    radius: Math.min(geometry.radius, height / 2),
                  },
                })
              }
            />
            <SketchNumber
              label="Радиус углов"
              value={geometry.radius}
              max={Math.min(geometry.width, geometry.height) / 2}
              onChange={(radius) =>
                onChange({ ...shape, geometry: { ...geometry, radius } })
              }
            />
          </>
        )}
        {geometry.kind === "circle" && (
          <SketchNumber
            label="Диаметр отверстия"
            min={0.1}
            value={geometry.diameter}
            onChange={(diameter) =>
              onChange({ ...shape, geometry: { ...geometry, diameter } })
            }
          />
        )}
        {geometry.kind !== "circle" && (
          <SketchNumber
            label="Поворот"
            min={-360}
            max={360}
            step={1}
            unit="°"
            value={shape.rotationDegrees}
            onChange={(rotationDegrees) =>
              onChange({ ...shape, rotationDegrees })
            }
          />
        )}
      </div>
      {geometry.kind === "polygon" && (
        <details>
          <summary>Вершины · {geometry.points.length}</summary>
          <div className="sketch-vertices">
            {geometry.points.map((point, i) => (
              <div className="sketch-fields" key={i}>
                <SketchNumber
                  label={`X${i + 1}`}
                  value={point.x}
                  min={-10_000}
                  onChange={(x) =>
                    onChange({
                      ...shape,
                      geometry: {
                        ...geometry,
                        points: geometry.points.map((p, j) =>
                          j === i ? { ...p, x } : p,
                        ),
                      },
                    })
                  }
                />
                <SketchNumber
                  label={`Y${i + 1}`}
                  value={point.y}
                  min={-10_000}
                  onChange={(y) =>
                    onChange({
                      ...shape,
                      geometry: {
                        ...geometry,
                        points: geometry.points.map((p, j) =>
                          j === i ? { ...p, y } : p,
                        ),
                      },
                    })
                  }
                />
              </div>
            ))}
          </div>
        </details>
      )}
      <label className="sketch-select">
        <span>Обработка</span>
        <select
          aria-label="Обработка фигуры"
          value={op.kind}
          onChange={(e) =>
            onChange(
              changeOperation(
                shape,
                e.target.value as SketchOperation["kind"],
                tools,
              ),
            )
          }
        >
          {Object.entries(operationLabels)
            .filter(([kind]) => kind !== "drill" || geometry.kind === "circle")
            .map(([kind, label]) => (
              <option key={kind} value={kind}>
                {label}
              </option>
            ))}
        </select>
      </label>
      <label className="sketch-select">
        <span>Инструмент</span>
        <select
          aria-label="Фреза для фигуры"
          value={op.toolId}
          onChange={(e) =>
            operation(toolSettings(tools.find((t) => t.id === e.target.value)))
          }
        >
          <option value="">Выберите инструмент</option>
          {op.toolId && !tool && (
            <option value={op.toolId}>
              Инструмент отсутствует в библиотеке
            </option>
          )}
          {compatibleTools(tools, op.kind).map((t) => (
            <option key={t.id} value={t.id}>
              Ø{t.diameterMm} · {t.name}
            </option>
          ))}
        </select>
      </label>
      {tool && (
        <div className="sketch-tool">
          <ToolSchematic tool={tool} compact />
          <span>
            Ø {tool.diameterMm} мм
            <br />
            <small>Режущая часть {tool.cuttingLengthMm} мм</small>
          </span>
        </div>
      )}
      <div className="sketch-depth">
        <label>
          <input
            type="checkbox"
            checked={op.through}
            disabled={op.kind === "engrave"}
            onChange={(e) =>
              operation({
                through: e.target.checked,
                tabs: {
                  ...op.tabs,
                  count: e.target.checked ? op.tabs.count : 0,
                },
              })
            }
          />
          Насквозь
        </label>
        <strong>Z −{actualDepth.toFixed(2)} мм</strong>
      </div>
      <div className="sketch-fields">
        {!op.through && (
          <SketchNumber
            label="Глубина"
            min={0.01}
            max={stock.thicknessMm}
            value={op.depthMm}
            onChange={(depthMm) => operation({ depthMm })}
          />
        )}
        <SketchNumber
          label="За проход"
          min={0.01}
          max={10}
          value={op.stepdownMm}
          onChange={(stepdownMm) => operation({ stepdownMm })}
        />
        <SketchNumber
          label="Подача XY"
          min={1}
          max={30_000}
          step={50}
          unit="мм/мин"
          value={op.feedMmPerMin}
          onChange={(feedMmPerMin) => operation({ feedMmPerMin })}
        />
      </div>
      {op.through && ["inside", "outside"].includes(op.kind) && (
        <div className="sketch-tabs-settings">
          <label>
            <input
              type="checkbox"
              checked={op.tabs.count > 0}
              onChange={(e) =>
                operation({
                  tabs: { ...op.tabs, count: e.target.checked ? 4 : 0 },
                })
              }
            />
            Удерживающие перемычки
          </label>
          {op.tabs.count > 0 && (
            <div className="sketch-fields">
              <SketchNumber
                label="Количество перемычек"
                value={op.tabs.count}
                min={2}
                max={16}
                step={1}
                unit="шт"
                onChange={(v) =>
                  operation({ tabs: { ...op.tabs, count: Math.round(v) } })
                }
              />
              <SketchNumber
                label="Ширина перемычки"
                value={op.tabs.widthMm}
                min={0.5}
                max={50}
                onChange={(widthMm) =>
                  operation({ tabs: { ...op.tabs, widthMm } })
                }
              />
              <SketchNumber
                label="Высота от низа листа"
                value={op.tabs.heightMm}
                min={0.05}
                max={stock.thicknessMm}
                onChange={(heightMm) =>
                  operation({ tabs: { ...op.tabs, heightMm } })
                }
              />
            </div>
          )}
        </div>
      )}
      <details>
        <summary>Подача Z и шпиндель</summary>
        <div className="sketch-fields">
          <SketchNumber
            label="Подача Z"
            value={op.plungeMmPerMin}
            min={1}
            max={10_000}
            step={10}
            unit="мм/мин"
            onChange={(plungeMmPerMin) => operation({ plungeMmPerMin })}
          />
          <SketchNumber
            label="Обороты"
            value={op.spindleRpm}
            min={1000}
            max={100_000}
            step={500}
            unit="rpm"
            onChange={(spindleRpm) =>
              operation({ spindleRpm: Math.round(spindleRpm) })
            }
          />
          {op.kind === "pocket" && (
            <SketchNumber
              label="Боковой шаг"
              value={op.stepoverPercent}
              min={5}
              max={50}
              step={5}
              unit="% Ø"
              onChange={(stepoverPercent) => operation({ stepoverPercent })}
            />
          )}
        </div>
      </details>
      <details>
        <summary>Повторить с шагом</summary>
        <div className="sketch-fields">
          <SketchNumber
            label="Копий"
            value={count}
            min={1}
            max={30}
            step={1}
            unit="шт"
            onChange={(v) => setCount(Math.round(v))}
          />
          <SketchNumber
            label="Шаг X"
            value={dx}
            min={-10_000}
            onChange={setDx}
          />
          <SketchNumber
            label="Шаг Y"
            value={dy}
            min={-10_000}
            onChange={setDy}
          />
        </div>
        <button
          type="button"
          disabled={dx === 0 && dy === 0}
          onClick={() => onArray(count, dx, dy)}
        >
          <Copy size={15} />
          Создать копии
        </button>
      </details>
    </section>
  );
}
