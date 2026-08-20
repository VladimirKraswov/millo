import { Crosshair, Home, ShieldCheck } from "lucide-react";
import { useEffect, useState } from "react";

import { startHoming } from "../../api/controller";
import type { ControllerSnapshot } from "../../shared/machine";

interface MachineReferencePanelProps {
  desktopRuntime: boolean;
  disabled: boolean;
  homingInstalled: boolean;
  snapshot: ControllerSnapshot;
  onError: (error?: string) => void;
  onSnapshot: (snapshot: ControllerSnapshot) => void;
}

const referenceLabel = (snapshot: ControllerSnapshot, homingInstalled: boolean) => {
  if (!homingInstalled) return "Homing не настроен";
  switch (snapshot.homing.state) {
    case "homing":
      return "Базирование выполняется";
    case "homed":
      return "Базирован в этой сессии";
    case "invalidated":
      return "Базирование утрачено после reset/reconnect";
    case "failed":
      return "Базирование не завершено";
    default:
      return "Станок ещё не базирован";
  }
};

const referenceDetail = (snapshot: ControllerSnapshot, homingInstalled: boolean) => {
  switch (snapshot.homing.state) {
    case "homing":
      return snapshot.homing.timeoutMs
        ? `Контроллер выполняет $H · таймаут ${Math.ceil(snapshot.homing.timeoutMs / 1_000)} с`
        : "Контроллер выполняет $H";
    case "homed":
      return "Границы по машинным координатам активны до reset или reconnect";
    case "invalidated":
      return "Перед движением по машинным границам выполните $H заново";
    case "failed":
      return snapshot.homing.detail ?? "Проверьте Alarm, концевики и настройки $22–$27";
    default:
      return homingInstalled
        ? "Машинные границы станут достоверными после $H"
        : "Jog ограничен профилем станка";
  }
};

export function MachineReferencePanel({
  desktopRuntime,
  disabled,
  homingInstalled,
  snapshot,
  onError,
  onSnapshot,
}: MachineReferencePanelProps) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const homing = snapshot.homing.state === "homing";
  const homed = snapshot.homing.state === "homed";
  const canStart =
    desktopRuntime &&
    homingInstalled &&
    !disabled &&
    !busy &&
    !homing &&
    snapshot.connection === "connected" &&
    ["idle", "alarm"].includes(snapshot.machine.mode);

  useEffect(() => {
    if (snapshot.connection !== "connected" || homing) setConfirming(false);
  }, [homing, snapshot.connection]);

  const homeMachine = async () => {
    setBusy(true);
    onError(undefined);
    try {
      const outcome = await startHoming({ operatorConfirmed: true });
      onSnapshot(outcome.snapshot);
      setConfirming(false);
    } catch (error) {
      onError(String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className={`machine-reference is-${snapshot.homing.state}`}>
      <div className="machine-reference-state">
        {homed ? <ShieldCheck aria-hidden="true" size={15} /> : <Crosshair aria-hidden="true" size={15} />}
        <span>
          <strong>{referenceLabel(snapshot, homingInstalled)}</strong>
          <small>{referenceDetail(snapshot, homingInstalled)}</small>
        </span>
      </div>
      {confirming ? (
        <div className="machine-reference-confirm">
          <span>Рабочая зона свободна, инструмент поднят</span>
          <button disabled={!canStart} onClick={() => void homeMachine()} type="button">
            Выполнить $H
          </button>
          <button onClick={() => setConfirming(false)} type="button">Отмена</button>
        </div>
      ) : (
        <button
          aria-label="Базировать станок"
          disabled={!canStart}
          onClick={() => setConfirming(true)}
          title={homingInstalled ? "Запустить типизированный цикл GRBL $H" : "Включите Homing в профиле станка"}
          type="button"
        >
          <Home aria-hidden="true" size={14} />
          Базировать
        </button>
      )}
    </section>
  );
}
