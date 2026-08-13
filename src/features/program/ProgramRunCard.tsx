import {
  History,
  LocateFixed,
  Pause,
  Play,
  RotateCcw,
  Square,
  Wrench,
  X,
} from "lucide-react";
import type { ReactNode } from "react";

import type { SenderSnapshot } from "../../shared/dryRun";
import type {
  CheckSenderAction,
  PhysicalSenderActionLayout,
} from "./operatorLayoutModel";
import { isSenderTerminal } from "./senderStateModel";
import { senderStateLabel } from "./senderPresentationModel";
import { SenderTiming } from "./SenderTiming";

interface ProgramRunCardProps {
  readonly busy: boolean;
  readonly checkAction: CheckSenderAction;
  readonly checkControlsAvailable: boolean;
  readonly checkRun: boolean;
  readonly failureSummary?: string;
  readonly machineContextAvailable: boolean;
  readonly onCancelCheck: () => void;
  readonly onPause: () => void;
  readonly onPrepareRerun: () => void;
  readonly onResolveInterruption: () => void;
  readonly onResume: () => void;
  readonly onReturnFromCheck: () => void;
  readonly onReturnToWorkOrigin: () => void;
  readonly onStop: () => void;
  readonly onToolChange: () => void;
  readonly physicalActions: PhysicalSenderActionLayout;
  readonly programControlsAvailable: boolean;
  readonly programRun: boolean;
  readonly progressPercent: number;
  readonly recoveryAvailable: boolean;
  readonly recoveryChecked: boolean;
  readonly sender: SenderSnapshot;
}

export function ProgramRunCard({
  busy,
  checkAction,
  checkControlsAvailable,
  checkRun,
  failureSummary,
  machineContextAvailable,
  onCancelCheck,
  onPause,
  onPrepareRerun,
  onResolveInterruption,
  onResume,
  onReturnFromCheck,
  onReturnToWorkOrigin,
  onStop,
  onToolChange,
  physicalActions,
  programControlsAvailable,
  programRun,
  progressPercent,
  recoveryAvailable,
  recoveryChecked,
  sender,
}: ProgramRunCardProps) {
  return (
    <div className={`dry-run-card program-run-card is-${sender.state}`}>
      <div className="dry-run-heading">
        <div>
          <span>{runModeLabel(sender, checkRun)}</span>
          <strong>
            {sender.state === "draining"
              ? "Ждём полной остановки станка"
              : senderStateLabel(sender.state)}
          </strong>
        </div>
        <code>{progressPercent}%</code>
      </div>
      <div
        aria-label="Прогресс выполнения программы"
        aria-valuemax={100}
        aria-valuemin={0}
        aria-valuenow={progressPercent}
        className="dry-run-progress"
        role="progressbar"
      >
        <i style={{ width: `${progressPercent}%` }} />
      </div>
      <div className="dry-run-line">
        <span>
          {sender.currentSourceLine === undefined ? "Подготовка" : `L${sender.currentSourceLine}`}
        </span>
        <code>{sender.currentCommand ?? "M5 · M9 перед запуском"}</code>
      </div>
      <SenderTiming sender={sender} />
      <div className="dry-run-actions">
        {programRun && programControlsAvailable && physicalActions.primary === "pause" && (
          <ActionButton
            busy={busy}
            icon={<Pause aria-hidden="true" size={13} />}
            label="Пауза"
            onClick={onPause}
          />
        )}
        {programRun && programControlsAvailable && physicalActions.primary === "resume" && (
          <ActionButton
            busy={busy}
            icon={<Play aria-hidden="true" size={13} />}
            label="Продолжить"
            onClick={onResume}
          />
        )}
        {programRun && programControlsAvailable && physicalActions.primary === "toolChange" && (
          <ActionButton
            busy={busy}
            icon={<Wrench aria-hidden="true" size={13} />}
            label="Подтвердить замену"
            onClick={onToolChange}
          />
        )}
        {programRun && programControlsAvailable && physicalActions.stopVisible && (
          <button
            aria-label="Остановить текущее задание"
            className="is-cancel"
            disabled={busy}
            onClick={onStop}
            title="Feed Hold, затем Soft Reset; незавершённую работу можно восстановить или закрыть"
            type="button"
          >
            <Square aria-hidden="true" size={13} />
            Остановить
          </button>
        )}
        {checkRun && checkControlsAvailable && checkAction === "cancel" && (
          <button className="is-cancel" disabled={busy} onClick={onCancelCheck} type="button">
            <X aria-hidden="true" size={13} />
            Отменить проверку
          </button>
        )}
        {checkRun && checkAction === "returnToPreparation" && (
          <button
            className="is-terminal-action"
            disabled={busy}
            onClick={onReturnFromCheck}
            type="button"
          >
            <RotateCcw aria-hidden="true" size={13} />
            Вернуться к подготовке
          </button>
        )}
        {programRun && isSenderTerminal(sender.state) && machineContextAvailable && (
          <div className="sender-zero-return" aria-label="Возврат к рабочему нулю">
            <span>После остановки</span>
            <button
              disabled={busy}
              onClick={onReturnToWorkOrigin}
              title="Millo поднимет Z, вернёт XY и только затем опустит Z к рабочему нулю"
              type="button"
            >
              <LocateFixed aria-hidden="true" size={13} />
              Вернуться в рабочий ноль
            </button>
          </div>
        )}
        {programRun && physicalActions.primary === "prepareRerun" && (
          <button
            className="is-terminal-action"
            disabled={busy}
            onClick={onPrepareRerun}
            type="button"
          >
            <RotateCcw aria-hidden="true" size={13} />
            Подготовить повторный запуск
          </button>
        )}
        {programRun && physicalActions.primary === "resolveInterruption" && (
          <button
            className="is-terminal-action"
            disabled={!recoveryChecked}
            onClick={onResolveInterruption}
            type="button"
          >
            <History aria-hidden="true" size={13} />
            {!recoveryChecked
              ? "Сохраняем остановку..."
              : recoveryAvailable
                ? "Продолжить или начать заново"
                : "Подготовить новый запуск"}
          </button>
        )}
      </div>
      <small>{runStatusDetail(sender, checkRun, failureSummary)}</small>
    </div>
  );
}

