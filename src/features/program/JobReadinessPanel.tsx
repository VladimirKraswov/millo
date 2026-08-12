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
} from "lucide-react";

import type {
  JobReadinessAction,
  JobReadinessStep,
  JobReadinessStepId,
  JobReadinessView,
} from "./jobReadinessModel";

interface JobReadinessPanelProps {
  readonly busy: boolean;
  readonly details: Readonly<Record<JobReadinessStepId, string>>;
  readonly intent: "airRun" | "cutting";
  readonly onIntent: (intent: "airRun" | "cutting") => void;
  readonly onOpenOrigin: () => void;
  readonly onPrimary: (action: JobReadinessAction) => void;
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
  if (action === "resolveRecovery") return <History aria-hidden="true" size={16} />;
  if (action === "reviewProgram") return <FileWarning aria-hidden="true" size={16} />;
  return <Play aria-hidden="true" size={16} />;
}

export function JobReadinessPanel({
  busy,
  details,
  intent,
  onIntent,
  onOpenOrigin,
  onPrimary,
  view,
}: JobReadinessPanelProps) {
  const primaryLabel =
    view.primaryAction === "startProgram"
      ? intent === "cutting"
        ? "Начать гравировку"
        : "Запустить без резания"
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
            disabled={busy}
            onClick={() => onIntent("airRun")}
            type="button"
          >
            Без резания
          </button>
          <button
            aria-pressed={intent === "cutting"}
            disabled={busy}
            onClick={() => onIntent("cutting")}
            type="button"
          >
            Гравировка
          </button>
        </div>
      </header>

      <div className="job-readiness-list">
        {view.steps.map((step) => (
          <div className={`job-readiness-step is-${step.state}`} key={step.id}>
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

      <button
        className={`job-primary-action is-${view.primaryAction}`}
        disabled={busy || view.primaryDisabled}
        onClick={() => onPrimary(view.primaryAction)}
        type="button"
      >
        <PrimaryIcon action={view.primaryAction} busy={busy} />
        {primaryLabel}
      </button>
    </section>
  );
}
