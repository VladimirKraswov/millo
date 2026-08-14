import {
  Check,
  CircleAlert,
  Power,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";

import type {
  FirstCutConfirmation,
  FirstCutPreparation,
  ProgramExecutionOptions,
  ProgramRunIntent,
  RunPreflightReport,
} from "../../shared/realRun";
import type { SenderSnapshot } from "../../shared/dryRun";
import {
  emptyFirstCutConfirmation,
  firstCutAuthorizationControls,
  type FirstCutAuthorizationControls,
} from "./firstCutAuthorizationModel";

interface FirstCutAuthorizationDialogProps {
  readonly open: boolean;
  readonly intent: ProgramRunIntent;
  readonly executionOptions: ProgramExecutionOptions;
  readonly depthCorrection?: {
    readonly adjustmentMm: number;
  };
  readonly report?: RunPreflightReport;
  readonly startingToolNumber?: number;
  readonly onAuthorize: (
    confirmation: FirstCutConfirmation,
  ) => Promise<FirstCutPreparation>;
  readonly onAuthorized: (preparation: FirstCutPreparation) => void;
  readonly onStart: (preparation: FirstCutPreparation) => Promise<SenderSnapshot>;
  readonly onStarted: (snapshot: SenderSnapshot) => void;
  readonly onClose: () => void;
}

export function FirstCutAuthorizationDialog({
  open,
  intent,
  executionOptions,
  depthCorrection,
  report,
  startingToolNumber,
  onAuthorize,
  onAuthorized,
  onStart,
  onStarted,
  onClose,
}: FirstCutAuthorizationDialogProps) {
  const [confirmation, setConfirmation] = useState<FirstCutConfirmation>(
    emptyFirstCutConfirmation,
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();

  useEffect(() => {
    if (!open) return;
    setConfirmation({ ...emptyFirstCutConfirmation, intent, executionOptions });
    setBusy(false);
    setError(undefined);
  }, [executionOptions, open, intent, report?.programFingerprint]);

  if (!open) return null;

  const controls: FirstCutAuthorizationControls = firstCutAuthorizationControls(
    confirmation,
    { report, gatewayAvailable: true, busy },
  );

  const authorizeAndStart = async () => {
    if (!controls.canAuthorize) return;
    setBusy(true);
    setError(undefined);
    try {
      const next = await onAuthorize(confirmation);
      onAuthorized(next);
      onStarted(await onStart(next));
      onClose();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const setupReady = confirmation.xyzZeroVerified &&
    confirmation.safeZVerified &&
    confirmation.pathClear &&
    confirmation.powerControlReachable &&
    (intent === "airRun"
      ? confirmation.toolRemoved
      : confirmation.stockSecured && confirmation.toolSecured);
  const setSetupReady = (ready: boolean) => setConfirmation((current) => ({
    ...current,
    xyzZeroVerified: ready,
    safeZVerified: ready,
    pathClear: ready,
    powerControlReachable: ready,
    stockSecured: current.intent === "cutting" && ready,
    toolSecured: current.intent === "cutting" && ready,
    toolRemoved: current.intent === "airRun" && ready,
  }));
  const hasSurfaceMap = executionOptions.surfaceMapId !== undefined;

  return (
    <div className="machine-dialog-backdrop first-cut-backdrop" role="presentation">
      <section
        aria-labelledby="first-cut-title"
        aria-modal="true"
        className="machine-dialog first-cut-dialog"
        role="dialog"
      >
        <header>
          <div>
            <span>Последнее действие</span>
            <h2 id="first-cut-title">Начать движение</h2>
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

        <div className="first-cut-intro">
          <CircleAlert aria-hidden="true" size={18} />
          <div>
            <strong>Проверьте станок перед стартом</strong>
            <span>Контроллер и G-code уже проверены. Остались только физические действия.</span>
          </div>
          <code>{intent === "airRun" ? "CHECK" : "RUN"}</code>
        </div>
        <div className="program-run-mode-summary">
          <span>Режим</span>
          <strong>{intent === "airRun" ? "Проверка движения" : "Обработка"}</strong>
        </div>
        {intent === "cutting" && depthCorrection && (
          <div className="program-run-mode-summary">
            <span>Коррекция глубины</span>
            <strong>ΔZ {formatSignedOffset(depthCorrection.adjustmentMm)} мм</strong>
          </div>
        )}
        {intent === "cutting" && startingToolNumber !== undefined && (
          <div className="program-run-mode-summary">
            <span>Стартовый инструмент</span>
            <strong>T{startingToolNumber}</strong>
          </div>
        )}
        <div className="first-cut-checklist">
          <label>
            <input
              checked={setupReady}
              disabled={busy}
              onChange={(event) => setSetupReady(event.target.checked)}
              type="checkbox"
            />
            <span aria-hidden="true" className="first-cut-checkmark">
              <Check size={13} />
            </span>
            <span>
              <strong>
                Заготовка, фреза{intent === "cutting" && startingToolNumber !== undefined
                  ? ` T${startingToolNumber}`
                  : ""}, ноль и траектория готовы
              </strong>
              <small>
                {intent === "airRun"
                  ? "Инструмент снят, рабочая зона свободна"
                  : `${startingToolNumber === undefined ? "Фреза установлена" : `Установлен T${startingToolNumber}`}; крепёж не пересекает путь, питание доступно`}
              </small>
            </span>
          </label>
          {intent === "cutting" && hasSurfaceMap && (
            <label>
              <input
                checked={confirmation.probeRemoved}
                disabled={busy}
                onChange={(event) => setConfirmation((current) => ({
                  ...current,
                  probeRemoved: event.target.checked,
                }))}
                type="checkbox"
              />
              <span aria-hidden="true" className="first-cut-checkmark"><Check size={13} /></span>
              <span>
                <strong>Щуп и провода убраны</strong>
                <small>Цепь щупа не может попасть под инструмент или оси</small>
              </span>
            </label>
          )}
          <label>
            <input
              checked={intent === "airRun"
                ? confirmation.manualSpindleOff
                : confirmation.manualSpindleRunning}
              disabled={busy}
              onChange={(event) => setConfirmation((current) => ({
                ...current,
                manualSpindleOff: current.intent === "airRun" && event.target.checked,
                manualSpindleRunning: current.intent === "cutting" && event.target.checked,
              }))}
              type="checkbox"
            />
            <span aria-hidden="true" className="first-cut-checkmark"><Check size={13} /></span>
            <span>
              <strong>{intent === "airRun" ? "Шпиндель выключен" : "Шпиндель запущен"}</strong>
              <small>{intent === "airRun"
                ? "Станок движется по траектории без обработки материала"
                : "Ручной шпиндель вращается в нужном направлении"}</small>
            </span>
          </label>
        </div>
        <p
          aria-hidden={!error}
          className={`first-cut-error${error ? "" : " is-empty"}`}
        >
          {error ?? "Нет ошибок"}
        </p>
        <footer>
          <button disabled={busy} onClick={onClose} type="button">Отмена</button>
          <button
            className="first-cut-authorize"
            disabled={!controls.canAuthorize}
            onClick={() => void authorizeAndStart()}
            type="button"
          >
            <Power aria-hidden="true" size={15} />
            {busy
              ? "Проверка и запуск..."
              : intent === "airRun"
                ? "Начать проверку движения"
                : "Начать обработку"}
          </button>
        </footer>
      </section>
    </div>
  );
}

function formatSignedOffset(value: number): string {
  if (Math.abs(value) < 0.0005) return "0.000";
  return `${value > 0 ? "+" : "−"}${Math.abs(value).toFixed(3)}`;
}