function ActionButton({
  busy,
  icon,
  label,
  onClick,
}: {
  readonly busy: boolean;
  readonly icon: ReactNode;
  readonly label: string;
  readonly onClick: () => void;
}) {
  return (
    <button disabled={busy} onClick={onClick} type="button">
      {icon}
      {label}
    </button>
  );
}

const runModeLabel = (sender: SenderSnapshot, checkRun: boolean): string =>
  checkRun ? "Проверка GRBL" : sender.mode === "airRun" ? "Проверка движения" : "Обработка";

const runStatusDetail = (
  sender: SenderSnapshot,
  checkRun: boolean,
  failureSummary: string | undefined,
): string => {
  switch (sender.state) {
    case "completed":
      return checkRun
        ? "Все строки приняты в $C; контроллер вернулся в Idle"
        : "Возврат: сначала безопасно поднимите Z, затем X0/Y0; Z0 выполняйте последним";
    case "failed":
      return failureSummary ?? "Выполнение остановлено";
    case "toolChange":
      return `M6 удерживается приложением${sender.requestedTool === undefined ? "" : ` · требуется T${sender.requestedTool}`}`;
    case "paused":
      return "Задание на паузе: продолжите его или завершите, чтобы освободить Jog";
    case "cancelled":
      return checkRun
        ? "Проверка остановлена; вернитесь к подготовке и запустите её снова"
        : "Задание завершено оператором; выберите восстановление или новый запуск";
    default:
      return checkRun
        ? "По одной строке · без движения и включения выходов"
        : "Пауза сохраняет продолжение; завершение останавливает поток через Hold и Reset";
  }
};
