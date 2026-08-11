import {
  Check,
  CircleAlert,
  Power,
  ShieldCheck,
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
  readonly report?: RunPreflightReport;
  readonly onAuthorize: (
    confirmation: FirstCutConfirmation,
  ) => Promise<FirstCutPreparation>;
  readonly onAuthorized: (preparation: FirstCutPreparation) => void;
  readonly onStart: (preparation: FirstCutPreparation) => Promise<SenderSnapshot>;
  readonly onStarted: (snapshot: SenderSnapshot) => void;
  readonly onClose: () => void;
}

type ConfirmationKey = Exclude<
  keyof FirstCutConfirmation,
  "intent" | "executionOptions"
>;

const commonChecklist: ReadonlyArray<{
  key: ConfirmationKey;
  title: string;
  detail: string;
}> = [
  {
    key: "xyzZeroVerified",
    title: "Ноль XYZ проверен",
    detail: "Рабочий ноль активной G54-G59 совпадает с нулём программы.",
  },
  {
    key: "safeZVerified",
    title: "Safe Z свободен",
    detail: "Все перемещения Z проходят выше заготовки, крепежа и оснастки.",
  },
  {
    key: "pathClear",
    title: "Габарит траектории свободен",
    detail: "Preview проверен; движения XYZ не пересекают упоры, прижимы и раму.",
  },
  {
    key: "powerControlReachable",
    title: "Питание доступно",
    detail: "Можно немедленно обесточить шпиндель и станок рукой.",
  },
];

const cuttingChecklist: ReadonlyArray<{
  key: ConfirmationKey;
  title: string;
  detail: string;
}> = [
  {
    key: "stockSecured",
    title: "Заготовка закреплена",
    detail: "Прижимы затянуты и не пересекают траекторию инструмента.",
  },
  {
    key: "toolSecured",
    title: "Инструмент установлен",
    detail: "Фреза соответствует программе и надёжно зажата в цанге.",
  },
  {
    key: "manualSpindleRunning",
    title: "Ручной шпиндель запущен",
    detail: "Вращение включено вручную, направление и звук проверены.",
  },
];

const airRunChecklist: ReadonlyArray<{
  key: ConfirmationKey;
  title: string;
  detail: string;
}> = [
  {
    key: "toolRemoved",
    title: "Инструмент снят",
    detail: "В цанге нет фрезы; случайное касание заготовки исключено.",
  },
  {
    key: "manualSpindleOff",
    title: "Шпиндель выключен",
    detail: "Ручное питание шпинделя отключено; sender дополнительно начинает с M5/M9.",
  },
];

export function FirstCutAuthorizationDialog({
  open,
  intent,
  executionOptions,
  report,
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
  const [preparation, setPreparation] = useState<FirstCutPreparation>();

  useEffect(() => {
    if (!open) return;
    setConfirmation({ ...emptyFirstCutConfirmation, intent, executionOptions });
    setBusy(false);
    setError(undefined);
    setPreparation(undefined);
  }, [executionOptions, open, intent, report?.programFingerprint]);

  if (!open) return null;

  const controls: FirstCutAuthorizationControls = firstCutAuthorizationControls(
    confirmation,
    { report, gatewayAvailable: true, busy },
  );

  const authorize = async () => {
    if (!controls.canAuthorize) return;
    setBusy(true);
    setError(undefined);
    try {
      const next = await onAuthorize(confirmation);
      setPreparation(next);
      onAuthorized(next);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const start = async () => {
    if (!preparation || busy) return;
    setBusy(true);
    setError(undefined);
    try {
      onStarted(await onStart(preparation));
      onClose();
    } catch (reason) {
      setPreparation(undefined);
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const checklist = [
    ...(confirmation.intent === "airRun" ? airRunChecklist : cuttingChecklist),
    ...commonChecklist,
  ];

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
            <span>One-time safety gate</span>
            <h2 id="first-cut-title">Запуск программы</h2>
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

        {preparation ? (
          <div className="first-cut-authorized" role="status">
            <ShieldCheck aria-hidden="true" size={30} />
            <strong>Одноразовое разрешение выпущено</strong>
            <span>
              Authorization #{preparation.authorization.id} ·{" "}
              {Math.round(preparation.authorization.expiresInMs / 1_000)} секунд
            </span>
            <p>
              Оно привязано к этой программе и текущей позиции. Любое движение,
              reset, reconnect или истечение времени отменит его.
            </p>
            <small>G-code ещё не отправлялся. Следующее нажатие атомарно потребит разрешение.</small>
            <button
              className="program-run-start"
              disabled={busy}
              onClick={() => void start()}
              type="button"
            >
              <Power aria-hidden="true" size={15} />
              {busy
                ? "Запуск..."
                : preparation.authorization.intent === "airRun"
                  ? "Начать Air run"
                  : "Начать обработку"}
            </button>
          </div>
        ) : (
          <>
            <div className="first-cut-intro">
              <CircleAlert aria-hidden="true" size={18} />
              <div>
                <strong>Проверьте станок непосредственно перед запуском</strong>
                <span>
                  После подтверждения backend повторит полный serial preflight.
                </span>
              </div>
              <code>{controls.completedCount}/{controls.totalCount}</code>
            </div>
            <div className="program-run-mode-summary">
              <span>Режим</span>
              <strong>{intent === "airRun" ? "Air run" : "Обработка с инструментом"}</strong>
            </div>
            <div className="first-cut-checklist">
              {checklist.map((item) => (
                <label key={item.key}>
                  <input
                    checked={confirmation[item.key]}
                    disabled={busy}
                    onChange={(event) =>
                      setConfirmation((current) => ({
                        ...current,
                        [item.key]: event.target.checked,
                      }))
                    }
                    type="checkbox"
                  />
                  <span aria-hidden="true" className="first-cut-checkmark">
                    <Check size={13} />
                  </span>
                  <span>
                    <strong>{item.title}</strong>
                    <small>{item.detail}</small>
                  </span>
                </label>
              ))}
            </div>
            {error && <p className="first-cut-error">{error}</p>}
            <footer>
              <span>
                Разрешение одноразовое и действует 30 секунд. Движения сейчас не будет.
              </span>
              <button
                className="first-cut-authorize"
                disabled={!controls.canAuthorize}
                onClick={() => void authorize()}
                type="button"
              >
                <Power aria-hidden="true" size={15} />
                {busy ? "Повторная проверка..." : "Авторизовать запуск"}
              </button>
            </footer>
          </>
        )}
      </section>
    </div>
  );
}
