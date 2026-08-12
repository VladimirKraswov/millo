import { Crosshair } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import type { WorkCoordinateGateway } from "../../platform/machine/WorkCoordinateGateway";
import type {
  ControllerSnapshot,
  WorkAxis,
  WorkZeroOutcome,
} from "../../shared/machine";
import { WorkZeroInteractor } from "./WorkZeroInteractor";

const axes: readonly WorkAxis[] = ["x", "y", "z"];

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
  const [busyAxis, setBusyAxis] = useState<WorkAxis | "xyz">();
  const [outcome, setOutcome] = useState<WorkZeroOutcome>();
  const connected = snapshot.connection === "connected";
  const stableIdle =
    connected &&
    snapshot.machine.mode === "idle" &&
    snapshot.alarm === undefined &&
    snapshot.resetNotice === undefined;
  const canSet =
    desktopRuntime && stableIdle && positionConfirmed && !disabled && !busyAxis;

  useEffect(() => {
    if (!connected) {
      setPositionConfirmed(false);
      setOutcome(undefined);
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

      <label className="work-zero-confirmation">
        <input
          checked={positionConfirmed}
          disabled={!stableIdle || disabled || busyAxis !== undefined}
          onChange={(event) => {
            setPositionConfirmed(event.target.checked);
            setOutcome(undefined);
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
