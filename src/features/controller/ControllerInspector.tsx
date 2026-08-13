import { ReadinessPanel } from "../../components/ReadinessPanel";
import type { HardwareInspection } from "../../shared/machine";

interface ControllerInspectorProps {
  readonly busy: boolean;
  readonly connected: boolean;
  readonly inspecting: boolean;
  readonly inspection?: HardwareInspection;
  readonly onRead: () => void;
}

export function ControllerInspector({
  busy,
  connected,
  inspecting,
  inspection,
  onRead,
}: ControllerInspectorProps) {
  return (
    <section className="device-inspector" aria-labelledby="inspector-title">
      <div className="inspector-heading">
        <div>
          <span>Только чтение</span>
          <h2 id="inspector-title">Состояние контроллера</h2>
        </div>
        <button disabled={!connected || busy} onClick={onRead} type="button">
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
                  <span>Прошивка</span>
                  <strong>
                    {inspection.device.firmwareVersion ?? "Неизвестная версия GRBL"}
                  </strong>
                  <small>
                    {inspection.device.firmwareBuildInfo ?? "Нет сведений о сборке"}
                  </small>
                </div>
                <dl className="inspection-meta">
                  <div>
                    <dt>Возможности</dt>
                    <dd title={inspection.device.firmwareOptions}>
                      {inspection.device.controllerCapabilities
                        ? `${inspection.device.controllerCapabilities.optionFlags} · P${inspection.device.controllerCapabilities.plannerBufferBlocks ?? "?"} · RX${inspection.device.controllerCapabilities.rxBufferBytes ?? "?"}`
                        : (inspection.device.firmwareOptions ?? "--")}
                    </dd>
                  </div>
                  <div>
                    <dt>Настройки</dt>
                    <dd>{Object.keys(inspection.device.settings).length}</dd>
                  </div>
                  <div>
                    <dt>Параметры</dt>
                    <dd>{Object.keys(inspection.device.parameters).length}</dd>
                  </div>
                </dl>
                <div className="modal-state">
                  <span>Модальное состояние</span>
                  <div>
                    {inspection.device.modalState.map((mode) => (
                      <code key={mode}>{mode}</code>
                    ))}
                  </div>
                </div>
                <div className="query-results" aria-label="Результаты запросов к контроллеру">
                  {inspection.device.responses.map((response) => (
                    <div className={`is-${response.completion}`} key={response.command}>
                      <code>{response.command}</code>
                      <strong>
                        {response.completion}
                        {response.code === undefined ? "" : `:${response.code}`}
                      </strong>
                    </div>
                  ))}
                </div>
              </div>

              <div className="inspector-registers">
                <RegisterList title="Настройки контроллера" values={inspection.device.settings} />
                <RegisterList title="Системы координат" values={inspection.device.parameters} />
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
  );
}

function RegisterList({
  title,
  values,
}: {
  readonly title: string;
  readonly values: Readonly<Record<string, string>>;
}) {
  return (
    <div>
      <span>{title}</span>
      <div className="register-list">
        {Object.entries(values).map(([key, value]) => (
          <div key={key}>
            <code>{key}</code>
            <strong>{value}</strong>
          </div>
        ))}
      </div>
    </div>
  );
}
