import {
  Box,
  Check,
  Crosshair,
  Grid3X3,
  Pause,
  Play,
  KeyRound,
  ScanLine,
  SlidersHorizontal,
  Square,
  Table2,
  Trash2,
  WandSparkles,
} from "lucide-react";
import { lazy, Suspense, useEffect, useMemo, useRef, useState } from "react";

import type { HeightmapGateway } from "../../platform/machine/HeightmapGateway";
import type { ZProbeGateway } from "../../platform/machine/ZProbeGateway";
import type { ControllerSnapshot, MachineTravel, Position } from "../../shared/machine";
import type { HeightmapOperationSnapshot, HeightmapPlanRequest, SurfaceSession } from "../../shared/heightmap";
import type { GcodeProgram } from "../../shared/program";
import {
  defaultHeightmapRequest,
  emptyHeightmapOperation,
  emptySurfaceSession,
} from "./heightmapDefaults";
import {
  initialHeightmapDraft,
  loadHeightmapDraft,
  saveHeightmapDraft,
} from "./heightmapDraftStore";
import { HeightmapValues } from "./HeightmapValues";
import {
  applyDensity,
  describeHeightmapFailure,
  estimateHeightmapSeconds,
  heightmapCalibrationPlateThickness,
  heightmapSafeWorkZ,
  heightmapSurfaceVariation,
  perimeterFromProgram,
  validateHeightmapRequest,
  withHeightmapSurfaceVariation,
  type HeightmapDensity,
} from "./heightmapModel";

const HeightmapScene = lazy(async () => {
  const module = await import("./HeightmapScene");
  return { default: module.HeightmapScene };
});

interface HeightmapPanelProps {
  readonly desktopRuntime: boolean;
  readonly disabled?: boolean;
  readonly gateway: HeightmapGateway;
  readonly zProbeGateway: ZProbeGateway;
  readonly machineProfileId?: string;
  readonly machineTravel?: MachineTravel;
  readonly onAbort: () => Promise<ControllerSnapshot>;
  readonly onError: (message?: string) => void;
  readonly onActivityChange?: (active: boolean) => void;
  readonly onSnapshot: (snapshot: ControllerSnapshot) => void;
  readonly onSaveMode: () => Promise<void>;
  readonly onUnlock: () => Promise<ControllerSnapshot>;
  readonly program?: GcodeProgram;
  readonly snapshot: ControllerSnapshot;
}

const duration = (seconds: number): string => {
  const minutes = Math.ceil(seconds / 60);
  return minutes < 60 ? `до ${minutes} мин` : `до ${Math.floor(minutes / 60)} ч ${minutes % 60} мин`;
};

const requestFromSession = (session: SurfaceSession): HeightmapPlanRequest | undefined =>
  session.pending?.operation.map?.plan.request ?? session.active?.map.plan.request;

interface SurfaceCalibration {
  readonly calibrationPlateThicknessMm: number;
  readonly contactMode: HeightmapPlanRequest["contactMode"];
  readonly finalWorkZ: number;
  readonly perimeter: Pick<HeightmapPlanRequest, "originXMm" | "originYMm" | "widthMm" | "heightMm">;
  readonly resetCount: number;
  readonly workCoordinateOffset?: Position;
}

const samePosition = (left: Position, right: Position): boolean =>
  Math.abs(left.x - right.x) <= 0.001 &&
  Math.abs(left.y - right.y) <= 0.001 &&
  Math.abs(left.z - right.z) <= 0.001;

