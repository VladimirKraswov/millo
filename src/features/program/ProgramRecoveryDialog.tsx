import {
  Check,
  CircleAlert,
  History,
  RotateCcw,
  ShieldAlert,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";

import type {
  ProgramRecoveryCandidate,
  ProgramRecoveryPackage,
  ProgramRecoveryPreparationRequest,
  RecoveryContinuity,
  RecoveryInterruptionKind,
} from "../../shared/recovery";
import {
  canPrepareRecovery,
  emptyRecoveryPreparation,
  setRecoveryReadiness,
  type RecoveryConfirmationKey,
} from "./programRecoveryModel";

interface ProgramRecoveryDialogProps {
  readonly candidate: ProgramRecoveryCandidate;
  readonly open: boolean;
  readonly onClose: () => void;
  readonly onDismiss: (recoveryId: number) => Promise<void>;
  readonly onPrepare: (
    request: ProgramRecoveryPreparationRequest,
  ) => Promise<ProgramRecoveryPackage>;
  readonly onPrepared: (prepared: ProgramRecoveryPackage) => Promise<void> | void;
}

const checklist: ReadonlyArray<{
  key: RecoveryConfirmationKey;
  title: string;
  detail: string;
}> = [
  {
    key: "machineReferenceRestored",
    title: "Координаты станка восстановлены",
    detail: "После потери питания выполнен homing или ручная привязка осей.",
  },
  {
    key: "workZeroRestored",
    title: "Рабочий ноль выставлен заново",
    detail: "Активная G54-G59 снова совпадает с нулём исходной программы.",
  },
  {
    key: "motionPowerRestored",
    title: "Силовая часть и позиция проверены",
    detail: "Драйверы осей запитаны, а фактическая позиция не взята из одного только GRBL.",
  },
  {
    key: "restartPointInspected",
    title: "Точка возврата проверена",
    detail: "Положение и повторяемый участок сверены с заготовкой и preview.",
  },
  {
    key: "pathClear",
    title: "Маршрут Safe Z свободен",
    detail: "Подъём, переход XY и повторный проход не пересекают крепёж.",
  },
  {
    key: "powerControlReachable",
    title: "Питание доступно",
    detail: "Станок и шпиндель можно немедленно обесточить рукой.",
  },
];

const interruptionLabels: Record<RecoveryInterruptionKind, string> = {
  hostStopped: "Приложение или компьютер остановились во время выполнения",
  controllerDisconnected: "Связь с контроллером пропала во время выполнения",
  controllerReset: "Контроллер перезапустился во время выполнения",
  controllerUnresponsive: "Контроллер перестал отвечать во время выполнения",
  controllerAlarm: "Контроллер остановился с ALARM",
  programRejected: "GRBL отклонил исполняемый блок",
  operatorStopped: "Выполнение было остановлено оператором",
  unknown: "Причина остановки не доказана",
};

const continuityOptions: ReadonlyArray<{
  value: RecoveryContinuity;
  title: string;
  detail: string;
}> = [
  {
    value: "motionPowerLostOrUnknown",
    title: "Силовая часть отключалась или не уверен",
    detail: "Безопасный вариант: начать программу с начала после восстановления XYZ-ноля.",
  },
  {
    value: "controllerInterrupted",
    title: "Станок и контроллер отключились",
    detail: "Использовать последний физический Ln и повторить участок с clearance rapid.",
  },
  {
    value: "hostInterruptedMachinePowered",
    title: "Отключился только ПК или приложение",
    detail: "Станок, драйверы и контроллер непрерывно оставались под питанием.",
  },
];

export function ProgramRecoveryDialog({
  candidate,
  open,
  onClose,
  onDismiss,
  onPrepare,
  onPrepared,
}: ProgramRecoveryDialogProps) {
  const [request, setRequest] = useState(() =>
    emptyRecoveryPreparation(candidate),
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();

  useEffect(() => {
    if (!open) return;
    setRequest(emptyRecoveryPreparation(candidate));
    setBusy(false);
    setError(undefined);
  }, [candidate, open]);

  if (!open) return null;
  const readinessConfirmed = checklist.every((item) => request[item.key]);
  const continuityLabel = continuityOptions.find(
    (option) => option.value === request.continuity,
  )?.title;

  const prepare = async () => {
    if (!canPrepareRecovery(candidate, request, busy)) return;
    setBusy(true);
    setError(undefined);
    try {
      await onPrepared(await onPrepare(request));
      onClose();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const dismiss = async () => {
    if (busy) return;
    setBusy(true);
    setError(undefined);
    try {
      await onDismiss(candidate.id);
      onClose();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="machine-dialog-backdrop recovery-backdrop" role="presentation">
      <section
        aria-labelledby="recovery-title"
        aria-modal="true"
        className="machine-dialog recovery-dialog"
        role="dialog"
      >
        <header>
          <div>
            <span>Предыдущий запуск</span>
            <h2 id="recovery-title">Завершение не подтверждено</h2>
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

        <div className="recovery-dialog-body">
          <div className={`recovery-evidence${candidate.ready ? "" : " is-blocked"}`}>
            {candidate.ready ? (
              <History aria-hidden="true" size={20} />
            ) : (
              <ShieldAlert aria-hidden="true" size={20} />
            )}
            <div>
              <strong>{candidate.sourceName}</strong>
              <span>{interruptionLabels[candidate.interruption]}</span>
              <small>{candidate.detail}</small>
            </div>
            <dl>
              <div>
                <dt>Последняя строка</dt>
                <dd>{candidate.executingSourceLine ?? "нет"}</dd>
              </div>
              <div>
                <dt>Продолжение</dt>
                <dd>
                  {candidate.checkpointRestartAvailable
                    ? candidate.restartSourceLine
                    : "полный"}
                </dd>
              </div>
              <div>
                <dt>Принято строк</dt>
                <dd>{candidate.acknowledgedLines}</dd>
              </div>
              {candidate.restartPosition && (
                <div className="recovery-restart-position">
                  <dt>Restart XYZ</dt>
                  <dd>
                    {candidate.restartPosition.x.toFixed(3)} ·{" "}
                    {candidate.restartPosition.y.toFixed(3)} ·{" "}
                    {candidate.restartPosition.z.toFixed(3)}
                  </dd>
                </div>
              )}
            </dl>
          </div>

          <div className="recovery-decision">
            <CircleAlert aria-hidden="true" size={17} />
            <span>
              <strong>Что сделать с этой записью?</strong>
              <small>
                Даже если станок дошёл до конца, Millo мог не успеть сохранить финальный
                статус. Подтвердите завершение или подготовьте безопасный повторный запуск.
              </small>
            </span>
          </div>

          {candidate.ready ? (
            <>
              <details className="recovery-continuity-disclosure">
                <summary>
                  <span>Сценарий: {continuityLabel}</span>
                  <small>Изменить</small>
                </summary>
                <fieldset className="recovery-continuity">
                  <legend>Что оставалось под питанием</legend>
                  {continuityOptions.map((option) => {
                    const checkpointOption =
                      option.value !== "motionPowerLostOrUnknown";
                    const disabled =
                      busy ||
                      (checkpointOption && !candidate.checkpointRestartAvailable);
                    return (
                      <label key={option.value}>
                        <input
                          checked={request.continuity === option.value}
                          disabled={disabled}
                          name="recovery-continuity"
                          onChange={() =>
                            setRequest((current) => ({
                              ...current,
                              continuity: option.value,
                            }))
                          }
                          type="radio"
                          value={option.value}
                        />
                        <span>
                          <strong>{option.title}</strong>
                          <small>
                            {disabled && checkpointOption
                              ? "Недоступно: прошивка не сохранила физический Ln."
                              : option.detail}
                          </small>
                        </span>
                      </label>
                    );
                  })}
                </fieldset>
              </details>
              <label className="recovery-safe-z">
                <span>
                  <strong>Safe Z</strong>
                  <small>
                    Не ниже {candidate.minimumSafeZMm?.toFixed(3) ?? "?"} mm
                  </small>
                </span>
                <span>
                  <input
                    disabled={busy}
                    min={candidate.minimumSafeZMm}
                    onChange={(event) =>
                      setRequest((current) => ({
                        ...current,
                        safeZMm: Number(event.target.value),
                      }))
                    }
                    step="0.1"
                    type="number"
                    value={request.safeZMm}
                  />
                  <code>mm</code>
                </span>
              </label>
              <div className="recovery-checklist is-compact">
                <label>
                  <input
                    checked={readinessConfirmed}
                    disabled={busy}
                    onChange={(event) =>
                      setRequest((current) =>
                        setRecoveryReadiness(current, event.target.checked),
                      )
                    }
                    type="checkbox"
                  />
                  <span aria-hidden="true">
                    <Check size={13} />
                  </span>
                  <span>
                    <strong>Станок готов к восстановлению</strong>
                    <small>Координаты, питание, точка возврата и свободный маршрут проверены</small>
                  </span>
                </label>
              </div>
              <details className="confirmation-details recovery-confirmation-details">
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
              <details className="recovery-explanation">
                <summary>Почему по умолчанию выбран полный restart</summary>
                <div>
                  <CircleAlert aria-hidden="true" size={17} />
                  <p>
                    GRBL может питаться от USB и увеличивать <code>Ln:</code>, пока
                    драйверы двигателей обесточены. При сомнении Millo начинает
                    программу с начала.
                  </p>
                </div>
              </details>
            </>
          ) : (
            <div className="recovery-warning is-blocked">
              <ShieldAlert aria-hidden="true" size={17} />
              <p>
                Сохранённый исходник или его fingerprint не прошёл проверку. Эта
                запись остаётся только диагностикой и не может создать G-code.
              </p>
            </div>
          )}

          <p
            aria-hidden={!error}
            className={`first-cut-error${error ? "" : " is-empty"}`}
          >
            {error ?? "Нет ошибок"}
          </p>
        </div>
        <footer>
          <button disabled={busy} onClick={() => void dismiss()} type="button">
            <X aria-hidden="true" size={14} />
            Работа уже завершена
          </button>
          {candidate.ready && (
            <button
              className="first-cut-authorize"
              disabled={!canPrepareRecovery(candidate, request, busy)}
              onClick={() => void prepare()}
              type="button"
            >
              <RotateCcw aria-hidden="true" size={15} />
              {busy
                ? "Подготовка..."
                : request.continuity === "motionPowerLostOrUnknown"
                  ? "Подготовить повторный запуск"
                  : "Подготовить продолжение"}
            </button>
          )}
        </footer>
      </section>
    </div>
  );
}
