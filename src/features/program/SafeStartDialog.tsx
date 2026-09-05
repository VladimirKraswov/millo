import { DialogSurface } from "../../components/DialogSurface";
import { CornerDownRight, Play, Route, ScanSearch, X } from "lucide-react";
import { useEffect, useState } from "react";

import type { SafeStartPackage } from "../../shared/realRun";
import { canPrepareSafeStart } from "./safeStartModel";

interface SafeStartDialogProps {
  readonly minimumSafeZ: number;
  readonly motionCount: number;
  readonly onClose: () => void;
  readonly onPrepare: (safeZMm: number) => Promise<SafeStartPackage>;
  readonly onPrepared: (prepared: SafeStartPackage) => Promise<void> | void;
  readonly open: boolean;
  readonly selectedCommand: string;
  readonly sourceLine: number;
  readonly suggestedSafeZ: number;
}

export function SafeStartDialog({
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

  useEffect(() => {
    if (!open) return;
    setSafeZ(suggestedSafeZ);
    setBusy(false);
    setError(undefined);
  }, [open, sourceLine, suggestedSafeZ]);

  if (!open) return null;

  const prepare = async () => {
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
    setError(undefined);
    try {
      await onPrepared(await onPrepare(safeZ));
      onClose();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
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
              !canPrepareSafeStart({
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
