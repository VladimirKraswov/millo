import { Crosshair, LocateFixed } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import type { WorkCoordinateGateway } from "../../platform/machine/WorkCoordinateGateway";
import type {
  ControllerSnapshot,
  ReturnToWorkOriginOutcome,
  ReturnToWorkZeroOutcome,
  WorkAxis,
  WorkZeroOutcome,
} from "../../shared/machine";
import {
  isControllerConnected,
  isControllerStableIdle,
} from "../../shared/controllerReadiness";
import { WorkZeroInteractor } from "./WorkZeroInteractor";

const axes: readonly WorkAxis[] = ["x", "y", "z"];
type BusyOperation = WorkAxis | "xyz" | "return-origin";

interface WorkZeroPanelProps {
  readonly activeCoordinateSystem?: string;
  readonly snapshot: ControllerSnapshot;
  readonly desktopRuntime: boolean;
  readonly disabled?: boolean;
  readonly gateway: WorkCoordinateGateway;
  readonly onSnapshot: (snapshot: ControllerSnapshot) => void;
  readonly onError: (error?: string) => void;
  readonly onOutcome?: (outcome: WorkZeroOutcome) => void;
  readonly useProbeForZ?: boolean;
}

export function WorkZeroPanel({
  activeCoordinateSystem,
  snapshot,
  desktopRuntime,
  disabled = false,
  gateway,
  onSnapshot,
  onError,
  onOutcome,
  useProbeForZ = false,
}: WorkZeroPanelProps) {
  const interactor = useMemo(() => new WorkZeroInteractor(gateway), [gateway]);
  const [positionConfirmed, setPositionConfirmed] = useState(false);
  const [rotaryConfirmed, setRotaryConfirmed] = useState(false);
  const [busyAxis, setBusyAxis] = useState<BusyOperation>();
  const [outcome, setOutcome] = useState<WorkZeroOutcome>();
  const [, setReturnOutcome] = useState<ReturnToWorkZeroOutcome>();
  const [originOutcome, setOriginOutcome] = useState<ReturnToWorkOriginOutcome>();
  const connected = isControllerConnected(snapshot);
  const stableIdle = isControllerStableIdle(snapshot);
  const rotaryReported = [snapshot.machine.machinePosition, snapshot.machine.workPosition,
    snapshot.machine.workCoordinateOffset].every((position) => Number.isFinite(position?.a));
  const canSet =
    desktopRuntime && stableIdle && positionConfirmed && !disabled && !busyAxis;
  const canReturn = desktopRuntime && stableIdle && !disabled && !busyAxis && gateway.returnToOrigin !== undefined;
  const canSetRotary = desktopRuntime && stableIdle && rotaryReported && rotaryConfirmed && !disabled && !busyAxis;

  useEffect(() => {
    setPositionConfirmed(false);
    setRotaryConfirmed(false);
    setOutcome(undefined);
  }, [snapshot.resetCount, snapshot.reconnectCount, connected, rotaryReported, stableIdle,
    snapshot.machine.machinePosition?.a]);

  useEffect(() => {
    if (!connected) {
      setPositionConfirmed(false);
      setOutcome(undefined);
      setReturnOutcome(undefined);
      setOriginOutcome(undefined);
    }
  }, [connected]);

  const setZero = async (axis: WorkAxis) => {
    if (axis === "a" ? !canSetRotary : !canSet) return;
    setBusyAxis(axis);
    setOutcome(undefined);
    onError(undefined);
    try {
      const next = await interactor.set(axis, axis === "a" ? rotaryConfirmed : positionConfirmed);
      setOutcome(next);
      onSnapshot(next.snapshot);
      onOutcome?.(next);
    } catch (error) {
      onError(String(error));
    } finally {
      setPositionConfirmed(false);
      setRotaryConfirmed(false);
      setBusyAxis(undefined);
    }
  };

  const returnToOrigin = async () => {
    if (!canReturn) return;
    setBusyAxis("return-origin");
    setOutcome(undefined);
    setReturnOutcome(undefined);
    setOriginOutcome(undefined);
    onError(undefined);
    try {
      const next = await gateway.returnToOrigin!({
        clearanceZMm: 2,
        xyFeedMmPerMin: 300,
        zFeedMmPerMin: 100,
      });
      setOriginOutcome(next);
      onSnapshot(next.snapshot);
    } catch (error) {
      onError(String(error));
    } finally {
      setBusyAxis(undefined);
    }
  };

  const setAllZero = async () => {
    if (!canSet) return;
    setBusyAxis("xyz");
    setOutcome(undefined);
    onError(undefined);
    try {
      const results = await interactor.setCartesian(positionConfirmed, !useProbeForZ,
        (next) => onSnapshot(next.snapshot));
      const next = results.at(-1);
      if (next) {
        setOutcome(next);
        onOutcome?.(next);
      }
    } catch (error) {
      onError(String(error));
    } finally {
      setPositionConfirmed(false);
      setBusyAxis(undefined);
    }
  };

  return (
    <section className="work-zero" aria-labelledby="work-zero-title">
      <div className="work-zero-heading">
        <div>
          <span>Рабочие координаты</span>
          <strong id="work-zero-title">Какие оси обнулить</strong>
        </div>
        <code>
          {outcome?.coordinateSystem.toUpperCase() ?? activeCoordinateSystem ?? "G54-G59"}
        </code>
      </div>

      <section className="work-zero-return" aria-labelledby="work-zero-return-title">
        <div>
          <LocateFixed aria-hidden="true" size={16} />
          <span><strong id="work-zero-return-title">Вернуться к сохранённому нулю</strong><small>Двигает ось к 0, но не меняет G54–G59</small></span>
        </div>
        <button
          className="work-zero-return-all"
          disabled={!canReturn}
          onClick={() => void returnToOrigin()}
          type="button"
        >
          <LocateFixed aria-hidden="true" size={15} />
          Вернуться в рабочий ноль
        </button>
        <span className="work-zero-return-status" aria-live="polite">
          {busyAxis === "return-origin"
            ? "Поднимаем Z, возвращаем XY и опускаем Z к нулю..."
            : originOutcome
              ? `${originOutcome.coordinateSystem.toUpperCase()} · станок подтверждён в X0 Y0 Z0`
              : "Безопасный маршрут: Z вверх → XY к нулю → Z к нулю"}
        </span>
      </section>

      <label className="work-zero-confirmation">
        <input
          checked={positionConfirmed}
          disabled={!stableIdle || disabled || busyAxis !== undefined}
          onChange={(event) => {
            setPositionConfirmed(event.target.checked);
            setOutcome(undefined);
            setReturnOutcome(undefined);
          }}
          type="checkbox"
        />
        <span>Инструмент находится в точке, которую нужно принять за ноль</span>
      </label>

      <button
        className="work-zero-all"
        disabled={!canSet}
        onClick={() => void setAllZero()}
        type="button"
      >
        <Crosshair aria-hidden="true" size={15} />
        {useProbeForZ ? "Установить только XY = 0" : "Установить XYZ = 0"}
      </button>

      <div className="work-zero-actions" role="group" aria-label="Рабочий ноль">
        {axes.map((axis) => (
          <button
            disabled={!canSet || (axis === "z" && useProbeForZ)}
            key={axis}
            onClick={() => void setZero(axis)}
            type="button"
          >
            <span>0</span>
            {axis === "z" && useProbeForZ ? "Z0 уже найден щупом" : `Только ${axis.toUpperCase()}`}
          </button>
        ))}
      </div>

      {rotaryReported && <div>
        <label className="work-zero-confirmation">
          <input
            checked={rotaryConfirmed}
            disabled={!stableIdle || disabled || busyAxis !== undefined}
            onChange={(event) => setRotaryConfirmed(event.target.checked)}
            type="checkbox"
          />
          <span>Подтверждаю текущий угол A как рабочий ноль <code>A {snapshot.machine.workPosition?.a?.toFixed(3)}°</code></span>
        </label>
        <div className="work-zero-actions" role="group" aria-label="Рабочий ноль A">
          <button disabled={!canSetRotary} onClick={() => void setZero("a")} type="button">
            <Crosshair aria-hidden="true" size={15} />
            Только A
          </button>
        </div>
      </div>}

      <div className="work-zero-status" aria-live="polite">
        {busyAxis && <span>Запись {busyAxis.toUpperCase()} и проверка через $G / $#...</span>}
        {!busyAxis && outcome && (
          <span>
            {outcome.coordinateSystem.toUpperCase()} {outcome.axis.toUpperCase()} ={" "}
            {outcome.workPosition.toFixed(3)} {outcome.axis === "a" ? "°" : "mm"}
          </span>
        )}
        {!busyAxis && !outcome && <span>{useProbeForZ
          ? "Z0 защищён от перезаписи; изменяются только X/Y"
          : "После записи координаты повторно считываются из GRBL"}</span>}
      </div>
    </section>
  );
}
