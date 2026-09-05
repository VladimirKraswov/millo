import { useEffect, useState } from "react";
import { Pause, RotateCcw, Square } from "lucide-react";
import { cancelJog, confirmSoftReset, feedHold, requestSoftReset } from "../../api/controller";
import { isControllerConnected } from "../../shared/controllerReadiness";
import type { ControllerSnapshot, ResetChallenge } from "../../shared/machine";

export interface RealtimeControlsProps {
  snapshot: ControllerSnapshot;
  desktopRuntime: boolean;
  onSnapshot: (snapshot: ControllerSnapshot) => void;
  onError: (error?: string) => void;
  onReset?: () => void;
}

export function RealtimeControls({ snapshot, desktopRuntime, onSnapshot, onError, onReset }: RealtimeControlsProps) {
  const [holdPending, setHoldPending] = useState(false);
  const [resetPending, setResetPending] = useState(false);
  const [cancelPending, setCancelPending] = useState(false);
  const [challenge, setChallenge] = useState<ResetChallenge>();
  const connected = isControllerConnected(snapshot);

  useEffect(() => {
    if (!challenge) return;
    const timer = window.setTimeout(() => setChallenge(undefined), challenge.expiresInMs);
    return () => window.clearTimeout(timer);
  }, [challenge]);

  useEffect(() => setChallenge(undefined), [connected, snapshot.resetCount, snapshot.reconnectCount]);

  const execute = async (action: () => Promise<ControllerSnapshot>, pending: (value: boolean) => void) => {
    pending(true);
    try { onSnapshot(await action()); } catch (error) { onError(String(error)); }
    finally { pending(false); }
  };

  const reset = async () => {
    setResetPending(true);
    try {
      if (challenge) {
        onReset?.();
        onSnapshot(await confirmSoftReset(challenge.id));
        setChallenge(undefined);
      } else {
        setChallenge(await requestSoftReset());
      }
    } catch (error) { onError(String(error)); }
    finally { setResetPending(false); }
  };

  return (
    <div className="safety-actions realtime-controls" role="group" aria-label="Остановка станка">
      <button className="hold-action" disabled={!desktopRuntime || !connected || holdPending || !["run", "jog", "home"].includes(snapshot.machine.mode)} onClick={() => void execute(feedHold, setHoldPending)} title="Приостановить движение (Hold)" type="button">
        <Pause aria-hidden="true" size={16} /><span>Пауза</span>
      </button>
      <button className={`reset-action${challenge ? " is-confirming" : ""}`} disabled={!desktopRuntime || !connected || resetPending} onClick={() => void reset()} title={challenge ? "Нажмите ещё раз для сброса контроллера" : "Сбросить контроллер (Reset)"} type="button">
        <RotateCcw aria-hidden="true" size={16} /><span>{challenge ? "Сбросить?" : "Reset"}</span>
      </button>
      <button className="jog-cancel-action" aria-label="Отменить jog" disabled={!desktopRuntime || !connected || cancelPending || snapshot.machine.mode !== "jog"} onClick={() => void execute(cancelJog, setCancelPending)} title="Остановить ручное перемещение" type="button">
        <Square aria-hidden="true" size={15} /><span>Стоп jog</span>
      </button>
    </div>
  );
}
