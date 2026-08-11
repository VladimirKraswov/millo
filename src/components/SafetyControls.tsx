import { useEffect, useMemo, useState } from "react";

import {
  cancelJog,
  confirmSoftReset,
  feedHold,
  prepareTestJog,
  requestSoftReset,
  stepJog,
} from "../api/controller";
import type {
  ControllerSnapshot,
  HardwareInspection,
  JogAxis,
  OperatorConfirmation,
  ResetChallenge,
  StepJogReceipt,
  TestJogPreparation,
} from "../shared/machine";

const emptyConfirmation: OperatorConfirmation = {
  spindleOff: false,
  toolClear: false,
  powerControlReachable: false,
};

interface SafetyControlsProps {
  snapshot: ControllerSnapshot;
  desktopRuntime: boolean;
  onSnapshot: (snapshot: ControllerSnapshot) => void;
  onInspection: (inspection?: HardwareInspection) => void;
  onError: (error?: string) => void;
}

const secondsRemaining = (deadline: number | undefined, now: number): number =>
  deadline === undefined ? 0 : Math.max(0, Math.ceil((deadline - now) / 1_000));

export function SafetyControls({
  snapshot,
  desktopRuntime,
  onSnapshot,
  onInspection,
  onError,
}: SafetyControlsProps) {
  const [busy, setBusy] = useState(false);
  const [holdPending, setHoldPending] = useState(false);
  const [challenge, setChallenge] = useState<ResetChallenge>();
  const [challengeDeadline, setChallengeDeadline] = useState<number>();
  const [confirmation, setConfirmation] =
    useState<OperatorConfirmation>(emptyConfirmation);
  const [preparation, setPreparation] = useState<TestJogPreparation>();
  const [authorizationDeadline, setAuthorizationDeadline] = useState<number>();
  const [jogAxis, setJogAxis] = useState<JogAxis>("x");
  const [jogDistance, setJogDistance] = useState(0.1);
  const [jogFeed, setJogFeed] = useState(50);
  const [lastJog, setLastJog] = useState<StepJogReceipt>();
  const [now, setNow] = useState(() => Date.now());

  const connected = snapshot.connection === "connected";
  const stableIdle =
    connected &&
    snapshot.machine.mode === "idle" &&
    snapshot.alarm === undefined &&
    snapshot.resetNotice === undefined;
  const canHold =
    connected && ["run", "jog", "home"].includes(snapshot.machine.mode);
  const confirmationComplete = Object.values(confirmation).every(Boolean);
  const challengeSeconds = secondsRemaining(challengeDeadline, now);
  const authorizationSeconds = secondsRemaining(authorizationDeadline, now);
  const authorizationActive =
    preparation?.authorization !== undefined && authorizationSeconds > 0;

  useEffect(() => {
    if (!challenge && !preparation?.authorization) return;
    const timer = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(timer);
  }, [challenge, preparation?.authorization]);

  useEffect(() => {
    if (!connected) {
      setChallenge(undefined);
      setChallengeDeadline(undefined);
    }
    if (!stableIdle) {
      setPreparation(undefined);
      setAuthorizationDeadline(undefined);
    }
  }, [connected, stableIdle]);

  useEffect(() => {
    if (challenge && challengeSeconds === 0) {
      setChallenge(undefined);
      setChallengeDeadline(undefined);
    }
  }, [challenge, challengeSeconds]);

  useEffect(() => {
    if (preparation?.authorization && authorizationSeconds === 0) {
      setPreparation((current) =>
        current ? { ...current, authorization: undefined } : undefined,
      );
      setAuthorizationDeadline(undefined);
    }
  }, [authorizationSeconds, preparation?.authorization]);

  const checklist = useMemo(
    () =>
      [
        ["spindleOff", "Шпиндель физически выключен"],
        ["toolClear", "Инструмент не касается заготовки"],
        ["powerControlReachable", "Питание станка доступно оператору"],
      ] as const,
    [],
  );

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
      setPreparation(undefined);
      setAuthorizationDeadline(undefined);
    } catch (error) {
      onError(String(error));
    } finally {
      setHoldPending(false);
    }
  };

  const sendJogCancel = () =>
    run(async () => {
      onSnapshot(await cancelJog());
      setPreparation(undefined);
      setAuthorizationDeadline(undefined);
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
      setPreparation(undefined);
      setAuthorizationDeadline(undefined);
    });

  const runPreflight = () =>
    run(async () => {
      const next = await prepareTestJog(confirmation);
      const checkedAt = Date.now();
      onInspection(next.inspection);
      setNow(checkedAt);
      setPreparation(next);
      setAuthorizationDeadline(
        next.authorization
          ? checkedAt + next.authorization.expiresInMs
          : undefined,
      );
      setLastJog(undefined);
    });

  const executeStepJog = (direction: -1 | 1) => {
    const authorization = preparation?.authorization;
    if (!authorizationActive || !authorization) return;

    setPreparation((current) =>
      current ? { ...current, authorization: undefined } : undefined,
    );
    setAuthorizationDeadline(undefined);
    void run(async () => {
      const receipt = await stepJog({
        authorizationId: authorization.id,
        axis: jogAxis,
        distanceMm: direction * jogDistance,
        feedMmPerMin: jogFeed,
      });
      setLastJog(receipt);
    });
  };

  const updateConfirmation = (
    key: keyof OperatorConfirmation,
    checked: boolean,
  ) => {
    setConfirmation((current) => ({ ...current, [key]: checked }));
    setPreparation(undefined);
    setAuthorizationDeadline(undefined);
  };

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

      <details className="test-jog-preflight">
        <summary>
          <span>Test jog preflight</span>
          <small>{authorizationActive ? `${authorizationSeconds} сек` : "Locked"}</small>
        </summary>
        <div className="preflight-content">
          <div className="operator-checklist">
            {checklist.map(([key, label]) => (
              <label key={key}>
                <input
                  checked={confirmation[key]}
                  disabled={!connected || busy}
                  onChange={(event) => updateConfirmation(key, event.target.checked)}
                  type="checkbox"
                />
                <span>{label}</span>
              </label>
            ))}
          </div>
          <button
            className="preflight-action"
            disabled={
              !desktopRuntime || !stableIdle || !confirmationComplete || busy
            }
            onClick={() => void runPreflight()}
            type="button"
          >
            {busy ? "Проверка" : "Проверить заново"}
          </button>
          {preparation && (
            <div
              className={`preflight-result ${authorizationActive ? "is-authorized" : "is-blocked"}`}
              aria-live="polite"
            >
              <strong>
                {authorizationActive
                  ? "Test jog authorization активна"
                  : "Authorization не выдана"}
              </strong>
              <span>
                {authorizationActive
                  ? `Одноразовый lease #${preparation.authorization?.id}`
                  : `${preparation.inspection.readiness.blockerCount} blocker(s)`}
              </span>
            </div>
          )}
          {authorizationActive && (
            <div className="step-jog-control" aria-label="Step jog control">
              <div className="jog-axis-selector" role="group" aria-label="Ось jog">
                {(["x", "y", "z"] as const).map((axis) => (
                  <button
                    aria-pressed={jogAxis === axis}
                    className={jogAxis === axis ? "is-selected" : undefined}
                    disabled={busy}
                    key={axis}
                    onClick={() => setJogAxis(axis)}
                    type="button"
                  >
                    {axis.toUpperCase()}
                  </button>
                ))}
              </div>
              <div className="jog-parameters">
                <label>
                  <span>Шаг, мм</span>
                  <select
                    disabled={busy}
                    onChange={(event) => setJogDistance(Number(event.target.value))}
                    value={jogDistance}
                  >
                    <option value={0.01}>0.01</option>
                    <option value={0.1}>0.10</option>
                    <option value={0.5}>0.50</option>
                    <option value={1}>1.00</option>
                  </select>
                </label>
                <label>
                  <span>Подача</span>
                  <select
                    disabled={busy}
                    onChange={(event) => setJogFeed(Number(event.target.value))}
                    value={jogFeed}
                  >
                    <option value={10}>10 мм/мин</option>
                    <option value={25}>25 мм/мин</option>
                    <option value={50}>50 мм/мин</option>
                    <option value={100}>100 мм/мин</option>
                  </select>
                </label>
              </div>
              <div className="jog-direction-actions">
                <button
                  aria-label={`Jog ${jogAxis.toUpperCase()} в минус`}
                  disabled={busy}
                  onClick={() => executeStepJog(-1)}
                  type="button"
                >
                  − {jogAxis.toUpperCase()}
                </button>
                <button
                  aria-label={`Jog ${jogAxis.toUpperCase()} в плюс`}
                  disabled={busy}
                  onClick={() => executeStepJog(1)}
                  type="button"
                >
                  + {jogAxis.toUpperCase()}
                </button>
              </div>
              <small>Один клик · один lease · максимум 1 мм / 100 мм/мин</small>
            </div>
          )}
          {lastJog && (
            <div className="last-jog" aria-live="polite">
              Принято: <code>{lastJog.command}</code>
            </div>
          )}
        </div>
      </details>
    </section>
  );
}
