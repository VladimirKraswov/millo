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
  readonly snapshot: ControllerSnapshot;
  readonly desktopRuntime: boolean;
  readonly disabled?: boolean;
  readonly gateway: WorkCoordinateGateway;
  readonly onSnapshot: (snapshot: ControllerSnapshot) => void;
  readonly onError: (error?: string) => void;
}

export function WorkZeroPanel({
  snapshot,
  desktopRuntime,
  disabled = false,
  gateway,
  onSnapshot,
  onError,
}: WorkZeroPanelProps) {
  const interactor = useMemo(() => new WorkZeroInteractor(gateway), [gateway]);
  const [positionConfirmed, setPositionConfirmed] = useState(false);
  const [busyAxis, setBusyAxis] = useState<WorkAxis>();
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
          <span>Work coordinates</span>
          <strong id="work-zero-title">Set work zero</strong>
        </div>
        <code>{outcome?.coordinateSystem.toUpperCase() ?? "G54-G59"}</code>
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
        <span>Инструмент установлен в нулевой точке выбранной оси</span>
      </label>

      <div className="work-zero-actions" role="group" aria-label="Рабочий ноль">
        {axes.map((axis) => (
          <button
            disabled={!canSet}
            key={axis}
            onClick={() => void setZero(axis)}
            type="button"
          >
            <span>0</span>
            Zero {axis.toUpperCase()}
          </button>
        ))}
      </div>

      <div className="work-zero-status" aria-live="polite">
        {busyAxis && <span>Проверка {busyAxis.toUpperCase()} через $G / $#...</span>}
        {!busyAxis && outcome && (
          <span>
            {outcome.coordinateSystem.toUpperCase()} {outcome.axis.toUpperCase()} ={" "}
            {outcome.workPosition.toFixed(3)} mm
          </span>
        )}
        {!busyAxis && !outcome && <span>Подтверждение действует на одно нажатие</span>}
      </div>
    </section>
  );
}
