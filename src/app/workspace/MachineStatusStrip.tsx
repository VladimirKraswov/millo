import { KeyRound, Crosshair } from "lucide-react";
import { PositionReadout } from "../../components/PositionReadout";
import { ProbeIndicator } from "../../features/probe/ProbeIndicator";
import { RealtimeControls } from "../../features/machine-control/RealtimeControls";
import type { ControllerSnapshot, MachineMode, Position } from "../../shared/machine";

const modes: Record<MachineMode, string> = {
  unknown: "Нет связи", idle: "Готов", run: "Выполнение", hold: "Пауза",
  jog: "Перемещение", alarm: "Авария", door: "Дверь открыта", check: "Проверка",
  home: "Базирование", sleep: "Сон",
};

interface MachineStatusStripProps {
  snapshot: ControllerSnapshot;
  position?: Position;
  coordinateSystem: string;
  desktopRuntime: boolean;
  busy: boolean;
  onProbe: () => void;
  onZero: () => void;
  onUnlock: () => void;
  onAcknowledgeReset: () => void;
  onSnapshot: (value: ControllerSnapshot) => void;
  onError: (value?: string) => void;
  onReset: () => void;
}

export function MachineStatusStrip(props: MachineStatusStripProps) {
  const { snapshot } = props;
  const connected = snapshot.connection === "connected";
  return (
    <section className="machine-status-strip" aria-label="Состояние и координаты станка">
      <div className={`machine-status-summary is-${connected ? snapshot.machine.mode : "unknown"}`}>
        <span className="machine-status-light" aria-hidden="true" />
        <div><strong>{connected ? modes[snapshot.machine.mode] : "Нет связи"}</strong>
          <small>{snapshot.alarm ? `ALARM ${snapshot.alarm.code ?? ""}` : snapshot.resetNotice ? "Контроллер перезапущен" : `${props.coordinateSystem} · Рабочие координаты`}</small>
        </div>
        {snapshot.alarm && <button aria-label="Разблокировать станок" disabled={props.busy || !props.desktopRuntime} onClick={props.onUnlock} title="Разблокировать станок" type="button"><KeyRound size={16} /></button>}
        {snapshot.resetNotice && !snapshot.alarm && <button onClick={props.onAcknowledgeReset} disabled={props.busy || !props.desktopRuntime} type="button">OK</button>}
      </div>
      <PositionReadout position={connected ? props.position : undefined} />
      <div className="datum-actions">
        <button aria-label="Рабочий ноль" onClick={props.onZero} title="Установить или вернуть рабочий ноль" type="button"><Crosshair size={18} /></button>
        <ProbeIndicator active={snapshot.machine.pins?.probe ?? false} connection={snapshot.connection} onClick={props.onProbe} />
      </div>
      <RealtimeControls snapshot={snapshot} desktopRuntime={props.desktopRuntime} onSnapshot={props.onSnapshot} onError={props.onError} onReset={props.onReset} />
    </section>
  );
}
