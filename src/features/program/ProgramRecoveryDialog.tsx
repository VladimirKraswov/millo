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
} from "../../shared/recovery";
import {
  canPrepareRecovery,
  emptyRecoveryPreparation,
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
            <span>Interrupted job</span>
            <h2 id="recovery-title">Восстановление программы</h2>
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

        <div className={`recovery-evidence${candidate.ready ? "" : " is-blocked"}`}>
          {candidate.ready ? (
            <History aria-hidden="true" size={20} />
          ) : (
            <ShieldAlert aria-hidden="true" size={20} />
          )}
          <div>
            <strong>{candidate.sourceName}</strong>
            <span>{candidate.detail}</span>
          </div>
          <dl>
            <div>
              <dt>GRBL Ln</dt>
              <dd>{candidate.executingSourceLine ?? "нет"}</dd>
            </div>
            <div>
              <dt>Restart</dt>
              <dd>{candidate.restartSourceLine ?? "blocked"}</dd>
            </div>
            <div>
              <dt>Accepted</dt>
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

        {candidate.ready ? (
          <>
            <div className="recovery-warning">
              <CircleAlert aria-hidden="true" size={17} />
              <p>
                Это новый запуск с более ранней безопасной точки. Уже обработанный
                участок будет пройден повторно; автоматического движения сейчас нет.
              </p>
            </div>
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
            <div className="recovery-checklist">
              {checklist.map((item) => (
                <label key={item.key}>
                  <input
                    checked={request[item.key]}
                    disabled={busy}
                    onChange={(event) =>
                      setRequest((current) => ({
                        ...current,
                        [item.key]: event.target.checked,
                      }))
                    }
                    type="checkbox"
                  />
                  <span aria-hidden="true"><Check size={13} /></span>
                  <span>
                    <strong>{item.title}</strong>
                    <small>{item.detail}</small>
                  </span>
                </label>
              ))}
            </div>
          </>
        ) : (
          <div className="recovery-warning is-blocked">
            <ShieldAlert aria-hidden="true" size={17} />
            <p>
              Без физического `Ln:` нельзя отличить выполненные движения от блоков,
              которые GRBL только принял в очередь. Автоматический restart отключён.
            </p>
          </div>
        )}

        {error && <p className="first-cut-error">{error}</p>}
        <footer>
          <button disabled={busy} onClick={() => void dismiss()} type="button">
            <X aria-hidden="true" size={14} />
            Не восстанавливать
          </button>
          {candidate.ready && (
            <button
              className="first-cut-authorize"
              disabled={!canPrepareRecovery(candidate, request, busy)}
              onClick={() => void prepare()}
              type="button"
            >
              <RotateCcw aria-hidden="true" size={15} />
              {busy ? "Подготовка..." : "Создать recovery program"}
            </button>
          )}
        </footer>
      </section>
    </div>
  );
}