export function HeightmapPanel({
  desktopRuntime,
  disabled = false,
  gateway,
  zProbeGateway,
  machineProfileId,
  machineTravel,
  onAbort,
  onActivityChange,
  onError,
  onSnapshot,
  onSaveMode,
  onUnlock,
  program,
  snapshot,
}: HeightmapPanelProps) {
  const initialDraft = useMemo(
    () => initialHeightmapDraft(machineProfileId),
    [machineProfileId],
  );
  const hadStoredDraft = useRef(Boolean(loadHeightmapDraft(machineProfileId))).current;
  const [request, setRequest] = useState(() => initialDraft.request);
  const [operation, setOperation] = useState<HeightmapOperationSnapshot>(emptyHeightmapOperation);
  const [runtimeReady, setRuntimeReady] = useState(!desktopRuntime);
  const [session, setSession] = useState<SurfaceSession>(emptySurfaceSession);
  const [density, setDensity] = useState<HeightmapDensity>(() => initialDraft.density);
  const [margin, setMargin] = useState(() => initialDraft.marginMm);
  const [surfaceSearchMm, setSurfaceSearchMm] = useState(() => initialDraft.surfaceSearchMm);
  const [zeroPlateThicknessMm, setZeroPlateThicknessMm] = useState(() => initialDraft.zeroPlateThicknessMm);
  const [surfaceCalibration, setSurfaceCalibration] = useState<SurfaceCalibration>();
  const [representation, setRepresentation] = useState<"surface" | "values">("surface");
  const [view, setView] = useState<"top" | "iso">("iso");
  const [showPerimeter, setShowPerimeter] = useState(true);
  const [showJob, setShowJob] = useState(true);
  const [showProbeGrid, setShowProbeGrid] = useState(true);
  const [showInterpolation, setShowInterpolation] = useState(true);
  const [showInterpolationGrid, setShowInterpolationGrid] = useState(false);
  const [interpolationColumns, setInterpolationColumns] = useState(50);
  const [interpolationRows, setInterpolationRows] = useState(50);
  const [busy, setBusy] = useState(false);
  const [localError, setLocalError] = useState<string>();

  useEffect(() => {
    saveHeightmapDraft(machineProfileId, {
      schemaVersion: 2,
      request,
      density,
      marginMm: margin,
      surfaceSearchMm,
      zeroPlateThicknessMm,
    });
  }, [density, machineProfileId, margin, request, surfaceSearchMm, zeroPlateThicknessMm]);

  useEffect(() => {
    if (!desktopRuntime) return;
    setRuntimeReady(false);
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    void Promise.all([gateway.getSession(), gateway.getOperation()])
      .then(([nextSession, nextOperation]) => {
        if (disposed) return;
        setSession(nextSession);
        setOperation(nextOperation);
        setRuntimeReady(true);
        if (!hadStoredDraft) setRequest(requestFromSession(nextSession) ?? defaultHeightmapRequest());
      })
      .catch((error) => {
        if (disposed) return;
        setRuntimeReady(true);
        setLocalError(String(error));
      });
    void gateway.subscribeSession((next) => {
      if (!disposed) setSession(next);
    }).then((unlisten) => unlisteners.push(unlisten));
    void gateway.subscribeOperation((next) => {
      if (!disposed) setOperation(next);
    }).then((unlisten) => unlisteners.push(unlisten));
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [desktopRuntime, gateway, hadStoredDraft, machineProfileId]);

  useEffect(() => {
    if (surfaceCalibration && (
      surfaceCalibration.resetCount !== snapshot.resetCount ||
      snapshot.connection !== "connected"
    )) {
      setSurfaceCalibration(undefined);
    }
  }, [machineProfileId, snapshot.connection, snapshot.resetCount, surfaceCalibration]);

  const active = operation.state === "running" || operation.state === "paused";
  useEffect(() => {
    onActivityChange?.(active || busy);
    return () => onActivityChange?.(false);
  }, [active, busy, onActivityChange]);
  const displayedMap = active || operation.state === "failed" || operation.state === "cancelled"
    ? operation.map ?? session.pending?.operation.map ?? session.active?.map
    : session.pending?.operation.map ?? session.active?.map;
  const validationError = validateHeightmapRequest(request, machineTravel);
  const controlsBlocked = disabled || busy || !runtimeReady;
  const settingsLocked = active || controlsBlocked;
  const calibrationPlateThicknessMm = heightmapCalibrationPlateThickness(
    request,
    zeroPlateThicknessMm,
  );
  const contactConfigurationError = request.contactMode === "fixedPlate" && (
    !Number.isFinite(request.contactOffsetMm) || request.contactOffsetMm < 0.01 || request.contactOffsetMm > 100
  ) ? "Укажите толщину сплошной пластины от 0.01 до 100 mm" : undefined;
  const surfaceReady = Boolean(surfaceCalibration &&
    surfaceCalibration.contactMode === request.contactMode &&
    Math.abs(surfaceCalibration.calibrationPlateThicknessMm - calibrationPlateThicknessMm) <= 0.001 &&
    Math.abs(surfaceCalibration.perimeter.originXMm - request.originXMm) <= 0.001 &&
    Math.abs(surfaceCalibration.perimeter.originYMm - request.originYMm) <= 0.001 &&
    Math.abs(surfaceCalibration.perimeter.widthMm - request.widthMm) <= 0.001 &&
    Math.abs(surfaceCalibration.perimeter.heightMm - request.heightMm) <= 0.001 &&
    (!surfaceCalibration.workCoordinateOffset || !snapshot.machine.workCoordinateOffset ||
      samePosition(surfaceCalibration.workCoordinateOffset, snapshot.machine.workCoordinateOffset)));
  const surfaceVariationMm = heightmapSurfaceVariation(request);
  const safeWorkZ = heightmapSafeWorkZ(request);
  const totalPoints = request.columns * request.rows;
  const measuredPoints = operation.progress.total > 0
    ? operation.progress.measured
    : displayedMap?.samples.filter(Boolean).length ?? 0;
  const progressPoints = operation.progress.total || displayedMap?.plan.points.length || totalPoints;
  const showProgress = active || Boolean(session.active) || Boolean(session.pending) ||
    operation.state === "failed" || operation.state === "cancelled";
  const estimate = useMemo(() => estimateHeightmapSeconds(request), [request]);
  const programOutside = Boolean(program?.summary.bounds && (
    program.summary.bounds.min.x < request.originXMm ||
    program.summary.bounds.min.y < request.originYMm ||
    program.summary.bounds.max.x > request.originXMm + request.widthMm ||
    program.summary.bounds.max.y > request.originYMm + request.heightMm
  ));

  const updateNumber = (key: keyof HeightmapPlanRequest, value: string) => {
    setRequest((current) => ({ ...current, [key]: Number(value) }));
    if (key === "columns" || key === "rows" || key === "widthMm" || key === "heightMm") {
      setDensity("custom");
    }
  };
  const updateClearance = (value: string) => {
    const clearanceZMm = Number(value);
    setRequest((current) => ({
      ...current,
      clearanceZMm,
      maxProbeDepthMm: clearanceZMm + heightmapSurfaceVariation(current),
    }));
  };
  const runAction = async (action: () => Promise<void>) => {
    setBusy(true);
    setLocalError(undefined);
    onError(undefined);
    try {
      await action();
    } catch (error) {
      const message = String(error);
      setLocalError(message);
      onError(message);
    } finally {
      setBusy(false);
    }
  };
  const autoPerimeter = () => {
    if (!program?.summary.bounds) return;
    setRequest((current) => {
      const perimeter = perimeterFromProgram(current, program.summary.bounds!, margin);
      return density === "custom" ? perimeter : applyDensity(perimeter, density);
    });
  };
  const locateSurface = () => runAction(async () => {
    if (!machineProfileId) throw new Error("Сначала выберите профиль станка");
    if (snapshot.connection !== "connected" || snapshot.machine.mode !== "idle") {
      throw new Error("Для поиска поверхности нужен подключённый контроллер в состоянии Idle");
    }
    if (snapshot.machine.pins?.probe) throw new Error("Щуп уже замкнут. Разомкните контакт перед поиском.");
    if (contactConfigurationError) throw new Error(contactConfigurationError);
    if (!Number.isFinite(surfaceSearchMm) || surfaceSearchMm < 0.1 || surfaceSearchMm > 100) {
      throw new Error("Диапазон поиска должен быть от 0.1 до 100 mm");
    }
    await onSaveMode();
    const outcome = await zProbeGateway.run({
      settings: {
        mode: "heightmap",
        plateThicknessMm: calibrationPlateThicknessMm,
        maxTravelMm: surfaceSearchMm,
        probeFeedMmPerMin: request.probeFeedMmPerMin,
        retractMm: request.clearanceZMm,
        retractFeedMmPerMin: request.retractFeedMmPerMin,
      },
      setupConfirmed: true,
    });
    onSnapshot(outcome.snapshot);
    const calibration = {
      calibrationPlateThicknessMm,
      contactMode: request.contactMode,
      finalWorkZ: outcome.finalWorkZ,
      perimeter: {
        originXMm: request.originXMm,
        originYMm: request.originYMm,
        widthMm: request.widthMm,
        heightMm: request.heightMm,
      },
      resetCount: outcome.snapshot.resetCount,
      workCoordinateOffset: outcome.snapshot.machine.workCoordinateOffset,
    };
    setSurfaceCalibration(calibration);
    setOperation(emptyHeightmapOperation);
  });
  const unlock = () => runAction(async () => {
    const next = await onUnlock();
    onSnapshot(next);
  });
  const start = () => runAction(async () => {
    if (!machineProfileId) throw new Error("Сначала выберите профиль станка");
    if (snapshot.connection !== "connected" || snapshot.machine.mode !== "idle") {
      throw new Error("Для снятия карты нужен подключённый контроллер в состоянии Idle");
    }
    if (snapshot.machine.pins?.probe) throw new Error("Щуп уже замкнут. Разомкните контакт перед запуском.");
    if (validationError) throw new Error(validationError);
    if (programOutside) throw new Error("Часть задания находится за периметром карты");
    if (!surfaceReady) throw new Error("Сначала найдите поверхность и установите Z0");
    await onSaveMode();
    setOperation(await gateway.start({
      plan: request,
      setupConfirmed: true,
      contactAvailableAtEveryPoint: true,
    }, machineProfileId));
  });
  const clearSavedMap = () => runAction(async () => {
    setSession(await gateway.clear());
    setOperation(emptyHeightmapOperation);
  });

  const operationMessage = describeHeightmapFailure(
    localError ?? operation.error,
    operation.state === "failed" ? request.maxProbeDepthMm : surfaceSearchMm,
  );
  const recoveredMap = Boolean(
    session.active && operation.state === "idle" && session.requiresSetupConfirmation,
  );
  const statusMessage = operationMessage ?? contactConfigurationError ?? validationError ??
    (programOutside ? "Задание выходит за периметр карты. Исправьте область или снова нажмите «Авто по заданию»." : undefined) ??
    (recoveredMap ? "Карта восстановлена для просмотра. Перед новым измерением снова найдите поверхность." : undefined);
  const progressLabel = active
    ? operation.state === "paused" ? "Карта на паузе" : "Снимаю карту"
    : operation.state === "failed" || operation.state === "cancelled"
      ? "Измерение остановлено"
      : operation.state === "completed" ? "Карта готова" : "Последняя карта сохранена";
  const surfaceStatusLabel = snapshot.alarm
    ? `ALARM:${snapshot.alarm.code ?? "?"}`
    : surfaceReady
      ? "Z0 найден"
      : snapshot.connection !== "connected"
        ? "Нет соединения"
        : snapshot.machine.mode !== "idle"
          ? snapshot.machine.reportedMode
          : snapshot.machine.pins?.probe
            ? "Контакт замкнут"
            : "Нужно найти Z0";

  return (
    <div className="heightmap-workspace">
      <div className="heightmap-visual">
        <div className="heightmap-visual-toolbar">
          <div className="heightmap-view-tabs" role="tablist" aria-label="Представление карты">
            <button aria-selected={representation === "surface"} onClick={() => setRepresentation("surface")} role="tab" type="button"><Box size={14} /> 3D-карта</button>
            <button aria-selected={representation === "values"} onClick={() => setRepresentation("values")} role="tab" type="button"><Table2 size={14} /> Таблица</button>
          </div>
          {representation === "surface" && (
            <div className="heightmap-view-controls">
              <div className="heightmap-view-tabs compact">
                <button aria-pressed={view === "top"} onClick={() => setView("top")} type="button">Сверху</button>
                <button aria-pressed={view === "iso"} onClick={() => setView("iso")} type="button">3D</button>
              </div>
              <details className="heightmap-layer-menu">
                <summary><SlidersHorizontal size={14} /> Слои</summary>
                <div>
                  <label><input checked={showPerimeter} onChange={(event) => setShowPerimeter(event.target.checked)} type="checkbox" /> Периметр измерения</label>
                  <label><input checked={showJob} onChange={(event) => setShowJob(event.target.checked)} type="checkbox" /> Контур задания</label>
                  <label><input checked={showProbeGrid} onChange={(event) => setShowProbeGrid(event.target.checked)} type="checkbox" /> Точки касания</label>
                  <label><input checked={showInterpolation} onChange={(event) => setShowInterpolation(event.target.checked)} type="checkbox" /> Поверхность карты</label>
                  <div className="heightmap-interpolation-settings">
                    <span>Детализация поверхности</span>
                    <input aria-label="Столбцы поверхности" max="150" min="2" onChange={(event) => setInterpolationColumns(Number(event.target.value))} type="number" value={interpolationColumns} /> ×
                    <input aria-label="Строки поверхности" max="150" min="2" onChange={(event) => setInterpolationRows(Number(event.target.value))} type="number" value={interpolationRows} />
                    <label><input checked={showInterpolationGrid} onChange={(event) => setShowInterpolationGrid(event.target.checked)} type="checkbox" /> показать линии сетки</label>
                  </div>
                </div>
              </details>
            </div>
          )}
        </div>
        <div className="heightmap-visual-stage">
          {representation === "surface" ? (
            <Suspense fallback={<div className="heightmap-scene-loading">3D карта загружается...</div>}>
              <HeightmapScene
                interpolationColumns={interpolationColumns}
                interpolationRows={interpolationRows}
                map={displayedMap}
                program={program}
                request={request}
                showInterpolation={showInterpolation}
                showInterpolationGrid={showInterpolationGrid}
                showJob={showJob}
                showPerimeter={showPerimeter}
                showProbeGrid={showProbeGrid}
                view={view}
              />
            </Suspense>
          ) : <HeightmapValues map={displayedMap} />}
          {representation === "surface" && <div className="heightmap-dimensions"><span>X {request.originXMm.toFixed(2)} → {(request.originXMm + request.widthMm).toFixed(2)}</span><span>Y {request.originYMm.toFixed(2)} → {(request.originYMm + request.heightMm).toFixed(2)}</span></div>}
          {representation === "surface" && displayedMap?.samples.some(Boolean) && <div className="heightmap-color-legend"><span>ниже</span><i /><span>выше</span></div>}
        </div>
      </div>

      <div className="heightmap-settings">
        <div className="heightmap-settings-scroll">
        <section className={`heightmap-surface-setup${snapshot.alarm ? " is-alarm" : surfaceReady ? " is-ready" : ""}`}>
          <header>
            <span>1 · Рабочая поверхность</span>
            <strong>{surfaceStatusLabel}</strong>
          </header>
          <div className="heightmap-surface-state">
            <span className="heightmap-surface-icon">{surfaceCalibration ? <Check size={15} /> : <Crosshair size={15} />}</span>
            <div>
              <strong>{surfaceReady ? "Поверхность найдена" : snapshot.alarm ? "Станок остановлен" : snapshot.connection !== "connected" ? "Подключите станок" : snapshot.machine.mode !== "idle" ? `Дождитесь Idle · сейчас ${snapshot.machine.reportedMode}` : snapshot.machine.pins?.probe ? "Щуп уже замкнут" : "Подведите фрезу над материалом"}</strong>
              <small>{surfaceReady && surfaceCalibration
                ? `Рабочий Z0 установлен, фреза поднята на Z ${surfaceCalibration.finalWorkZ.toFixed(2)} mm. ${request.contactMode === "directSurface" ? "Уберите пластину Z0 перед сеткой: дальше щуп касается самой проводящей поверхности." : "Для сетки оставьте только сплошную пластину, покрывающую весь периметр."}`
                : snapshot.alarm
                  ? "Разблокируйте контроллер здесь. Настройки карты уже сохранены."
                  : snapshot.connection !== "connected"
                    ? "Параметры можно настроить сейчас; движение станет доступно после подключения."
                    : snapshot.machine.pins?.probe
                      ? "Разомкните контакт перед поиском поверхности."
                      : "Выберите самую высокую точку области. После касания она станет Z0, а фреза вернётся на безопасную высоту."}</small>
            </div>
          </div>
          <div className="heightmap-surface-controls">
            <label><span>Первый контакт</span><span><input disabled={settingsLocked} max="100" min="0.1" onChange={(event) => setSurfaceSearchMm(Number(event.target.value))} step="0.5" type="number" value={surfaceSearchMm} /><small>mm вниз</small></span></label>
            <label><span>Макс. перепад поверхности</span><span><input disabled={settingsLocked} min="0.1" onChange={(event) => setRequest((current) => withHeightmapSurfaceVariation(current, Number(event.target.value)))} step="0.1" type="number" value={surfaceVariationMm} /><small>mm вниз</small></span></label>
          </div>
          <p className="heightmap-surface-hint">Первый предел нужен только для поиска Z0. Перепад ограничивает поиск в каждой точке сетки — увеличьте его для заметно неровной формы.</p>
          <div className="heightmap-surface-actions">
            {snapshot.alarm && <button className="is-unlock" disabled={controlsBlocked || !desktopRuntime} onClick={() => void unlock()} type="button"><KeyRound size={14} /> Разблокировать</button>}
            <button className="is-calibrate" disabled={controlsBlocked || active || !desktopRuntime || Boolean(contactConfigurationError) || snapshot.connection !== "connected" || snapshot.machine.mode !== "idle" || Boolean(snapshot.machine.pins?.probe)} onClick={() => void locateSurface()} type="button"><Crosshair size={14} /> {busy ? "Выполняется…" : surfaceReady ? "Найти заново" : "Найти поверхность и установить Z0"}</button>
          </div>
        </section>

        <section>
          <header><span>2 · Периметр заготовки</span><button disabled={!program?.summary.bounds || settingsLocked} onClick={autoPerimeter} type="button"><WandSparkles size={14} /> Авто по заданию</button></header>
          <div className="heightmap-margin"><label>Отступ <input disabled={settingsLocked} min="0" onChange={(event) => setMargin(Number(event.target.value))} step="0.5" type="number" value={margin} /> mm</label></div>
          <div className="heightmap-field-grid">
            {(["originXMm", "originYMm", "widthMm", "heightMm"] as const).map((key) => (
              <label key={key}><span>{{ originXMm: "X от", originYMm: "Y от", widthMm: "Ширина", heightMm: "Высота" }[key]}</span><input disabled={settingsLocked} onChange={(event) => updateNumber(key, event.target.value)} step="0.1" type="number" value={request[key]} /><small>mm</small></label>
            ))}
          </div>
          {programOutside && <p className="heightmap-warning">Красные участки задания выходят за выбранный периметр.</p>}
        </section>

        <section>
          <header><span>3 · Плотность касаний</span><strong>{totalPoints} точек · {duration(estimate)}</strong></header>
          <div className="heightmap-density">
            {(["sparse", "normal", "precise"] as const).map((value) => <button aria-pressed={density === value} disabled={settingsLocked} key={value} onClick={() => { setDensity(value); setRequest((current) => applyDensity(current, value)); }} type="button">{{ sparse: "Редко", normal: "Обычно", precise: "Точно" }[value]}</button>)}
          </div>
          <div className="heightmap-field-grid two">
            <label><span>Точек X</span><input disabled={settingsLocked} max="101" min="2" onChange={(event) => updateNumber("columns", event.target.value)} type="number" value={request.columns} /></label>
            <label><span>Точек Y</span><input disabled={settingsLocked} max="101" min="2" onChange={(event) => updateNumber("rows", event.target.value)} type="number" value={request.rows} /></label>
          </div>
          <p className="heightmap-spacing"><Grid3X3 size={13} /> Шаг X {(request.widthMm / Math.max(1, request.columns - 1)).toFixed(2)} mm · Y {(request.heightMm / Math.max(1, request.rows - 1)).toFixed(2)} mm</p>
        </section>

        <details className="heightmap-advanced">
          <summary>Контакт и подачи</summary>
          <div className="heightmap-contact-mode">
            <label><input checked={request.contactMode === "directSurface"} disabled={settingsLocked} name="contactMode" onChange={() => setRequest((current) => ({ ...current, contactMode: "directSurface", contactOffsetMm: 0 }))} type="radio" /> Прямой контакт с проводящей поверхностью</label>
            <label><input checked={request.contactMode === "fixedPlate"} disabled={settingsLocked} name="contactMode" onChange={() => setRequest((current) => ({ ...current, contactMode: "fixedPlate", contactOffsetMm: 0 }))} type="radio" /> Сплошная пластина на всей области</label>
          </div>
          {request.contactMode === "directSurface" && <div className="heightmap-zero-plate">
            <label><input checked={zeroPlateThicknessMm > 0} disabled={settingsLocked} onChange={(event) => setZeroPlateThicknessMm(event.target.checked ? 19.1 : 0)} type="checkbox" /> Использую отдельную калибровочную пластину только для поиска Z0</label>
            {zeroPlateThicknessMm > 0 && <label><span>Измеренная толщина</span><span><input disabled={settingsLocked} max="100" min="0.01" onChange={(event) => setZeroPlateThicknessMm(Number(event.target.value))} step="0.01" type="number" value={zeroPlateThicknessMm} /><small>mm</small></span></label>}
            <small>Это толщина съёмной шайбы, а не заготовки. Для прямого касания поверхности оставьте выключенным.</small>
          </div>}
          {request.contactMode === "fixedPlate" && <p className="heightmap-fixed-plate-note">Эта пластина остаётся на всей области. Её толщина используется и для первого Z0, и для каждой точки карты.</p>}
          <div className="heightmap-field-grid two">
            <label><span>Зазор над поверхностью</span><input disabled={settingsLocked} min="0.1" onChange={(event) => updateClearance(event.target.value)} step="0.1" type="number" value={request.clearanceZMm} /></label>
            {(["probeFeedMmPerMin", "travelFeedMmPerMin", "retractFeedMmPerMin"] as const).map((key) => <label key={key}><span>{{ probeFeedMmPerMin: "Подача щупа", travelFeedMmPerMin: "Переход XY", retractFeedMmPerMin: "Подъём Z" }[key]}</span><input disabled={settingsLocked} min="0.1" onChange={(event) => updateNumber(key, event.target.value)} type="number" value={request[key]} /></label>)}
            {request.contactMode === "fixedPlate" && <label><span>Сплошная пластина сетки</span><input disabled={settingsLocked} min="0.01" onChange={(event) => updateNumber("contactOffsetMm", event.target.value)} step="0.01" type="number" value={request.contactOffsetMm} /></label>}
          </div>
          <p className="heightmap-safe-z-note">Переходы: Z {safeWorkZ.toFixed(2)} mm · поиск сетки: {request.maxProbeDepthMm.toFixed(2)} mm вниз</p>
        </details>
        </div>

        <div className="heightmap-action-dock">
        <div aria-hidden={!showProgress} className={`heightmap-progress${showProgress ? "" : " is-empty"}`} aria-live="polite">
          <span><ScanLine size={15} /> {progressLabel}</span>
          <strong>{measuredPoints}/{progressPoints}</strong>
          {session.active && !active && <button aria-label="Удалить сохранённую карту" className="icon-only" disabled={controlsBlocked} onClick={() => void clearSavedMap()} title="Удалить сохранённую карту" type="button"><Trash2 size={14} /></button>}
          <i><b style={{ width: `${progressPoints ? measuredPoints / progressPoints * 100 : 0}%` }} /></i>
        </div>
        <div className="heightmap-primary-actions">
          {active ? (
            <>
              <button disabled={controlsBlocked} onClick={() => void runAction(async () => setOperation(operation.state === "paused" ? await gateway.resume() : await gateway.pause()))} type="button">{operation.state === "paused" ? <Play size={15} /> : <Pause size={15} />}{operation.state === "paused" ? "Продолжить" : "Пауза"}</button>
              <button className="is-danger" disabled={controlsBlocked} onClick={() => void runAction(async () => { await onAbort(); })} type="button"><Square size={14} /> Остановить</button>
            </>
          ) : <button className="is-primary" disabled={controlsBlocked || !desktopRuntime || !surfaceReady || Boolean(contactConfigurationError) || Boolean(validationError) || programOutside || snapshot.connection !== "connected" || snapshot.machine.mode !== "idle" || Boolean(snapshot.machine.pins?.probe)} onClick={() => void start()} type="button"><Crosshair size={15} /> {busy ? "Выполняется…" : surfaceReady ? `Снять карту · ${totalPoints} точек` : "Сначала найдите поверхность"}</button>}
        </div>
        <p aria-live="polite" className={`heightmap-status-message${operationMessage || contactConfigurationError || validationError || programOutside ? " is-error" : ""}${statusMessage ? "" : " is-empty"}`}>{statusMessage ?? "Состояние карты без ошибок"}</p>
        </div>
      </div>
    </div>
  );
}
