import { DialogSurface } from "../../components/DialogSurface";
import { useAsyncScope } from "../../components/useAsyncScope";
import { CornerDownRight, Play, Route, ScanSearch, X } from "lucide-react";
import { useEffect, useState } from "react";

import type { SafeStartPackage } from "../../shared/realRun";
import { canPrepareSafeStart } from "./safeStartModel";

interface SafeStartDialogProps {
  readonly rotaryProgram?: boolean;
  readonly minimumSafeZ: number;
  readonly motionCount: number;
  readonly onClose: () => void;
  readonly onPrepare: (safeZMm: number, rotary?: { readonly initialWorkADegrees: number; readonly rotaryClearanceConfirmed: boolean }) => Promise<SafeStartPackage>;
  readonly onPrepared: (prepared: SafeStartPackage) => Promise<void> | void;
  readonly open: boolean;
  readonly selectedCommand: string;
  readonly sourceLine: number;
  readonly suggestedSafeZ: number;
}

export function SafeStartDialog({
  rotaryProgram = false,
  minimumSafeZ,
  motionCount,
  onClose,
  onPrepare,
  onPrepared,
  open,
  selectedCommand,
  sourceLine,
  suggestedSafeZ,
}: SafeStartDialogProps) {
  const [safeZ, setSafeZ] = useState(suggestedSafeZ);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [initialA, setInitialA] = useState("0");
  const [rotaryConfirmed, setRotaryConfirmed] = useState(false);
  const rotaryReady = !rotaryProgram || rotaryConfirmed && initialA.trim() !== "" && Number.isFinite(Number(initialA));
  const captureScope = useAsyncScope([open, sourceLine, suggestedSafeZ]);

  useEffect(() => {
    if (!open) return;
    setSafeZ(suggestedSafeZ);
    setBusy(false);
    setError(undefined);
    setRotaryConfirmed(false);
  }, [open, sourceLine, suggestedSafeZ]);

  if (!open) return null;

  const prepare = async () => {
    if (!rotaryReady) return;
    if (
      !canPrepareSafeStart({
        busy,
        minimumSafeZ,
        motionCount,
        safeZ,
        sourceLine,
      })
    ) {
      return;
    }
    setBusy(true);
    const isCurrent = captureScope();
    setError(undefined);
    try {
      const prepared = rotaryProgram
        ? await onPrepare(safeZ, { initialWorkADegrees: Number(initialA), rotaryClearanceConfirmed: rotaryConfirmed })
        : await onPrepare(safeZ);
      if (!isCurrent()) return;
      await onPrepared(prepared);
      if (isCurrent()) onClose();
    } catch (reason) {
      if (isCurrent()) setError(String(reason));
    } finally {
      if (isCurrent()) setBusy(false);
    }
  };

  return (
    <div
      className="machine-dialog-backdrop safe-start-backdrop"
      role="presentation"
    >
      <DialogSurface
        onDismiss={onClose}
        dismissible={!busy}
        aria-labelledby="safe-start-title"
        className="machine-dialog safe-start-dialog"
      >
        <header>
          <div>
            <span>Частичный повтор</span>
            <h2 id="safe-start-title">Запустить с выбранного участка</h2>
          </div>
          <button
            aria-label="Закрыть"
            disabled={busy}
            onClick={onClose}
            title="Закрыть"
            type="button"
          >
            <X aria-hidden="true" size={18} />
          </button>
        </header>

        <div className="safe-start-body">
          <div className="safe-start-selection">
            <Route aria-hidden="true" size={19} />
            <div>
              <span>Выбранная траектория</span>
              <strong>L{sourceLine}</strong>
              <code title={selectedCommand}>{selectedCommand}</code>
            </div>
            <small>{motionCount} сегм.</small>
          </div>

          <label className="safe-start-z">
            <span>
              <strong>Safe Z</strong>
              <small>
                Не ниже геометрии программы: {minimumSafeZ.toFixed(3)} mm
              </small>
            </span>
            <span>
              <input
                autoFocus
                disabled={busy}
                min={minimumSafeZ}
                onChange={(event) => setSafeZ(Number(event.target.value))}
                step="0.1"
                type="number"
                value={safeZ}
              />
              <code>mm</code>
            </span>
          </label>

          <div className="safe-start-route">
            <CornerDownRight aria-hidden="true" size={18} />
            <p>
              Millo найдёт последний безопасный rapid-вход перед L{sourceLine},
              поднимется на Safe Z и восстановит WCS, режимы, подачу и
              инструмент. Рез внутри уже начатого участка не будет продолжен
              вслепую.
            </p>
          </div>

          {rotaryProgram && <div className="safe-start-rotary">
            <label>Угол A в начале исходной программы
              <span><input aria-label="Начальный угол A" type="number" step="0.1" value={initialA} disabled={busy} onChange={(event) => setInitialA(event.target.value)} /> °</span>
            </label>
            <label><input type="checkbox" checked={rotaryConfirmed} disabled={busy} onChange={(event) => setRotaryConfirmed(event.target.checked)} />
              <span>Индекс A восстановлен; на Safe Z заготовка и крепёж могут свободно повернуться</span>
            </label>
          </div>}

          <div className="safe-start-next">
            <ScanSearch aria-hidden="true" size={17} />
            <span>
              <strong>Следующий шаг: GRBL Check</strong>
              <small>
                Проверяется весь сформированный остаток, включая безопасный
                подлёт.
              </small>
            </span>
          </div>

          <p
            aria-hidden={!error}
            className={`first-cut-error${error ? "" : " is-empty"}`}
          >
            {error ?? "Нет ошибок"}
          </p>
        </div>

        <footer>
          <button disabled={busy} onClick={onClose} type="button">
            Отмена
          </button>
          <button
            className="first-cut-authorize"
            disabled={
              !rotaryReady || !canPrepareSafeStart({
                busy,
                minimumSafeZ,
                motionCount,
                safeZ,
                sourceLine,
              })
            }
            onClick={() => void prepare()}
            type="button"
          >
            <Play aria-hidden="true" size={15} />
            {busy ? "Запускаем Check..." : "Подготовить и запустить Check"}
          </button>
        </footer>
      </DialogSurface>
    </div>
  );
}
