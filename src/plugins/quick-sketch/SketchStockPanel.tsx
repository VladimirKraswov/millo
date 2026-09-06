import type { SketchJobRequest } from "../../shared/sketch";
import { SketchNumber } from "./SketchNumber";
export function SketchStockPanel({
  document: doc,
  onChange,
}: {
  readonly document: SketchJobRequest;
  readonly onChange: (doc: SketchJobRequest) => void;
}) {
  return (
    <details
      className="sketch-stock"
      open={doc.shapes.length === 0 || undefined}
    >
      <summary>
        Заготовка{" "}
        <span>
          {doc.stock.widthMm} × {doc.stock.heightMm} × {doc.stock.thicknessMm}{" "}
          мм
        </span>
      </summary>
      <div className="sketch-stock-fields">
        <div className="sketch-fields">
          <SketchNumber
            label="Ширина листа"
            min={1}
            value={doc.stock.widthMm}
            onChange={(widthMm) =>
              onChange({ ...doc, stock: { ...doc.stock, widthMm } })
            }
          />
          <SketchNumber
            label="Высота листа"
            min={1}
            value={doc.stock.heightMm}
            onChange={(heightMm) =>
              onChange({ ...doc, stock: { ...doc.stock, heightMm } })
            }
          />
          <SketchNumber
            label="Толщина листа"
            min={0.05}
            max={100}
            value={doc.stock.thicknessMm}
            onChange={(thicknessMm) =>
              onChange({ ...doc, stock: { ...doc.stock, thicknessMm } })
            }
          />
          <SketchNumber
            label="Безопасный Z"
            min={0.5}
            max={100}
            value={doc.stock.safeZMm}
            onChange={(safeZMm) =>
              onChange({ ...doc, stock: { ...doc.stock, safeZMm } })
            }
          />
        </div>
        <span className="sketch-datum">Z0 · верх материала</span>
        <details>
          <summary>Подложка и управление шпинделем</summary>
          <SketchNumber
            label="Выход в подложку"
            value={doc.stock.breakthroughMm}
            max={1}
            onChange={(breakthroughMm) =>
              onChange({
                ...doc,
                stock: { ...doc.stock, breakthroughMm },
              })
            }
          />
          <label className="sketch-select">
            <span>Шпиндель</span>
            <select
              aria-label="Управление шпинделем"
              value={doc.stock.spindleMode}
              onChange={(e) =>
                onChange({
                  ...doc,
                  stock: {
                    ...doc.stock,
                    spindleMode: e.target.value as "manual" | "controller",
                  },
                })
              }
            >
              <option value="manual">Включаю вручную</option>
              <option value="controller">
                Управляется контроллером · M3/M5
              </option>
            </select>
          </label>
        </details>
      </div>
    </details>
  );
}
