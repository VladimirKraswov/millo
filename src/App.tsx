import { useEffect, useMemo, useState } from "react";

import {
  connectMock,
  disconnect,
  getControllerSnapshot,
  isDesktopRuntime,
  onMachineState,
  refreshStatus,
} from "./api/controller";
import {
  emptySnapshot,
  type ControllerSnapshot,
  type Position,
} from "./shared/machine";

const connectionLabels = {
  disconnected: "Отключено",
  connecting: "Подключение",
  connected: "Подключено",
  faulted: "Ошибка",
} as const;

const formatCoordinate = (value: number | undefined): string =>
  value === undefined ? "--" : value.toFixed(3);

function AxisReadout({ axis, value }: { axis: string; value: number | undefined }) {
  return (
    <div className="axis-readout">
      <span>{axis}</span>
      <strong>{formatCoordinate(value)}</strong>
      <small>mm</small>
    </div>
  );
}

function PositionReadout({ position }: { position?: Position }) {
  return (
    <div className="position-grid">
      <AxisReadout axis="X" value={position?.x} />
      <AxisReadout axis="Y" value={position?.y} />
      <AxisReadout axis="Z" value={position?.z} />
      {position?.a !== undefined && <AxisReadout axis="A" value={position.a} />}
    </div>
  );
}

export default function App() {
  const [snapshot, setSnapshot] = useState<ControllerSnapshot>(emptySnapshot);
  const [busy, setBusy] = useState(false);
  const [uiError, setUiError] = useState<string>();
  const desktopRuntime = useMemo(isDesktopRuntime, []);

  useEffect(() => {
    if (!desktopRuntime) {
      return;
    }

    let active = true;
    let unlisten: (() => void) | undefined;

    void getControllerSnapshot().then((value) => {
      if (active) setSnapshot(value);
    });
    void onMachineState((value) => {
      if (active) setSnapshot(value);
    }).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [desktopRuntime]);

  const runAction = async (action: () => Promise<ControllerSnapshot>) => {
    setBusy(true);
    setUiError(undefined);
    try {
      setSnapshot(await action());
    } catch (error) {
      setUiError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const isConnected = snapshot.connection === "connected";
  const displayedError = uiError ?? snapshot.lastError;

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            <i />
          </span>
          <div>
            <strong>Gantryon</strong>
            <span>Machine control</span>
          </div>
        </div>

        <div className={`connection-state is-${snapshot.connection}`}>
          <span className="state-dot" />
          <div>
            <small>Mock transport</small>
            <strong>{connectionLabels[snapshot.connection]}</strong>
          </div>
        </div>
      </header>

      <main className="workspace">
        <section className="machine-panel" aria-labelledby="machine-state-title">
          <div className="section-heading">
            <div>
              <span>GRBL controller</span>
              <h1 id="machine-state-title">
                {snapshot.machine.reportedMode}
                {snapshot.machine.substate !== undefined
                  ? `:${snapshot.machine.substate}`
                  : ""}
              </h1>
            </div>
            <span className={`mode-indicator is-${snapshot.machine.mode}`}>
              {snapshot.machine.mode}
            </span>
          </div>

          <div className="readout-section">
            <div className="readout-label">
              <span>Machine position</span>
              <small>G53</small>
            </div>
            <PositionReadout position={snapshot.machine.machinePosition} />
          </div>

          <div className="telemetry-row">
            <div>
              <span>Feed</span>
              <strong>{snapshot.machine.feedRate.toFixed(1)}</strong>
              <small>mm/min</small>
            </div>
            <div>
              <span>Spindle</span>
              <strong>{snapshot.machine.spindleSpeed.toFixed(0)}</strong>
              <small>rpm</small>
            </div>
          </div>
        </section>

        <aside className="control-panel" aria-label="Connection controls">
          <div className="panel-title">
            <span>Transport</span>
            <strong>Mock GRBL</strong>
          </div>

          <div className="actions">
            <button
              className="primary-action"
              disabled={busy || isConnected || !desktopRuntime}
              onClick={() => void runAction(connectMock)}
              type="button"
            >
              Подключить
            </button>
            <button
              disabled={busy || !isConnected}
              onClick={() => void runAction(refreshStatus)}
              type="button"
            >
              Запросить статус
              <kbd>?</kbd>
            </button>
            <button
              disabled={busy || !isConnected}
              onClick={() => void runAction(disconnect)}
              type="button"
            >
              Отключить
            </button>
          </div>

          <div className="pipeline" aria-label="Active data path">
            <span>Mock</span>
            <i />
            <span>GRBL</span>
            <i />
            <span>State</span>
            <i />
            <span>UI</span>
          </div>

          {!desktopRuntime && (
            <p className="runtime-note">Управление доступно в окне Tauri.</p>
          )}
          {displayedError && <p className="error-note">{displayedError}</p>}
        </aside>
      </main>

      <footer className="statusbar">
        <span>Protocol: GRBL status v1.1</span>
        <span>Core: Rust</span>
        <span>UI: TypeScript</span>
      </footer>
    </div>
  );
}
