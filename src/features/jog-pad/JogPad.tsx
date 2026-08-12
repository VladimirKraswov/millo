import {
  ArrowDown,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  ChevronRight,
  Gauge,
  Move3d,
  Settings2,
} from "lucide-react";
import { useEffect, useMemo, useState, type ReactNode } from "react";

import type { MachineCommandGateway } from "../../platform/machine/MachineCommandGateway";
import type {
  ControllerSnapshot,
  HardwareInspection,
  JogAxis,
  OperatorConfirmation,
  StepJogReceipt,
} from "../../shared/machine";
import {
  MAX_JOG_DISTANCE_MM,
  MAX_JOG_FEED_MM_PER_MIN,
  MIN_JOG_DISTANCE_MM,
  MIN_JOG_FEED_MM_PER_MIN,
  JogPadInteractor,
  jogMotionProfiles,
  jogOperatorConfirmation,
  type JogDirection,
  type JogMotionProfileId,
} from "./JogPadInteractor";

interface JogPadProps {
  snapshot: ControllerSnapshot;
  desktopRuntime: boolean;
  disabled?: boolean;
  gateway: MachineCommandGateway;
  onInspection: (inspection?: HardwareInspection) => void;
  onError: (error?: string) => void;
  onOpenMotionSettings: () => void;
  maxDistanceMm: number;
  maxFeedMmPerMin: number;
}

