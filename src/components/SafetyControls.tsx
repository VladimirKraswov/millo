import { useEffect, useState } from "react";

import {
  cancelJog,
  confirmSoftReset,
  feedHold,
  requestSoftReset,
} from "../api/controller";
import {
  uiSlots,
  type UiExtensionRegistry,
} from "../platform/extensions/UiExtensionRegistry";
import { UiExtensionSlot } from "../platform/extensions/UiExtensionSlot";
import type { MachineCommandGateway } from "../platform/machine/MachineCommandGateway";
import type { WorkCoordinateGateway } from "../platform/machine/WorkCoordinateGateway";
import type {
  ControllerSnapshot,
  HardwareInspection,
  ResetChallenge,
} from "../shared/machine";

interface SafetyControlsProps {
  snapshot: ControllerSnapshot;
  desktopRuntime: boolean;
  extensionRegistry: UiExtensionRegistry;
  machineGateway: MachineCommandGateway;
  workCoordinateGateway: WorkCoordinateGateway;
  machineBound: boolean;
  onSnapshot: (snapshot: ControllerSnapshot) => void;
  onInspection: (inspection?: HardwareInspection) => void;
  onError: (error?: string) => void;
}

const secondsRemaining = (deadline: number | undefined, now: number): number =>
  deadline === undefined ? 0 : Math.max(0, Math.ceil((deadline - now) / 1_000));

export function SafetyControls({
  snapshot,
  desktopRuntime,
  extensionRegistry,
  machineGateway,
  workCoordinateGateway,
  machineBound,
  onSnapshot,
  onInspection,
  onError,
}: SafetyControlsProps) {
  const [busy, setBusy] = useState(false);
  const [holdPending, setHoldPending] = useState(false);
  const [challenge, setChallenge] = useState<ResetChallenge>();
  const [challengeDeadline, setChallengeDeadline] = useState<number>();
  const [now, setNow] = useState(() => Date.now());

  const connected = snapshot.connection === "connected";
  const stableIdle =
    connected &&
    snapshot.machine.mode === "idle" &&
    snapshot.alarm === undefined &&
    snapshot.resetNotice === undefined;
  const canHold =
    connected && ["run", "jog", "home"].includes(snapshot.machine.mode);
  const challengeSeconds = secondsRemaining(challengeDeadline, now);

  useEffect(() => {
    if (!challenge) return;
    const timer = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(timer);
  }, [challenge]);

  useEffect(() => {
    if (!connected) {
      setChallenge(undefined);
      setChallengeDeadline(undefined);
    }
  }, [connected]);

  useEffect(() => {
    if (challenge && challengeSeconds === 0) {
      setChallenge(undefined);
      setChallengeDeadline(undefined);
    }
  }, [challenge, challengeSeconds]);

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    onError(undefined);
    try {
      await action();
    } catch (error) {
      onError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const sendHold = async () => {
    setHoldPending(true);
    onError(undefined);
    try {
      onSnapshot(await feedHold());
    } catch (error) {
      onError(String(error));
    } finally {
      setHoldPending(false);
    }
  };

  const sendJogCancel = () =>
    run(async () => {
      onSnapshot(await cancelJog());
    });

  const beginReset = () =>
    run(async () => {
      const next = await requestSoftReset();
      const requestedAt = Date.now();
      setNow(requestedAt);
      setChallenge(next);
      setChallengeDeadline(requestedAt + next.expiresInMs);
    });

  const executeReset = () =>
    run(async () => {
      if (!challenge) return;
      onInspection(undefined);
      onSnapshot(await confirmSoftReset(challenge.id));
      setChallenge(undefined);
      setChallengeDeadline(undefined);
    });

  return (
    <section className="safety-controls" aria-labelledby="safety-title">
      <div className="safety-heading">
        <div>
          <span>Realtime safety</span>
          <strong id="safety-title">Safety controls</strong>
        </div>
        <small>{stableIdle ? "Idle" : snapshot.machine.reportedMode}</small>
      </div>

      <div className="safety-actions">
        <button
          className="hold-action"
          disabled={!desktopRuntime || !canHold || holdPending}
          onClick={() => void sendHold()}
          type="button"
        >
          <span aria-hidden="true">II</span>
          Feed Hold
        </button>
        <button
          className="reset-action"
          disabled={!desktopRuntime || !connected || busy}
          onClick={() => void beginReset()}
          type="button"
        >
          <span aria-hidden="true">↻</span>
          Soft Reset
        </button>
        {snapshot.machine.mode === "jog" && (
          <button
            className="jog-cancel-action"
            disabled={!desktopRuntime || busy}
            onClick={() => void sendJogCancel()}
            type="button"
          >
            <span aria-hidden="true">■</span>
            Jog Cancel
          </button>
        )}
      </div>

      {challenge && (
        <div className="reset-confirmation" role="alert">
          <div>
            <strong>Подтвердить Ctrl-X Reset</strong>
            <span>{challengeSeconds} сек</span>
          </div>
          <div>
            <button disabled={busy} onClick={() => void executeReset()} type="button">
              Подтвердить
            </button>
            <button
              disabled={busy}
              onClick={() => {
                setChallenge(undefined);
                setChallengeDeadline(undefined);
              }}
              type="button"
            >
              Отмена
            </button>
          </div>
        </div>
      )}

      <UiExtensionSlot
        context={{
          snapshot,
          desktopRuntime,
          controlsDisabled: busy || holdPending || !machineBound,
          machineCommands: machineGateway,
          workCoordinates: workCoordinateGateway,
          updateSnapshot: onSnapshot,
          updateInspection: onInspection,
          reportError: onError,
        }}
        registry={extensionRegistry}
        slot={uiSlots.controlMachine}
      />
      <UiExtensionSlot
        context={{
          snapshot,
          desktopRuntime,
          controlsDisabled: busy || holdPending || !machineBound,
          machineCommands: machineGateway,
          workCoordinates: workCoordinateGateway,
          updateSnapshot: onSnapshot,
          updateInspection: onInspection,
          reportError: onError,
        }}
        registry={extensionRegistry}
        slot={uiSlots.controlCoordinates}
      />
    </section>
  );
}
