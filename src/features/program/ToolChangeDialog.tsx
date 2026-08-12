import { Check, CircleAlert, Play, Wrench, X } from "lucide-react";
import { useEffect, useState } from "react";

import type { ToolChangeConfirmation } from "../../shared/realRun";
import {
  emptyToolChangeConfirmation,
  setToolChangeReadiness,
  toolChangeConfirmationProgress,
  type ToolChangeChecklistKey,
} from "./toolChangeConfirmationModel";

interface ToolChangeDialogProps {
  readonly open: boolean;
  readonly sourceLine: number;
  readonly requestedTool?: number;
  readonly onClose: () => void;
  readonly onComplete: (
    confirmation: ToolChangeConfirmation,
  ) => Promise<void>;
}

const checklist: ReadonlyArray<{
  key: ToolChangeChecklistKey;
  title: string;
  detail: string;
}> = [
  {
    key: "toolSecured",
    title: "Новый инструмент закреплён",
    detail: "Фреза соответствует программе и надёжно зажата в цанге.",
  },
  {
    key: "zZeroVerified",
    title: "Ноль Z проверен",
    detail: "Рабочий Z-ноль повторно выставлен для длины нового инструмента.",
  },
  {
    key: "safeZVerified",
    title: "Safe Z свободен",
    detail: "Инструмент находится выше заготовки, крепежа и оснастки.",
  },
  {
    key: "pathClear",
    title: "Оставшаяся траектория свободна",
    detail: "Инструмент, прижимы и заготовка не пересекают дальнейший путь.",
  },
  {
    key: "manualSpindleRunning",
    title: "Ручной шпиндель запущен",
    detail: "Вращение, направление и звук проверены перед продолжением.",
  },
  {
    key: "powerControlReachable",
    title: "Питание доступно",
    detail: "Шпиндель и станок можно немедленно обесточить рукой.",
  },
];

export function ToolChangeDialog({
  open,
  sourceLine,
  requestedTool,
  onClose,
  onComplete,
}: ToolChangeDialogProps) {
  const [confirmation, setConfirmation] = useState<ToolChangeConfirmation>(() =>
    emptyToolChangeConfirmation(sourceLine, requestedTool),
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();

  useEffect(() => {
    if (!open) return;
    setConfirmation(emptyToolChangeConfirmation(sourceLine, requestedTool));
    setBusy(false);
    setError(undefined);
  }, [open, requestedTool, sourceLine]);

  if (!open) return null;
  const progress = toolChangeConfirmationProgress(confirmation);

  const complete = async () => {
    if (!progress.complete || busy) return;
    setBusy(true);
    setError(undefined);
    try {
      await onComplete(confirmation);
      onClose();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="machine-dialog-backdrop first-cut-backdrop" role="presentation">
      <section
        aria-labelledby="tool-change-title"
        aria-modal="true"
        className="machine-dialog first-cut-dialog tool-change-dialog"
        role="dialog"
      >
        <header>
          <div>
            <span>Host-managed M6 · L{sourceLine}</span>
            <h2 id="tool-change-title">
              {requestedTool === undefined
                ? "Смена инструмента"
                : `Установить инструмент T${requestedTool}`}
            </h2>
          </div>
          <button
            aria-label="Свернуть"
            disabled={busy}
            onClick={onClose}
            title="Свернуть"
            type="button"
          >
            <X aria-hidden="true" size={18} />
          </button>
        </header>

        <div className="first-cut-intro">
          <CircleAlert aria-hidden="true" size={18} />
          <div>
            <strong>Sender остановлен перед M6</strong>
            <span>GRBL не получал M6. Продолжение потребует свежий Idle и проверку G54-G59.</span>
          </div>
          <code>M6</code>
        </div>

        <div className="tool-change-identity">
          <Wrench aria-hidden="true" size={16} />
          <span>Запрошен</span>
          <strong>{requestedTool === undefined ? "инструмент из задания" : `T${requestedTool}`}</strong>
          <code>L{sourceLine}</code>
        </div>

        <div className="first-cut-checklist is-compact">
          <label>
            <input
              checked={progress.complete}
              disabled={busy}
              onChange={(event) =>
                setConfirmation((current) =>
                  setToolChangeReadiness(current, event.target.checked),
                )
              }
              type="checkbox"
            />
            <span aria-hidden="true" className="first-cut-checkmark">
              <Check size={13} />
            </span>
            <span>
              <strong>Смена инструмента завершена</strong>
              <small>Инструмент закреплён, Z-ноль и свободный путь проверены, шпиндель запущен</small>
            </span>
          </label>
        </div>
        <details className="confirmation-details">
          <summary>Что входит в подтверждение</summary>
          <div>
            {checklist.map((item) => (
              <span key={item.key}>
                <Check aria-hidden="true" size={12} />
                <span>
                  <strong>{item.title}</strong>
                  <small>{item.detail}</small>
                </span>
              </span>
            ))}
          </div>
        </details>

        {error && <p className="first-cut-error">{error}</p>}
        <footer>
          <span>Sender повторно проверит Idle и рабочую систему координат.</span>
          <button
            className="first-cut-authorize"
            disabled={!progress.complete || busy}
            onClick={() => void complete()}
            type="button"
          >
            <Play aria-hidden="true" size={15} />
            {busy ? "Проверка контроллера..." : "Проверить и продолжить"}
          </button>
        </footer>
      </section>
    </div>
  );
}
