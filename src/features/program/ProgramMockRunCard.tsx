import { Pause, Play, X } from "lucide-react";

import type { SenderSnapshot } from "../../shared/dryRun";
import type { DryRunControls } from "./dryRunReadModel";
import type { SenderActionLayout } from "./operatorLayoutModel";
import { senderStateLabel } from "./senderPresentationModel";
import { SenderTiming } from "./SenderTiming";

interface ProgramMockRunCardProps {
  readonly actions: SenderActionLayout;
  readonly controls: DryRunControls;
  readonly dryRunAvailable: boolean;
  readonly failure: boolean;
  readonly gatewayAvailable: boolean;
  readonly onCancel: () => void;
  readonly onPrimary: () => void;
  readonly sender: SenderSnapshot;
  readonly status: string;
}

export function ProgramMockRunCard({
  actions,
  controls,
  dryRunAvailable,
  failure,
  gatewayAvailable,
  onCancel,
  onPrimary,
  sender,
  status,
}: ProgramMockRunCardProps) {
  return (
    <div className={`dry-run-card is-${sender.state}`}>
      <div className="dry-run-heading">
        <div>
          <span>Проверка движения</span>
          <strong>{senderStateLabel(sender.state)}</strong>
        </div>
        <code>{controls.progressPercent}%</code>
      </div>
      <div
        aria-label="Прогресс проверки движения"
        aria-valuemax={100}
        aria-valuemin={0}
        aria-valuenow={controls.progressPercent}
        className="dry-run-progress"
        role="progressbar"
      >
        <i style={{ width: `${controls.progressPercent}%` }} />
      </div>
      <div className="dry-run-line">
        <span>{sender.currentSourceLine === undefined ? "Подготовка" : `L${sender.currentSourceLine}`}</span>
        <code>{sender.currentCommand ?? "M5 · M9 перед запуском"}</code>
      </div>
      <SenderTiming sender={sender} />
      <div className="dry-run-actions">
        <button
          aria-hidden={actions.primary === "none"}
          className={actions.primary === "none" ? "is-placeholder" : undefined}
          disabled={
            !gatewayAvailable ||
            actions.primary === "none" ||
            (actions.primary === "start" && !controls.canStart) ||
            (actions.primary === "resume" && !controls.canResume)
          }
          onClick={onPrimary}
          tabIndex={actions.primary === "none" ? -1 : 0}
          title={
            actions.primary === "start" && !dryRunAvailable
              ? "Подключите Mock GRBL в состоянии Idle"
              : undefined
          }
          type="button"
        >
          {actions.primary === "pause" ? (
            <Pause aria-hidden="true" size={13} />
          ) : (
            <Play aria-hidden="true" size={13} />
          )}
          {actions.primary === "pause"
            ? "Пауза"
            : actions.primary === "resume"
              ? "Продолжить"
              : "Запустить тест"}
        </button>
        <button
          aria-hidden={!actions.cancelVisible}
          className={`is-cancel${actions.cancelVisible ? "" : " is-placeholder"}`}
          disabled={!gatewayAvailable || !actions.cancelVisible}
          onClick={onCancel}
          tabIndex={actions.cancelVisible ? 0 : -1}
          type="button"
        >
          <X aria-hidden="true" size={13} />
          Отменить
        </button>
      </div>
      <small className={failure ? "is-error" : undefined}>{status}</small>
    </div>
  );
}
