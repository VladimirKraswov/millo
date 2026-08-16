import {
  Check,
  CircleAlert,
  Power,
  RefreshCw,
  Waves,
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
  readonly surfaceMap?: {
    readonly mapId: number;
    readonly enabled: boolean;
    readonly usable: boolean;
    readonly coversProgram: boolean;
    readonly zRangeMm: number;
    readonly suspiciousNeighborJump: boolean;
    readonly maximumNeighborDeltaMm: number;
    readonly detail: string;
    readonly busy: boolean;
    readonly onApply: (enabled: boolean) => Promise<void>;
  };
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
  surfaceMap,
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
  const [surfaceMapSelected, setSurfaceMapSelected] = useState(
    surfaceMap?.enabled ?? false,
  );
  const [surfaceQualityConfirmed, setSurfaceQualityConfirmed] = useState(false);

  useEffect(() => {
    if (!open) return;
    setConfirmation({ ...emptyFirstCutConfirmation, intent, executionOptions });
    setSurfaceMapSelected(surfaceMap?.enabled ?? false);
    setSurfaceQualityConfirmed(false);
    setBusy(false);
    setError(undefined);
  }, [open, intent, report?.programFingerprint]);

  if (!open) return null;

  const operationBusy = busy || surfaceMap?.busy === true;
  const controls: FirstCutAuthorizationControls = firstCutAuthorizationControls(
    confirmation,
    { report, gatewayAvailable: true, busy: operationBusy },
  );
  const surfaceMapCanChange = surfaceMap?.usable === true && surfaceMap.coversProgram;
  const surfaceMapSelectionChanged = intent === "cutting" &&
    surfaceMap !== undefined &&
    surfaceMapSelected !== surfaceMap.enabled;
  const primaryLabel = operationBusy
    ? surfaceMapSelectionChanged
      ? "Применяем и проверяем..."
      : "Проверка и запуск..."
    : surfaceMapSelectionChanged
      ? surfaceMapSelected
        ? "Включить карту и перепроверить"
        : "Отключить карту и перепроверить"
      : intent === "airRun"
        ? "Начать проверку движения"
        : surfaceMap && !surfaceMapSelected && surfaceMapCanChange
          ? "Начать обработку без карты"
          : "Начать обработку";

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

  const applySurfaceMapSelection = async () => {
    if (!surfaceMap || !surfaceMapCanChange || !surfaceMapSelectionChanged) return;
    setBusy(true);
    setError(undefined);
    try {
      await surfaceMap.onApply(surfaceMapSelected);
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
  const suspiciousSurfaceMapSelected = intent === "cutting" &&
    hasSurfaceMap &&
    surfaceMap?.suspiciousNeighborJump === true;
  const canAuthorize = controls.canAuthorize &&
    (!suspiciousSurfaceMapSelected || surfaceQualityConfirmed);

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
            disabled={operationBusy}
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
        {intent === "cutting" && surfaceMap && (
          <div className={`first-cut-surface-map ${surfaceMapStatusClass(
            surfaceMap,
            surfaceMapSelected,
          )}`}>
            <Waves aria-hidden="true" size={18} />
            <span>
              <strong>{surfaceMapStatusTitle(surfaceMap, surfaceMapSelected)}</strong>
              <small>{surfaceMap.detail}</small>
              {!surfaceMapSelected && surfaceMapCanChange && (
                <em>
                  Без компенсации перепад поверхности до {surfaceMap.zRangeMm.toFixed(3)} мм
                  может изменить фактическую глубину обработки.
                </em>
              )}
              {surfaceMapSelectionChanged && (
                <em>После изменения Millo повторит GRBL Check без движения.</em>
              )}
            </span>
            <label>
              <input
                aria-label="Компенсировать траекторию по карте высот"
                checked={surfaceMapSelected}
                disabled={operationBusy || !surfaceMapCanChange}
                onChange={(event) => setSurfaceMapSelected(event.target.checked)}
                role="switch"
                type="checkbox"
              />
              <small>Компенсация</small>
            </label>
          </div>
        )}
        <div className="first-cut-checklist">
          <label>
            <input
              checked={setupReady}
              disabled={operationBusy}
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
                disabled={operationBusy}
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
          {suspiciousSurfaceMapSelected && surfaceMap && (
            <label className="is-surface-warning">
              <input
                checked={surfaceQualityConfirmed}
                disabled={operationBusy}
                onChange={(event) => setSurfaceQualityConfirmed(event.target.checked)}
                type="checkbox"
              />
              <span aria-hidden="true" className="first-cut-checkmark"><Check size={13} /></span>
              <span>
                <strong>Резкий перепад карты проверен</strong>
                <small>
                  Между соседними точками до {surfaceMap.maximumNeighborDeltaMm.toFixed(3)} мм.
                  Контакт щупа был надёжным, фреза и её вылет после измерения не менялись.
                </small>
              </span>
            </label>
          )}
          <label>
            <input
              checked={intent === "airRun"
                ? confirmation.manualSpindleOff
                : confirmation.manualSpindleRunning}
              disabled={operationBusy}
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
          <button disabled={operationBusy} onClick={onClose} type="button">Отмена</button>
          <button
            className="first-cut-authorize"
            disabled={surfaceMapSelectionChanged
              ? operationBusy || !surfaceMapCanChange
              : !canAuthorize}
            onClick={() => void (surfaceMapSelectionChanged
              ? applySurfaceMapSelection()
              : authorizeAndStart())}
            type="button"
          >
            {surfaceMapSelectionChanged
              ? <RefreshCw aria-hidden="true" size={15} />
              : <Power aria-hidden="true" size={15} />}
            {primaryLabel}
          </button>
        </footer>
      </section>
    </div>
  );
}

function surfaceMapStatusClass(
  surfaceMap: NonNullable<FirstCutAuthorizationDialogProps["surfaceMap"]>,
  selected: boolean,
): string {
  if (!surfaceMap.usable || !surfaceMap.coversProgram) return "is-unavailable";
  if (selected !== surfaceMap.enabled) return "is-pending";
  if (selected && surfaceMap.suspiciousNeighborJump) return "is-suspicious";
  return selected ? "is-enabled" : "is-warning";
}

function surfaceMapStatusTitle(
  surfaceMap: NonNullable<FirstCutAuthorizationDialogProps["surfaceMap"]>,
  selected: boolean,
): string {
  if (!surfaceMap.usable) return `Карта #${surfaceMap.mapId} устарела`;
  if (!surfaceMap.coversProgram) return `Карта #${surfaceMap.mapId} не покрывает задание`;
  if (selected !== surfaceMap.enabled) {
    return selected
      ? `Карта #${surfaceMap.mapId} будет включена`
      : `Карта #${surfaceMap.mapId} будет отключена`;
  }
  return selected
    ? surfaceMap.suspiciousNeighborJump
      ? `Карта #${surfaceMap.mapId} требует проверки`
      : `Карта #${surfaceMap.mapId} применяется`
    : `Карта #${surfaceMap.mapId} найдена, но не применяется`;
}

function formatSignedOffset(value: number): string {
  if (Math.abs(value) < 0.0005) return "0.000";
  return `${value > 0 ? "+" : "−"}${Math.abs(value).toFixed(3)}`;
}