export function JogPad({
  snapshot,
  desktopRuntime,
  disabled = false,
  gateway,
  onInspection,
  onError,
  onOpenMotionSettings,
  maxDistanceMm,
  maxFeedMmPerMin,
}: JogPadProps) {
  const interactor = useMemo(() => new JogPadInteractor(gateway), [gateway]);
  const [jogArmed, setJogArmed] = useState(false);
  const confirmation: OperatorConfirmation = jogOperatorConfirmation(jogArmed);
  const [motionProfile, setMotionProfile] =
    useState<JogMotionProfileId | "custom">("position");
  const [distanceDraft, setDistanceDraft] = useState("1");
  const [feedMmPerMin, setFeedMmPerMin] = useState(300);
  const [busy, setBusy] = useState(false);
  const [lastJog, setLastJog] = useState<StepJogReceipt>();
  const [blockedCount, setBlockedCount] = useState<number>();
  const profiles = useMemo(
    () => jogMotionProfiles(maxDistanceMm, maxFeedMmPerMin),
    [maxDistanceMm, maxFeedMmPerMin],
  );

  const connected = snapshot.connection === "connected";
  const stableIdle =
    connected &&
    snapshot.machine.mode === "idle" &&
    snapshot.alarm === undefined &&
    snapshot.resetNotice === undefined;
  const distanceMm = Number(distanceDraft);
  const motionValuesValid =
    Number.isFinite(distanceMm) &&
    distanceMm >= MIN_JOG_DISTANCE_MM &&
    distanceMm <= Math.min(maxDistanceMm, MAX_JOG_DISTANCE_MM) &&
    Number.isFinite(feedMmPerMin) &&
    feedMmPerMin >= MIN_JOG_FEED_MM_PER_MIN &&
    feedMmPerMin <= Math.min(maxFeedMmPerMin, MAX_JOG_FEED_MM_PER_MIN);
  const canMove =
    desktopRuntime &&
    stableIdle &&
    jogArmed &&
    motionValuesValid &&
    !disabled &&
    !busy;

  useEffect(() => {
    if (!connected) {
      setJogArmed(false);
      setLastJog(undefined);
      setBlockedCount(undefined);
    }
  }, [connected]);

  useEffect(() => {
    const selectedProfile = profiles.find((profile) => profile.id === motionProfile);
    if (selectedProfile) {
      setDistanceDraft(String(selectedProfile.distanceMm));
      setFeedMmPerMin(selectedProfile.feedMmPerMin);
      return;
    }

    setDistanceDraft((current) => {
      const value = Number(current);
      return String(
        Math.min(
          Math.max(Number.isFinite(value) ? value : MIN_JOG_DISTANCE_MM, MIN_JOG_DISTANCE_MM),
          maxDistanceMm,
        ),
      );
    });
    setFeedMmPerMin((current) =>
      Math.min(Math.max(current, MIN_JOG_FEED_MM_PER_MIN), maxFeedMmPerMin),
    );
  }, [maxDistanceMm, maxFeedMmPerMin, motionProfile, profiles]);

  const move = async (axis: JogAxis, direction: JogDirection) => {
    if (!canMove) return;

    setBusy(true);
    setLastJog(undefined);
    setBlockedCount(undefined);
    onError(undefined);
    try {
      const outcome = await interactor.move(
        confirmation,
        axis,
        direction,
        distanceMm,
        feedMmPerMin,
      );
      onInspection(outcome.inspection);
      setLastJog(outcome.receipt);
      setBlockedCount(
        outcome.receipt
          ? undefined
          : outcome.inspection.readiness.blockerCount,
      );
    } catch (error) {
      onError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const moveButton = (
    axis: JogAxis,
    direction: JogDirection,
    icon: ReactNode,
  ) => {
    const signedAxis = `${axis.toUpperCase()}${direction > 0 ? "+" : "−"}`;
    return (
      <button
        aria-label={`Jog ${signedAxis}`}
        disabled={!canMove}
        onClick={() => void move(axis, direction)}
        title={`Jog ${signedAxis}: ${distanceMm.toFixed(2)} мм · F${feedMmPerMin}`}
        type="button"
      >
        <span aria-hidden="true">{icon}</span>
        <small>{signedAxis}</small>
      </button>
    );
  };

  const selectProfile = (profile: (typeof profiles)[number]) => {
    setMotionProfile(profile.id);
    setDistanceDraft(String(profile.distanceMm));
    setFeedMmPerMin(profile.feedMmPerMin);
    setLastJog(undefined);
    setBlockedCount(undefined);
  };

  const displayedDistance = motionValuesValid
    ? distanceMm < 1
      ? distanceMm.toFixed(2)
      : distanceMm.toFixed(distanceMm % 1 === 0 ? 0 : 1)
    : "--";

  return (
    <section className="jog-pad" aria-labelledby="jog-pad-title">
      <div className="jog-pad-heading">
        <div>
          <span>Motion deck</span>
          <strong id="jog-pad-title">
            <Move3d aria-hidden="true" size={14} />
            Jog
          </strong>
        </div>
        <code>F{feedMmPerMin}</code>
      </div>

      <label className="jog-arm-control">
        <input
          checked={jogArmed}
          disabled={!connected || disabled || busy}
          onChange={(event) => {
            setJogArmed(event.target.checked);
            setLastJog(undefined);
            setBlockedCount(undefined);
          }}
          type="checkbox"
        />
        <span>
          <strong>Разрешить jog</strong>
          <small>Шпиндель выключен, зона свободна, питание доступно</small>
        </span>
      </label>

      <div className="jog-motion-profiles" role="group" aria-label="Профиль jog">
        {profiles.map((profile) => (
          <button
            aria-pressed={motionProfile === profile.id}
            className={motionProfile === profile.id ? "is-selected" : undefined}
            disabled={disabled || busy}
            key={profile.id}
            onClick={() => selectProfile(profile)}
            type="button"
          >
            <strong>{profile.label}</strong>
            <small>{profile.distanceMm} mm · F{profile.feedMmPerMin}</small>
          </button>
        ))}
      </div>

      <div className="jog-parameters">
        <label>
          <span>Перемещение</span>
          <span>
            <input
              aria-label="Длина jog"
              disabled={disabled || busy}
              max={maxDistanceMm}
              min={MIN_JOG_DISTANCE_MM}
              onChange={(event) => {
                setMotionProfile("custom");
                setDistanceDraft(event.target.value);
              }}
              step="0.01"
              type="number"
              value={distanceDraft}
            />
            <small>mm</small>
          </span>
        </label>
        <label>
          <span>Подача</span>
          <span>
            <input
              aria-label="Подача jog"
              disabled={disabled || busy}
              max={maxFeedMmPerMin}
              min={MIN_JOG_FEED_MM_PER_MIN}
              onChange={(event) => {
                setMotionProfile("custom");
                setFeedMmPerMin(Number(event.target.value));
              }}
              step="10"
              type="number"
              value={feedMmPerMin}
            />
            <small>mm/min</small>
          </span>
        </label>
      </div>

      <label className="jog-feed-rail">
        <Gauge aria-hidden="true" size={14} />
        <input
          aria-label="Скорость jog"
          disabled={disabled || busy}
          max={maxFeedMmPerMin}
          min={MIN_JOG_FEED_MM_PER_MIN}
          onChange={(event) => {
            setMotionProfile("custom");
            setFeedMmPerMin(Number(event.target.value));
          }}
          step="10"
          type="range"
          value={feedMmPerMin}
        />
        <code>{Math.round((feedMmPerMin / maxFeedMmPerMin) * 100)}%</code>
      </label>

      <div className="jog-pad-controls">
        <div className="jog-pad-xy" aria-label="Jog X и Y">
          <i />
          {moveButton("y", 1, <ArrowUp size={20} />)}
          <i />
          {moveButton("x", -1, <ArrowLeft size={20} />)}
          <span className="jog-pad-center" aria-hidden="true">
            <strong>{displayedDistance}</strong>
            <small>mm</small>
          </span>
          {moveButton("x", 1, <ArrowRight size={20} />)}
          <i />
          {moveButton("y", -1, <ArrowDown size={20} />)}
          <i />
        </div>
        <div className="jog-pad-z" aria-label="Jog Z">
          <span>Z</span>
          {moveButton("z", 1, <ArrowUp size={20} />)}
          {moveButton("z", -1, <ArrowDown size={20} />)}
        </div>
      </div>

      <button
        className="jog-acceleration-link"
        disabled={!connected || busy}
        onClick={onOpenMotionSettings}
        type="button"
      >
        <Settings2 aria-hidden="true" size={14} />
        <span>
          <strong>Ускорение осей</strong>
          <small>Настройки GRBL $120 · $121 · $122</small>
        </span>
        <ChevronRight aria-hidden="true" size={14} />
      </button>

      <div className="jog-pad-status" aria-live="polite">
        {busy && <span>Свежая проверка станка...</span>}
        {!busy && lastJog && (
          <span>
            Принято: <code>{lastJog.command}</code>
          </span>
        )}
        {!busy && blockedCount !== undefined && (
          <span className="is-blocked">Движение заблокировано: {blockedCount}</span>
        )}
        {!busy && !lastJog && blockedCount === undefined && (
          <span>Каждое нажатие повторяет status и Inspector</span>
        )}
      </div>
    </section>
  );
}
