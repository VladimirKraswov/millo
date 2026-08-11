import { useEffect, useMemo, useState, useSyncExternalStore } from "react";

import { bootstrapPluginHost } from "./app/bootstrapPluginHost";
import {
  acknowledgeReset,
  clearMockAlarm,
  connectTransport,
  disconnect,
  getActiveTransport,
  inspectDevice,
  isDesktopRuntime,
  listTransports,
  refreshStatus,
  triggerMockAlarm,
  triggerMockDisconnect,
  triggerMockRun,
  triggerMockReset,
  triggerMockTimeout,
} from "./api/controller";
import { ReadinessPanel } from "./components/ReadinessPanel";
import { SafetyControls } from "./components/SafetyControls";
import { bindMachineStateStream } from "./platform/machine/MachineStateEventStream";
import { tauriMachineCommandGateway } from "./platform/machine/tauriMachineCommandGateway";
import { tauriMachineStateEventStream } from "./platform/machine/tauriMachineStateEventStream";
import { tauriWorkCoordinateGateway } from "./platform/machine/tauriWorkCoordinateGateway";
import {
  emptySnapshot,
  type ControllerSnapshot,
  type HardwareInspection,
  type Position,
  type TransportDescriptor,
} from "./shared/machine";

const connectionLabels = {
  disconnected: "Отключено",
  connecting: "Подключение",
  connected: "Подключено",
  recovering: "Восстановление",
  faulted: "Ошибка",
} as const;

const mockTransport: TransportDescriptor = {
  id: "mock",
  kind: "mock",
  label: "Mock GRBL",
  detail: "Deterministic test controller",
  likelyGrbl: true,
  matchReason: "Built-in test controller",
};

