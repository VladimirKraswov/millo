import {
  ChevronDown,
  PlugZap,
  RefreshCw,
  ScrollText,
  SquareTerminal,
  Unplug,
} from "lucide-react";
import type { ReactNode } from "react";

import type { ControllerSnapshot, TransportDescriptor } from "../../shared/machine";

export const connectionLabels = {
  disconnected: "Отключено",
  connecting: "Подключение",
  connected: "Подключено",
  recovering: "Восстановление",
  faulted: "Ошибка",
} as const;

const baudRates = [9_600, 19_200, 38_400, 57_600, 115_200, 230_400];

export interface ConnectionPanelView {
  readonly baudRate: number;
  readonly canDisconnect: boolean;
  readonly controlsBusy: boolean;
  readonly desktopRuntime: boolean;
  readonly discovering: boolean;
  readonly displayedError?: string;
  readonly displayedTransport: TransportDescriptor;
  readonly hasConnection: boolean;
  readonly isConnected: boolean;
  readonly likelyGrblOnly: boolean;
  readonly selectedMachineName?: string;
  readonly selectedTransport: TransportDescriptor;
  readonly snapshot: ControllerSnapshot;
  readonly transportLocked: boolean;
  readonly visibleTransports: readonly TransportDescriptor[];
}

export interface ConnectionPanelActions {
  readonly onBaudRate: (baudRate: number) => void;
  readonly onConnect: () => void;
  readonly onDisconnect: () => void;
  readonly onDismissError: () => void;
  readonly onLikelyGrblOnly: (enabled: boolean) => void;
  readonly onOpenLog: () => void;
  readonly onOpenConsole: () => void;
  readonly onRefreshStatus: () => void;
  readonly onRefreshTransports: () => void;
  readonly onTransport: (transportId: string) => void;
}

interface ConnectionPanelProps {
  readonly actions: ConnectionPanelActions;
  readonly controls?: ReactNode;
  readonly view: ConnectionPanelView;
}

export function ConnectionPanel({ actions, controls, view }: ConnectionPanelProps) {
  const {
    baudRate,
    canDisconnect,
    controlsBusy,
    desktopRuntime,
    discovering,
    displayedError,
    displayedTransport,
    hasConnection,
    isConnected,
    likelyGrblOnly,
    selectedMachineName,
    selectedTransport,
    snapshot,
    transportLocked,
    visibleTransports,
  } = view;

  const transportSelectionDisabled =
    transportLocked || controlsBusy || discovering || !desktopRuntime;

  return (
    <aside className="control-panel" aria-label="Управление подключением">
      <div className="panel-title">
        <span>Подключение</span>
        <strong>{displayedTransport.label}</strong>
        <small>
          {selectedMachineName
            ? `Станок: ${selectedMachineName}`
            : "Сначала добавьте или выберите станок"}
        </small>
      </div>

      <div className="connection-primary">
        {hasConnection ? (
          <>
            <div className={`connection-inline is-${snapshot.connection}`}>
              <i aria-hidden="true" />
              <span>{connectionLabels[snapshot.connection]}</span>
            </div>
            <button
              aria-label="Отключить"
              className="disconnect-action"
              disabled={controlsBusy || !canDisconnect}
              onClick={actions.onDisconnect}
              title="Отключить"
              type="button"
            >
              <Unplug aria-hidden="true" size={15} />
            </button>
          </>
        ) : (
          <button
            className="primary-action"
            disabled={controlsBusy || !desktopRuntime || !selectedTransport.id}
            onClick={actions.onConnect}
            type="button"
          >
            <PlugZap aria-hidden="true" size={15} />
            Подключить
          </button>
        )}
      </div>

      <div className="connection-utility-actions">
        <button className="log-open-action" onClick={actions.onOpenLog} type="button">
          <ScrollText aria-hidden="true" size={14} />
          <span>
            <strong>Журнал</strong>
            <small>События и ошибки</small>
          </span>
        </button>
        <button className="console-open-action" onClick={actions.onOpenConsole} type="button">
          <SquareTerminal aria-hidden="true" size={14} />
          <span>
            <strong>Консоль</strong>
            <small>Безопасные запросы</small>
          </span>
        </button>
      </div>

      {hasConnection && controls}

      <details className="control-disclosure" open={!hasConnection}>
        <summary>
          <span>Параметры подключения</span>
          <ChevronDown aria-hidden="true" size={14} />
        </summary>
        <div className="transport-config">
          <label className="transport-filter">
            <input
              checked={likelyGrblOnly}
              disabled={controlsBusy || discovering}
              onChange={(event) => actions.onLikelyGrblOnly(event.target.checked)}
              type="checkbox"
            />
            <span>Только вероятные GRBL</span>
          </label>
          <label htmlFor="transport-select">Устройство</label>
          <div className="transport-select-row">
            <select
              id="transport-select"
              disabled={transportSelectionDisabled}
              onChange={(event) => actions.onTransport(event.target.value)}
              value={selectedTransport.id}
            >
              {visibleTransports.length === 0 && (
                <option value="">Порты не найдены</option>
              )}
              {visibleTransports.map((transport) => (
                <option key={transport.id} value={transport.id}>
                  {transport.label}
                </option>
              ))}
            </select>
            <button
              aria-label="Обновить список портов"
              disabled={transportSelectionDisabled}
              onClick={actions.onRefreshTransports}
              title="Обновить список портов"
              type="button"
            >
              <RefreshCw aria-hidden="true" size={15} />
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
              <span>Скорость порта</span>
              <select
                id="baud-rate"
                disabled={transportLocked || controlsBusy}
                onChange={(event) => actions.onBaudRate(Number(event.target.value))}
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
      </details>

      {hasConnection && (
        <details className="control-disclosure diagnostics-disclosure">
          <summary>
            <span>Диагностика соединения</span>
            <ChevronDown aria-hidden="true" size={14} />
          </summary>
          <div className="lifecycle-metrics">
            <Metric label="Опрос" value={`${snapshot.pollIntervalMs || "--"} ms`} />
            <Metric label="Тайм-аут" value={`${snapshot.statusTimeoutMs || "--"} ms`} />
            <Metric
              label="Сбои"
              value={`${snapshot.consecutiveFailures}/${snapshot.failureThreshold || "--"}`}
            />
            <Metric label="Переподключения" value={String(snapshot.reconnectCount)} />
          </div>
          <button
            className="status-request-action"
            disabled={controlsBusy || !isConnected}
            onClick={actions.onRefreshStatus}
            type="button"
          >
            <RefreshCw aria-hidden="true" size={14} />
            Запросить статус
            <kbd>?</kbd>
          </button>

        </details>
      )}

      {!desktopRuntime && <p className="runtime-note">Управление доступно в окне Tauri.</p>}
      {displayedError && (
        <div className="error-note" role="alert">
          <span>{displayedError}</span>
          <div>
            <button onClick={actions.onOpenLog} type="button">
              Журнал
            </button>
            <button aria-label="Закрыть ошибку" onClick={actions.onDismissError} type="button">
              ×
            </button>
          </div>
        </div>
      )}
    </aside>
  );
}

function Metric({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
