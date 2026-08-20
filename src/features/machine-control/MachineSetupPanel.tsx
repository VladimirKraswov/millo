import { Droplets, Power, RotateCw } from "lucide-react";
import { useState } from "react";

import { selectWorkCoordinateSystem, setMachineOutput } from "../../api/controller";
import type {
  ControllerSnapshot,
  SpindleControl,
  WorkCoordinateSystem,
} from "../../shared/machine";

interface MachineSetupPanelProps {
  activeCoordinateSystem: WorkCoordinateSystem;
  disabled: boolean;
  snapshot: ControllerSnapshot;
  spindleControl: SpindleControl;
  floodCoolantControl: boolean;
  mistCoolantControl: boolean;
  onError: (error?: string) => void;
  onSnapshot: (snapshot: ControllerSnapshot) => void;
}

const coordinateSystems: readonly WorkCoordinateSystem[] = [
  "g54", "g55", "g56", "g57", "g58", "g59",
];

export function MachineSetupPanel({
  activeCoordinateSystem,
  disabled,
  snapshot,
  spindleControl,
  floodCoolantControl,
  mistCoolantControl,
  onError,
  onSnapshot,
}: MachineSetupPanelProps) {
  const [busy, setBusy] = useState(false);
  const [speedRpm, setSpeedRpm] = useState(1_000);
  const stableIdle = snapshot.connection === "connected" && snapshot.machine.mode === "idle";
  const controlsDisabled = disabled || busy || !stableIdle;

  const run = async (action: () => Promise<ControllerSnapshot>) => {
    setBusy(true);
    onError(undefined);
    try {
      onSnapshot(await action());
    } catch (error) {
      onError(String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="machine-setup-panel">
      <div className="machine-wcs-selector" role="group" aria-label="Рабочая система координат">
        {coordinateSystems.map((coordinateSystem) => (
          <button
            aria-pressed={activeCoordinateSystem === coordinateSystem}
            className={activeCoordinateSystem === coordinateSystem ? "is-selected" : undefined}
            disabled={controlsDisabled}
            key={coordinateSystem}
            onClick={() => void run(async () => (await selectWorkCoordinateSystem(coordinateSystem)).snapshot)}
            type="button"
          >
            {coordinateSystem.toUpperCase()}
          </button>
        ))}
      </div>

      <div className="machine-output-row">
        <RotateCw aria-hidden="true" size={14} />
        <span>
          <strong>Шпиндель</strong>
          <small>{spindleControl === "controller" ? "Управляется GRBL" : "Ручное включение"}</small>
        </span>
        {spindleControl === "controller" && (
          <>
            <input
              aria-label="Обороты шпинделя"
              disabled={controlsDisabled}
              min="0"
              onChange={(event) => setSpeedRpm(Number(event.target.value))}
              step="100"
              type="number"
              value={speedRpm}
            />
            <button disabled={controlsDisabled} onClick={() => void run(async () => (await setMachineOutput({ spindleOn: { direction: "clockwise", speedRpm } })).snapshot)} type="button">M3</button>
            <button disabled={controlsDisabled} onClick={() => void run(async () => (await setMachineOutput("spindleOff")).snapshot)} type="button">M5</button>
          </>
        )}
      </div>

      {(floodCoolantControl || mistCoolantControl) && (
        <div className="machine-output-row">
          <Droplets aria-hidden="true" size={14} />
          <span>
            <strong>Охлаждение</strong>
            <small>Только заявленные выходы</small>
          </span>
          {floodCoolantControl && (
            <button disabled={controlsDisabled} onClick={() => void run(async () => (await setMachineOutput({ floodCoolant: true })).snapshot)} type="button">M8</button>
          )}
          {mistCoolantControl && (
            <button disabled={controlsDisabled} onClick={() => void run(async () => (await setMachineOutput({ mistCoolant: true })).snapshot)} type="button">M7</button>
          )}
          <button aria-label="Выключить шпиндель и охлаждение" disabled={controlsDisabled} onClick={() => void run(async () => (await setMachineOutput("allOff")).snapshot)} title="M5 + M9" type="button">
            <Power aria-hidden="true" size={13} />
          </button>
        </div>
      )}
    </div>
  );
}