const baudRates = [9_600, 19_200, 38_400, 57_600, 115_200, 230_400];

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
  const pluginHost = useMemo(
    () =>
      bootstrapPluginHost({
        initialSnapshot: emptySnapshot,
        machineCommands: tauriMachineCommandGateway,
      }),
    [],
  );
  const snapshot = useSyncExternalStore(
    pluginHost.machineState.subscribe,
    pluginHost.machineState.current,
    pluginHost.machineState.current,
  );
  const [transports, setTransports] = useState<TransportDescriptor[]>([
    mockTransport,
  ]);
  const [selectedTransportId, setSelectedTransportId] = useState("mock");
  const [activeTransport, setActiveTransport] =
    useState<TransportDescriptor>(mockTransport);
  const [baudRate, setBaudRate] = useState(115_200);
  const [likelyGrblOnly, setLikelyGrblOnly] = useState(true);
  const [inspection, setInspection] = useState<HardwareInspection>();
  const [inspecting, setInspecting] = useState(false);
  const [busy, setBusy] = useState(false);
  const [discovering, setDiscovering] = useState(false);
  const [uiError, setUiError] = useState<string>();
  const desktopRuntime = useMemo(isDesktopRuntime, []);

  useEffect(() => {
    if (!desktopRuntime) {
      return;
    }

    let active = true;
    const unbindMachineState = bindMachineStateStream({
      stream: tauriMachineStateEventStream,
      store: pluginHost.machineState,
      onSnapshot: (value) => {
        if (!active) return;
        if (
          value.connection !== "connected" ||
          value.machine.mode !== "idle" ||
          value.alarm !== undefined ||
          value.resetNotice !== undefined
        ) {
          setInspection(undefined);
        }
        if (value.connection === "connected" && value.consecutiveFailures === 0) {
          setUiError(undefined);
        }
      },
      onError: (error) => {
        if (active) setUiError(String(error));
      },
    });
    void getActiveTransport().then((value) => {
      if (active) {
        setActiveTransport(value);
        setSelectedTransportId(value.id);
      }
    });
    void listTransports()
      .then((value) => {
        if (active) setTransports(value);
      })
      .catch((error: unknown) => {
        if (active) setUiError(String(error));
      });
    return () => {
      active = false;
      unbindMachineState();
    };
  }, [desktopRuntime, pluginHost]);

  const runAction = async (
    action: () => Promise<ControllerSnapshot>,
  ): Promise<boolean> => {
    setBusy(true);
    setUiError(undefined);
    try {
      pluginHost.machineState.publish(await action());
      return true;
    } catch (error) {
      setUiError(String(error));
      return false;
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

  const discoverTransports = async () => {
    setDiscovering(true);
    setUiError(undefined);
    try {
      const discovered = await listTransports();
      setTransports(
        activeTransport.kind === "serial" &&
          !discovered.some((transport) => transport.id === activeTransport.id)
          ? [...discovered, activeTransport]
          : discovered,
      );
    } catch (error) {
      setUiError(String(error));
    } finally {
      setDiscovering(false);
    }
  };

  const isConnected = snapshot.connection === "connected";
  const hasConnection =
    snapshot.connection === "connected" || snapshot.connection === "recovering";
  const canDisconnect =
    snapshot.connection !== "disconnected" && snapshot.connection !== "connecting";
  const transportLocked = hasConnection || snapshot.connection === "connecting";
  const visibleTransports = transports.filter(
    (transport) =>
      !likelyGrblOnly ||
      transport.likelyGrbl ||
      (transportLocked && transport.id === activeTransport.id),
  );
  const selectedTransport =
    visibleTransports.find((transport) => transport.id === selectedTransportId) ??
    visibleTransports[0] ??
    activeTransport;
  const displayedTransport = transportLocked ? activeTransport : selectedTransport;
  const displayedError = uiError ?? snapshot.lastError;
  const controlsBusy = busy || inspecting;

  const readDeviceInspection = async () => {
    setInspecting(true);
    setUiError(undefined);
    try {
      setInspection(await inspectDevice());
    } catch (error) {
      setUiError(String(error));
    } finally {
      setInspecting(false);
    }
  };

  const connectSelectedTransport = async () => {
    setInspection(undefined);
    const connected = await runAction(() =>
      connectTransport(selectedTransport.id, baudRate),
    );
    if (connected) {
      setActiveTransport(selectedTransport);
      await readDeviceInspection();
    }
  };

  const disconnectController = async () => {
    const disconnected = await runAction(disconnect);
    if (disconnected) setInspection(undefined);
  };

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            <i />
          </span>
          <div>
            <strong>Millo</strong>
            <span>Machine control</span>
          </div>
        </div>

        <div className={`connection-state is-${snapshot.connection}`}>
          <span className="state-dot" />
          <div>
            <small>{displayedTransport.label}</small>
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
              <button type="button" onClick={() => void runAction(acknowledgeReset)}>
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

          <section className="device-inspector" aria-labelledby="inspector-title">
            <div className="inspector-heading">
              <div>
                <span>Read-only</span>
                <h2 id="inspector-title">Device Inspector</h2>
              </div>
              <button
                disabled={!isConnected || controlsBusy}
                onClick={() => void readDeviceInspection()}
                type="button"
              >
                <span>{inspecting ? "Чтение" : "Считать"}</span>
                <code>$I · $$ · $G · $#</code>
              </button>
            </div>

            {inspection ? (
              <>
                <ReadinessPanel report={inspection.readiness} />
                <details className="technical-inspection">
                  <summary>Технические данные контроллера</summary>
                  <div className="inspector-content">
                    <div className="inspector-identity">
                      <div className="firmware-readout">
                        <span>Firmware</span>
                        <strong>
                          {inspection.device.firmwareVersion ?? "Unknown GRBL"}
                        </strong>
                        <small>
                          {inspection.device.firmwareBuildInfo ?? "No build info"}
                        </small>
                      </div>
                      <dl className="inspection-meta">
                        <div>
                          <dt>Options</dt>
                          <dd>{inspection.device.firmwareOptions ?? "--"}</dd>
                        </div>
                        <div>
                          <dt>Settings</dt>
                          <dd>{Object.keys(inspection.device.settings).length}</dd>
                        </div>
                        <div>
                          <dt>Parameters</dt>
                          <dd>
                            {Object.keys(inspection.device.parameters).length}
                          </dd>
                        </div>
                      </dl>
                      <div className="modal-state">
                        <span>Modal state</span>
                        <div>
                          {inspection.device.modalState.map((mode) => (
                            <code key={mode}>{mode}</code>
                          ))}
                        </div>
                      </div>
                      <div
                        className="query-results"
                        aria-label="Device query results"
                      >
                        {inspection.device.responses.map((response) => (
                          <div
                            className={`is-${response.completion}`}
                            key={response.command}
                          >
                            <code>{response.command}</code>
                            <strong>
                              {response.completion}
                              {response.code !== undefined
                                ? `:${response.code}`
                                : ""}
                            </strong>
                          </div>
                        ))}
                      </div>
                    </div>

                    <div className="inspector-registers">
                      <div>
                        <span>Controller settings</span>
                        <div className="register-list">
                          {Object.entries(inspection.device.settings).map(
                            ([key, value]) => (
                              <div key={key}>
                                <code>{key}</code>
                                <strong>{value}</strong>
                              </div>
                            ),
                          )}
                        </div>
                      </div>
                      <div>
                        <span>Coordinate parameters</span>
                        <div className="register-list">
                          {Object.entries(inspection.device.parameters).map(
                            ([key, value]) => (
                              <div key={key}>
                                <code>{key}</code>
                                <strong>{value}</strong>
                              </div>
                            ),
                          )}
                        </div>
                      </div>
                    </div>
                  </div>
                </details>
              </>
            ) : (
              <div className="inspector-empty">
                <strong>Профиль контроллера не считан</strong>
                <span>Движение и управление шпинделем недоступны</span>
              </div>
            )}
          </section>

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
            <strong>{displayedTransport.label}</strong>
          </div>

          <SafetyControls
            desktopRuntime={desktopRuntime}
            extensionRegistry={pluginHost.uiRegistry}
            machineGateway={tauriMachineCommandGateway}
            workCoordinateGateway={tauriWorkCoordinateGateway}
            onError={setUiError}
            onInspection={setInspection}
            onSnapshot={pluginHost.machineState.publish}
            snapshot={snapshot}
          />

          <div className="transport-config">
            <label className="transport-filter">
              <input
                checked={likelyGrblOnly}
                disabled={controlsBusy || discovering}
                onChange={(event) => setLikelyGrblOnly(event.target.checked)}
                type="checkbox"
              />
              <span>Только вероятные GRBL</span>
            </label>
            <label htmlFor="transport-select">Device</label>
            <div className="transport-select-row">
              <select
                id="transport-select"
                disabled={
                  transportLocked || controlsBusy || discovering || !desktopRuntime
                }
                onChange={(event) => setSelectedTransportId(event.target.value)}
                value={selectedTransport.id}
              >
                {visibleTransports.map((transport) => (
                  <option key={transport.id} value={transport.id}>
                    {transport.label}
                  </option>
                ))}
              </select>
              <button
                aria-label="Обновить список портов"
                disabled={
                  transportLocked || controlsBusy || discovering || !desktopRuntime
                }
                onClick={() => void discoverTransports()}
                title="Обновить список портов"
                type="button"
              >
                ↻
              </button>
            </div>
            <small>
              {selectedTransport.detail}
              {selectedTransport.kind === "serial" &&
                selectedTransport.matchReason &&
                ` · ${selectedTransport.matchReason}`}
            </small>

            {selectedTransport.kind === "serial" && (
              <label className="baud-field" htmlFor="baud-rate">
                <span>Baud rate</span>
                <select
                  id="baud-rate"
                  disabled={transportLocked || controlsBusy}
                  onChange={(event) => setBaudRate(Number(event.target.value))}
                  value={baudRate}
                >
                  {baudRates.map((rate) => (
                    <option key={rate} value={rate}>
                      {rate.toLocaleString("en-US", { useGrouping: false })}
                    </option>
                  ))}
                </select>
              </label>
            )}
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
              disabled={controlsBusy || hasConnection || !desktopRuntime}
              onClick={() => void connectSelectedTransport()}
              type="button"
            >
              {hasConnection ? connectionLabels[snapshot.connection] : "Подключить"}
            </button>
            <button
              disabled={controlsBusy || !isConnected}
              onClick={() => void runAction(refreshStatus)}
              type="button"
            >
              Запросить статус
              <kbd>?</kbd>
            </button>
            <button
              disabled={controlsBusy || !canDisconnect}
              onClick={() => void disconnectController()}
              type="button"
            >
              Отключить
            </button>
          </div>

          {displayedTransport.kind === "mock" && (
            <div className="mock-scenarios">
              <span>Mock scenarios</span>
              <div>
                <button
                  disabled={controlsBusy || !isConnected}
                  onClick={() => void runMockAction(triggerMockRun)}
                  type="button"
                >
                  Run state
                </button>
                <button
                  disabled={controlsBusy || !isConnected}
                  onClick={() => void runMockAction(triggerMockReset)}
                  type="button"
                >
                  Reset banner
                </button>
                <button
                  disabled={controlsBusy || !isConnected}
                  onClick={() => void runMockAction(() => triggerMockAlarm(3))}
                  type="button"
                >
                  ALARM:3
                </button>
                <button
                  disabled={controlsBusy || !isConnected || !snapshot.alarm}
                  onClick={() => void runMockAction(clearMockAlarm)}
                  type="button"
                >
                  Clear alarm
                </button>
                <button
                  disabled={controlsBusy || !isConnected}
                  onClick={() => void runMockAction(triggerMockTimeout)}
                  type="button"
                >
                  Timeout ×2
                </button>
                <button
                  disabled={controlsBusy || !isConnected}
                  onClick={() => void runMockAction(triggerMockDisconnect)}
                  type="button"
                >
                  Link drop
                </button>
              </div>
            </div>
          )}

          <div className="pipeline" aria-label="Active data path">
            <span>{displayedTransport.kind === "serial" ? "Serial" : "Mock"}</span>
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
