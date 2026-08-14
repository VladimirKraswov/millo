import {
  CircleAlert,
  CircleCheck,
  CircleDashed,
  Crosshair,
  FileWarning,
  History,
  KeyRound,
  Play,
  PlugZap,
  RefreshCw,
  ScanSearch,
  Waves,
} from "lucide-react";
import { useEffect, useState } from "react";

import type {
  JobReadinessAction,
  JobReadinessStep,
  JobReadinessStepId,
  JobReadinessView,
} from "./jobReadinessModel";
import { depthAdjustmentFromDraft } from "./depthCorrectionModel";

interface JobReadinessPanelProps {
  readonly busy: boolean;
  readonly details: Readonly<Record<JobReadinessStepId, string>>;
  readonly intent: "airRun" | "cutting";
  readonly intentLocked?: boolean;
  readonly onIntent: (intent: "airRun" | "cutting") => void;
  readonly onOpenOrigin: () => void;
  readonly onPrimary: (action: JobReadinessAction) => void;
  readonly onSurfaceMap?: (enabled: boolean) => void;
  readonly depthCorrection?: {
    readonly available: boolean;
    readonly enabled: boolean;
    readonly adjustmentMm: number;
    readonly minimumAdjustmentMm: number;
    readonly maximumAdjustmentMm: number;
  };
  readonly onDepthCorrectionEnabled?: (enabled: boolean) => void;
  readonly onDepthAdjustment?: (adjustmentMm: number) => void;
  readonly surfaceMap?: {
    readonly checked: boolean;
    readonly detail: string;
    readonly disabled: boolean;
    readonly warning: boolean;
  };
  readonly view: JobReadinessView;
}

const titles: Readonly<Record<JobReadinessStepId, string>> = {
  machine: "Станок",
  file: "Файл",
  origin: "Рабочий ноль",
  validation: "Проверка",
};

function StepIcon({ step }: { readonly step: JobReadinessStep }) {
  if (step.state === "ready") return <CircleCheck aria-hidden="true" size={15} />;
  if (step.state === "pending") return <CircleDashed aria-hidden="true" size={15} />;
  return <CircleAlert aria-hidden="true" size={15} />;
}

function PrimaryIcon({ action, busy }: { readonly action: JobReadinessAction; readonly busy: boolean }) {
  if (busy) return <RefreshCw aria-hidden="true" className="is-spinning" size={16} />;
  if (action === "connect") return <PlugZap aria-hidden="true" size={16} />;
  if (action === "unlock") return <KeyRound aria-hidden="true" size={16} />;
  if (action === "setWorkZero") return <Crosshair aria-hidden="true" size={16} />;
  if (action === "runGrblCheck") return <ScanSearch aria-hidden="true" size={16} />;
  if (action === "syncMachine") return <RefreshCw aria-hidden="true" size={16} />;
  if (action === "resolveRecovery") return <History aria-hidden="true" size={16} />;
  if (action === "reviewProgram") return <FileWarning aria-hidden="true" size={16} />;
  return <Play aria-hidden="true" size={16} />;
}

