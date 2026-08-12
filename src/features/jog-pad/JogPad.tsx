import { useEffect, useMemo, useState } from "react";

import type { MachineCommandGateway } from "../../platform/machine/MachineCommandGateway";
import type {
  ControllerSnapshot,
  HardwareInspection,
  JogAxis,
  OperatorConfirmation,
  StepJogReceipt,
} from "../../shared/machine";
import {
  JOG_PAD_FEED_MM_PER_MIN,
  JOG_PAD_STEPS_MM,
  JogPadInteractor,
  jogOperatorConfirmation,
  type JogDirection,
  type JogPadStepMm,
} from "./JogPadInteractor";

interface JogPadProps {
  snapshot: ControllerSnapshot;
  desktopRuntime: boolean;
  disabled?: boolean;
  gateway: MachineCommandGateway;
  onInspection: (inspection?: HardwareInspection) => void;
  onError: (error?: string) => void;
}

export function JogPad({
  snapshot,
  desktopRuntime,
  disabled = false,
  gateway,
  onInspection,
  onError,
}: JogPadProps) {
  const interactor = useMemo(() => new JogPadInteractor(gateway), [gateway]);
  const [jogArmed, setJogArmed] = useState(false);
  const confirmation: OperatorConfirmation = jogOperatorConfirmation(jogArmed);
  const [stepMm, setStepMm] = useState<JogPadStepMm>(0.1);
  const [busy, setBusy] = useState(false);
  const [lastJog, setLastJog] = useState<StepJogReceipt>();
  const [blockedCount, setBlockedCount] = useState<number>();

  const connected = snapshot.connection === "connected";
  const stableIdle =
    connected &&
    snapshot.machine.mode === "idle" &&
    snapshot.alarm === undefined &&
    snapshot.resetNotice === undefined;
  const canMove =
    desktopRuntime &&
    stableIdle &&
    jogArmed &&
    !disabled &&
    !busy;

  useEffect(() => {
    if (!connected) {
      setJogArmed(false);
      setLastJog(undefined);
      setBlockedCount(undefined);
    }
  }, [connected]);

  const move = async (axis: JogAxis, direction: JogDirection) => {
    if (!canMove) return;

    setBusy(true);
    setLastJog(undefined);
    setBlockedCount(undefined);
    onError(undefined);
    try {
      const outcome = await interactor.move(
        confirmation,
        axis,
        direction,
        stepMm,
      );
      onInspection(outcome.inspection);
      setLastJog(outcome.receipt);
      setBlockedCount(
        outcome.receipt
          ? undefined
          : outcome.inspection.readiness.blockerCount,
      );
    } catch (error) {
      onError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const moveButton = (
    axis: JogAxis,
    direction: JogDirection,
    symbol: string,
  ) => {
    const signedAxis = `${axis.toUpperCase()}${direction > 0 ? "+" : "−"}`;
    return (
      <button
        aria-label={`Jog ${signedAxis}`}
        disabled={!canMove}
        onClick={() => void move(axis, direction)}
        title={`Jog ${signedAxis}: ${stepMm.toFixed(2)} мм`}
        type="button"
      >
        <span aria-hidden="true">{symbol}</span>
        <small>{signedAxis}</small>
      </button>
    );
  };

  return (
    <section className="jog-pad" aria-labelledby="jog-pad-title">
      <div className="jog-pad-heading">
        <div>
          <span>Machine control</span>
          <strong id="jog-pad-title">Jog pad</strong>
        </div>
        <code>F{JOG_PAD_FEED_MM_PER_MIN}</code>
      </div>

      <label className="jog-arm-control">
        <input
          checked={jogArmed}
          disabled={!connected || disabled || busy}
          onChange={(event) => {
            setJogArmed(event.target.checked);
            setLastJog(undefined);
            setBlockedCount(undefined);
          }}
          type="checkbox"
        />
        <span>
          <strong>Разрешить jog</strong>
          <small>Шпиндель выключен, зона свободна, питание доступно</small>
        </span>
      </label>

      <div className="jog-step-selector" role="group" aria-label="Шаг jog">
        {JOG_PAD_STEPS_MM.map((value) => (
          <button
            aria-pressed={stepMm === value}
            className={stepMm === value ? "is-selected" : undefined}
            disabled={disabled || busy}
            key={value}
            onClick={() => setStepMm(value)}
            type="button"
          >
            {value.toFixed(2)} mm
          </button>
        ))}
      </div>

      <div className="jog-pad-controls">
        <div className="jog-pad-xy" aria-label="Jog X и Y">
          <i />
          {moveButton("y", 1, "↑")}
          <i />
          {moveButton("x", -1, "←")}
          <span className="jog-pad-center" aria-hidden="true">
            XY
          </span>
          {moveButton("x", 1, "→")}
          <i />
          {moveButton("y", -1, "↓")}
          <i />
        </div>
        <div className="jog-pad-z" aria-label="Jog Z">
          {moveButton("z", 1, "↑")}
          {moveButton("z", -1, "↓")}
        </div>
      </div>

      <div className="jog-pad-status" aria-live="polite">
        {busy && <span>Свежая проверка станка...</span>}
        {!busy && lastJog && (
          <span>
            Принято: <code>{lastJog.command}</code>
          </span>
        )}
        {!busy && blockedCount !== undefined && (
          <span className="is-blocked">Движение заблокировано: {blockedCount}</span>
        )}
        {!busy && !lastJog && blockedCount === undefined && (
          <span>Каждое нажатие повторяет status и Inspector</span>
        )}
      </div>
    </section>
  );
}
