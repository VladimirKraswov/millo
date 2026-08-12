import { Crosshair, LocateFixed } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import type { WorkCoordinateGateway } from "../../platform/machine/WorkCoordinateGateway";
import type {
  ControllerSnapshot,
  ReturnToWorkZeroOutcome,
  WorkAxis,
  WorkZeroOutcome,
} from "../../shared/machine";
import { WorkZeroInteractor } from "./WorkZeroInteractor";

const axes: readonly WorkAxis[] = ["x", "y", "z"];
type BusyOperation = WorkAxis | "xyz" | `return-${WorkAxis}`;

interface WorkZeroPanelProps {
  readonly activeCoordinateSystem?: string;
  readonly snapshot: ControllerSnapshot;
  readonly desktopRuntime: boolean;
  readonly disabled?: boolean;
  readonly gateway: WorkCoordinateGateway;
  readonly onSnapshot: (snapshot: ControllerSnapshot) => void;
  readonly onError: (error?: string) => void;
  readonly onOutcome?: (outcome: WorkZeroOutcome) => void;
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
}: WorkZeroPanelProps) {
  const interactor = useMemo(() => new WorkZeroInteractor(gateway), [gateway]);
  const [positionConfirmed, setPositionConfirmed] = useState(false);
  const [busyAxis, setBusyAxis] = useState<BusyOperation>();
  const [outcome, setOutcome] = useState<WorkZeroOutcome>();
  const [returnOutcome, setReturnOutcome] = useState<ReturnToWorkZeroOutcome>();
  const connected = snapshot.connection === "connected";
  const stableIdle =
    connected &&
    snapshot.machine.mode === "idle" &&
    snapshot.alarm === undefined &&
    snapshot.resetNotice === undefined;
  const canSet =
    desktopRuntime && stableIdle && positionConfirmed && !disabled && !busyAxis;
  const canReturn = desktopRuntime && stableIdle && !disabled && !busyAxis;

  useEffect(() => {
    if (!connected) {
      setPositionConfirmed(false);
      setOutcome(undefined);
      setReturnOutcome(undefined);
    }
  }, [connected]);

  const setZero = async (axis: WorkAxis) => {
    if (!canSet) return;
    setBusyAxis(axis);
    setOutcome(undefined);
    onError(undefined);
    try {
      const next = await interactor.set(axis, positionConfirmed);
      setOutcome(next);
      onSnapshot(next.snapshot);
      onOutcome?.(next);
    } catch (error) {
      onError(String(error));
    } finally {
      setPositionConfirmed(false);
      setBusyAxis(undefined);
    }
  };

  const returnToZero = async (axis: WorkAxis) => {
    if (!canReturn) return;
    setBusyAxis(`return-${axis}`);
    setOutcome(undefined);
    setReturnOutcome(undefined);
    onError(undefined);
    try {
      const next = await interactor.returnToZero(axis, axis === "z" ? 100 : 300);
      setReturnOutcome(next);
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
      let next: WorkZeroOutcome | undefined;
      for (const axis of axes) {
        next = await interactor.set(axis, true);
        onSnapshot(next.snapshot);
      }
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
        <div role="group" aria-label="Вернуться к рабочему нулю">
          {axes.map((axis) => (
            <button
              disabled={!canReturn}
              key={axis}
              onClick={() => void returnToZero(axis)}
              type="button"
            >
              К {axis.toUpperCase()}0
            </button>
          ))}
        </div>
        <span className="work-zero-return-status" aria-live="polite">
          {busyAxis?.startsWith("return-")
            ? "Команда движения и свежая проверка GRBL..."
            : returnOutcome
              ? `${returnOutcome.coordinateSystem.toUpperCase()} · движение ${returnOutcome.axis.toUpperCase()} к 0 принято`
              : "X/Y доступны только при положительном рабочем Z; Z возвращается отдельно"}
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
        Установить XYZ = 0
      </button>

      <div className="work-zero-actions" role="group" aria-label="Рабочий ноль">
        {axes.map((axis) => (
          <button
            disabled={!canSet}
            key={axis}
            onClick={() => void setZero(axis)}
            type="button"
          >
            <span>0</span>
            Только {axis.toUpperCase()}
          </button>
        ))}
      </div>

      <div className="work-zero-status" aria-live="polite">
        {busyAxis && <span>Запись {busyAxis.toUpperCase()} и проверка через $G / $#...</span>}
        {!busyAxis && outcome && (
          <span>
            {outcome.coordinateSystem.toUpperCase()} {outcome.axis.toUpperCase()} ={" "}
            {outcome.workPosition.toFixed(3)} mm
          </span>
        )}
        {!busyAxis && !outcome && <span>После записи координаты повторно считываются из GRBL</span>}
      </div>
    </section>
  );
}
