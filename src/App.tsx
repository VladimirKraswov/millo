import { useEffect, useMemo, useState } from "react";

import {
  acknowledgeReset,
  clearMockAlarm,
  connectMock,
  disconnect,
  getControllerSnapshot,
  isDesktopRuntime,
  onMachineState,
  refreshStatus,
  triggerMockAlarm,
  triggerMockDisconnect,
  triggerMockReset,
  triggerMockTimeout,
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
  recovering: "Восстановление",
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
      if (active) {
        setSnapshot(value);
        if (value.connection === "connected" && value.consecutiveFailures === 0) {
          setUiError(undefined);
        }
      }
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

  const runMockAction = async (action: () => Promise<void>) => {
    setUiError(undefined);
    try {
      await action();
    } catch (error) {
      setUiError(String(error));
    }
  };

  const isConnected = snapshot.connection === "connected";
  const hasConnection =
    snapshot.connection === "connected" || snapshot.connection === "recovering";
  const canDisconnect =
    snapshot.connection !== "disconnected" && snapshot.connection !== "connecting";
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

          {snapshot.resetNotice && (
            <div className="operator-notice reset-notice" role="status">
              <div>
                <span>Controller reset</span>
                <strong>{snapshot.resetNotice.banner}</strong>
              </div>
              <button
                type="button"
                onClick={() => void runAction(acknowledgeReset)}
              >
                Подтвердить
              </button>
            </div>
          )}

          {snapshot.alarm && (
            <div className="operator-notice alarm-notice" role="alert">
              <div>
                <span>Alarm</span>
                <strong>
                  {snapshot.alarm.code !== undefined
                    ? `ALARM:${snapshot.alarm.code}`
                    : snapshot.alarm.message}
                </strong>
              </div>
              <small>Требуется проверка состояния станка</small>
            </div>
          )}

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

          <div className="lifecycle-metrics">
            <div>
              <span>Polling</span>
              <strong>{snapshot.pollIntervalMs || "--"} ms</strong>
            </div>
            <div>
              <span>Timeout</span>
              <strong>{snapshot.statusTimeoutMs || "--"} ms</strong>
            </div>
            <div>
              <span>Failures</span>
              <strong>
                {snapshot.consecutiveFailures}/{snapshot.failureThreshold || "--"}
              </strong>
            </div>
            <div>
              <span>Reconnects</span>
              <strong>{snapshot.reconnectCount}</strong>
            </div>
          </div>

          <div className="actions">
            <button
              className="primary-action"
              disabled={busy || hasConnection || !desktopRuntime}
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
              disabled={busy || !canDisconnect}
              onClick={() => void runAction(disconnect)}
              type="button"
            >
              Отключить
            </button>
          </div>

          <div className="mock-scenarios">
            <span>Mock scenarios</span>
            <div>
              <button
                disabled={!isConnected}
                onClick={() => void runMockAction(triggerMockReset)}
                type="button"
              >
                Reset banner
              </button>
              <button
                disabled={!isConnected}
                onClick={() => void runMockAction(() => triggerMockAlarm(3))}
                type="button"
              >
                ALARM:3
              </button>
              <button
                disabled={!isConnected || !snapshot.alarm}
                onClick={() => void runMockAction(clearMockAlarm)}
                type="button"
              >
                Clear alarm
              </button>
              <button
                disabled={!isConnected}
                onClick={() => void runMockAction(triggerMockTimeout)}
                type="button"
              >
                Timeout ×2
              </button>
              <button
                disabled={!isConnected}
                onClick={() => void runMockAction(triggerMockDisconnect)}
                type="button"
              >
                Link drop
              </button>
            </div>
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
        <span>Poll: #{snapshot.pollSequence}</span>
        <span>Core: Rust</span>
      </footer>
    </div>
  );
}