export function JobReadinessPanel({
  busy,
  details,
  intent,
  intentLocked = false,
  onIntent,
  depthCorrection,
  onDepthCorrectionEnabled,
  onDepthAdjustment,
  onOpenOrigin,
  onPrimary,
  onSurfaceMap,
  surfaceMap,
  view,
}: JobReadinessPanelProps) {
  const [depthAdjustmentDraft, setDepthAdjustmentDraft] = useState(
    formatDepthAdjustmentInput(depthCorrection?.adjustmentMm ?? 0),
  );

  useEffect(() => {
    setDepthAdjustmentDraft(formatDepthAdjustmentInput(depthCorrection?.adjustmentMm ?? 0));
  }, [depthCorrection?.adjustmentMm, depthCorrection?.enabled]);

  const primaryLabel =
    view.primaryAction === "startProgram"
      ? intent === "cutting"
        ? "Начать обработку"
        : "Запустить проверку движения"
      : view.primaryLabel;

  return (
    <section className="job-readiness" aria-labelledby="job-readiness-title">
      <header>
        <div>
          <span>Текущая работа</span>
          <strong id="job-readiness-title">Готовность к запуску</strong>
        </div>
        <div aria-label="Режим выполнения" className="program-run-intent" role="group">
          <button
            aria-pressed={intent === "airRun"}
            disabled={busy || intentLocked}
            onClick={() => onIntent("airRun")}
            type="button"
          >
            Проверка движения
          </button>
          <button
            aria-pressed={intent === "cutting"}
            disabled={busy || intentLocked}
            onClick={() => onIntent("cutting")}
            type="button"
          >
            Обработка
          </button>
        </div>
      </header>

      {surfaceMap && (
        <label className={`job-surface-map${surfaceMap.warning ? " is-warning" : ""}`}>
          <Waves aria-hidden="true" size={15} />
          <span>
            <strong>Компенсировать по карте</strong>
            <small>{surfaceMap.detail}</small>
          </span>
          <input
            aria-label="Применить карту высот к заданию"
            checked={surfaceMap.checked}
            disabled={busy || surfaceMap.disabled}
            onChange={(event) => onSurfaceMap?.(event.target.checked)}
            role="switch"
            type="checkbox"
          />
        </label>
      )}

      {intent === "cutting" && depthCorrection?.available && (
        <div className={`job-depth-correction${depthCorrection.enabled ? " is-enabled" : ""}`}>
          <label>
            <span>
              <strong>Коррекция глубины</strong>
              <small>
                {depthCorrection.enabled
                  ? `ΔZ ${formatSignedOffset(depthCorrection.adjustmentMm)} мм`
                  : "Исходная глубина без изменений"}
              </small>
            </span>
            <input
              aria-label="Включить коррекцию глубины"
              checked={depthCorrection.enabled}
              disabled={busy}
              onChange={(event) => onDepthCorrectionEnabled?.(event.target.checked)}
              role="switch"
              type="checkbox"
            />
          </label>
          <div className="job-depth-value" aria-hidden={!depthCorrection.enabled}>
            <span>Смещение Z</span>
            <input
              aria-label="Смещение глубины обработки"
              disabled={busy || !depthCorrection.enabled}
              max={depthCorrection.maximumAdjustmentMm}
              min={depthCorrection.minimumAdjustmentMm}
              onBlur={() => setDepthAdjustmentDraft(
                formatDepthAdjustmentInput(depthCorrection.adjustmentMm),
              )}
              onChange={(event) => {
                const draft = event.target.value;
                setDepthAdjustmentDraft(draft);
                const adjustmentMm = depthAdjustmentFromDraft(draft);
                if (adjustmentMm !== undefined) onDepthAdjustment?.(adjustmentMm);
              }}
              step="0.01"
              type="number"
              value={depthAdjustmentDraft}
            />
            <code>мм</code>
          </div>
        </div>
      )}

      <button
        className={`job-primary-action is-${view.primaryAction}`}
        disabled={busy || view.primaryDisabled}
        onClick={() => onPrimary(view.primaryAction)}
        type="button"
      >
        <PrimaryIcon action={view.primaryAction} busy={busy} />
        {primaryLabel}
      </button>

      <div className="job-readiness-list">
        {view.steps.map((step) => (
          <div
            aria-label={`${titles[step.id]}: ${details[step.id]}`}
            className={`job-readiness-step is-${step.state}`}
            key={step.id}
            title={details[step.id]}
          >
            <StepIcon step={step} />
            <span>
              <strong>{titles[step.id]}</strong>
              <small>{details[step.id]}</small>
            </span>
            {step.id === "origin" && step.state !== "pending" && (
              <button
                aria-label="Изменить рабочий ноль"
                disabled={busy}
                onClick={onOpenOrigin}
                title="Изменить рабочий ноль"
                type="button"
              >
                <Crosshair aria-hidden="true" size={14} />
              </button>
            )}
          </div>
        ))}
      </div>
    </section>
  );
}

function formatSignedOffset(value: number): string {
  if (Math.abs(value) < 0.0005) return "0.000";
  return `${value > 0 ? "+" : "−"}${Math.abs(value).toFixed(3)}`;
}

function formatDepthAdjustmentInput(value: number): string {
  return value.toFixed(3);
}
