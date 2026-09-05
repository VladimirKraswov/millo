import {
  ArrowDown,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  ChevronRight,
  Gauge,
  Keyboard,
  Move3d,
  RotateCw,
  Settings2,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { acceptsKeyboardJogEvent } from "./keyboardJogModel";

import type { MachineCommandGateway } from "../../platform/machine/MachineCommandGateway";
import type {
  ControllerSnapshot,
  ContinuousJogReceipt,
  HardwareInspection,
  JogAxis,
  OperatorConfirmation,
  RotaryAxisProfile,
  StepJogReceipt,
} from "../../shared/machine";
import {
  isControllerConnected,
  isControllerStableIdle,
} from "../../shared/controllerReadiness";
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
  rotaryAxis?: RotaryAxisProfile;
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
  rotaryAxis,
}: JogPadProps) {
  const interactor = useMemo(() => new JogPadInteractor(gateway), [gateway]);
  const [jogArmed, setJogArmed] = useState(false);
  const confirmation: OperatorConfirmation = jogOperatorConfirmation(jogArmed);
  const [motionProfile, setMotionProfile] =
    useState<JogMotionProfileId | "custom">("position");
  const [distanceDraft, setDistanceDraft] = useState("1");
  const [feedMmPerMin, setFeedMmPerMin] = useState(300);
  const [rotaryStepDegrees, setRotaryStepDegrees] = useState(5);
  const [rotaryFeedDegreesPerMin, setRotaryFeedDegreesPerMin] = useState(
    () => Math.min(360, rotaryAxis?.maxFeedDegreesPerMin ?? 360),
  );
  const [busy, setBusy] = useState(false);
  const [jogMode, setJogMode] = useState<"step" | "continuous">("step");
  const [keyboardJogEnabled, setKeyboardJogEnabled] = useState(false);
  const [continuousBusy, setContinuousBusy] = useState(false);
  const [activeContinuousControl, setActiveContinuousControl] = useState<string>();
  const [lastContinuousJog, setLastContinuousJog] = useState<ContinuousJogReceipt>();
  const activeKeyboardCode = useRef<string | undefined>(undefined);
  const onErrorRef = useRef(onError);
  const startContinuousRef = useRef<(
    axis: JogAxis,
    direction: JogDirection,
    controlId: string,
    feed?: number,
  ) => Promise<void>>(async () => undefined);
  const stopContinuousRef = useRef<() => Promise<void>>(async () => undefined);
  const canMoveRef = useRef(false);
  const linearFeedRef = useRef(feedMmPerMin);
  const rotaryFeedRef = useRef(rotaryFeedDegreesPerMin);
  const rotaryEnabledRef = useRef(rotaryAxis !== undefined);
  onErrorRef.current = onError;
  const [lastJog, setLastJog] = useState<StepJogReceipt>();
  const [blockedCount, setBlockedCount] = useState<number>();
  const profiles = useMemo(
    () => jogMotionProfiles(maxDistanceMm, maxFeedMmPerMin),
    [maxDistanceMm, maxFeedMmPerMin],
  );

  const connected = isControllerConnected(snapshot);
  const stableIdle = isControllerStableIdle(snapshot);
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
    !busy &&
    !continuousBusy &&
    activeContinuousControl === undefined;
  canMoveRef.current = canMove;
  linearFeedRef.current = feedMmPerMin;
  rotaryFeedRef.current = rotaryFeedDegreesPerMin;
  rotaryEnabledRef.current = rotaryAxis !== undefined;

  useEffect(() => {
    if (!connected) {
      setJogArmed(false);
      setLastJog(undefined);
      setBlockedCount(undefined);
      setKeyboardJogEnabled(false);
      setActiveContinuousControl(undefined);
      void interactor.stopContinuous();
    }
  }, [connected, interactor]);

  useEffect(() => {
    if (disabled) {
      setActiveContinuousControl(undefined);
      void interactor.stopContinuous();
    }
  }, [disabled, interactor]);

  useEffect(() => {
    const stop = () => {
      activeKeyboardCode.current = undefined;
      setActiveContinuousControl(undefined);
      void interactor.stopContinuous().catch((error) => onErrorRef.current(String(error)));
    };
    const onVisibilityChange = () => {
      if (document.visibilityState !== "visible") stop();
    };
    window.addEventListener("pointerup", stop, true);
    window.addEventListener("pointercancel", stop, true);
    window.addEventListener("blur", stop);
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => {
      window.removeEventListener("pointerup", stop, true);
      window.removeEventListener("pointercancel", stop, true);
      window.removeEventListener("blur", stop);
      document.removeEventListener("visibilitychange", onVisibilityChange);
      stop();
    };
  }, [interactor]);

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

  const move = async (
    axis: JogAxis,
    direction: JogDirection,
    distance = distanceMm,
    feed = feedMmPerMin,
  ) => {
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
        distance,
        feed,
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

  const startContinuous = async (
    axis: JogAxis,
    direction: JogDirection,
    controlId: string,
    feed = feedMmPerMin,
  ) => {
    if (!canMove) return;
    setContinuousBusy(true);
    setActiveContinuousControl(controlId);
    setLastJog(undefined);
    setLastContinuousJog(undefined);
    setBlockedCount(undefined);
    onError(undefined);
    try {
      const receipt = await interactor.startContinuous(
        confirmation,
        axis,
        direction,
        feed,
      );
      setLastContinuousJog(receipt);
    } catch (error) {
      setActiveContinuousControl(undefined);
      onError(String(error));
    } finally {
      setContinuousBusy(false);
    }
  };

  const stopContinuous = async () => {
    setActiveContinuousControl(undefined);
    try {
      await interactor.stopContinuous();
    } catch (error) {
      onError(String(error));
    }
  };
  startContinuousRef.current = startContinuous;
  stopContinuousRef.current = stopContinuous;

  const moveButton = (
    axis: JogAxis,
    direction: JogDirection,
    icon: ReactNode,
    options?: { distance: number; feed: number; unit: string },
  ) => {
    const signedAxis = `${axis.toUpperCase()}${direction > 0 ? "+" : "−"}`;
    const active = activeContinuousControl === signedAxis;
    const buttonDistance = options?.distance ?? distanceMm;
    const buttonFeed = options?.feed ?? feedMmPerMin;
    const unit = options?.unit ?? "мм";
    const buttonValuesValid = options
      ? Number.isFinite(buttonDistance) &&
        buttonDistance >= MIN_JOG_DISTANCE_MM &&
        buttonDistance <= (rotaryAxis?.maxJogDegrees ?? 0) &&
        Number.isFinite(buttonFeed) &&
        buttonFeed >= 1 &&
        buttonFeed <= (rotaryAxis?.maxFeedDegreesPerMin ?? 0)
      : motionValuesValid;
    return (
      <button
        aria-label={`Jog ${signedAxis}`}
        aria-pressed={active}
        className={active ? "is-active" : undefined}
        disabled={(!canMove || !buttonValuesValid) && !active}
        onClick={() => {
          if (jogMode === "step") void move(axis, direction, buttonDistance, buttonFeed);
        }}
        onContextMenu={(event) => event.preventDefault()}
        onPointerDown={(event) => {
          if (jogMode !== "continuous" || event.button !== 0) return;
          event.preventDefault();
          event.currentTarget.setPointerCapture(event.pointerId);
          void startContinuous(axis, direction, signedAxis, buttonFeed);
        }}
        onPointerUp={() => {
          if (jogMode === "continuous") void stopContinuous();
        }}
        title={
          jogMode === "step"
            ? `Jog ${signedAxis}: ${buttonDistance.toFixed(2)} ${unit} · F${buttonFeed}`
            : `Удерживайте для движения ${signedAxis} · F${buttonFeed}`
        }
        type="button"
      >
        <span aria-hidden="true">{icon}</span>
        <small>{signedAxis}</small>
      </button>
    );
  };

  useEffect(() => {
    if (!keyboardJogEnabled) return;
    const bindings: Record<string, [JogAxis, JogDirection]> = {
      ArrowLeft: ["x", -1],
      ArrowRight: ["x", 1],
      ArrowUp: ["y", 1],
      ArrowDown: ["y", -1],
      PageUp: ["z", 1],
      PageDown: ["z", -1],
      BracketLeft: ["a", -1],
      BracketRight: ["a", 1],
    };
    const onKeyDown = (event: KeyboardEvent) => {
      const binding = bindings[event.code];
      if (!binding || !acceptsKeyboardJogEvent(event) || activeKeyboardCode.current) return;
      if (!canMoveRef.current) return;
      event.preventDefault();
      const [axis, direction] = binding;
      if (axis === "a" && !rotaryEnabledRef.current) return;
      activeKeyboardCode.current = event.code;
      void startContinuousRef.current(
        axis,
        direction,
        `key:${event.code}`,
        axis === "a" ? rotaryFeedRef.current : linearFeedRef.current,
      );
    };
    const onKeyUp = (event: KeyboardEvent) => {
      if (event.code !== activeKeyboardCode.current) return;
      event.preventDefault();
      activeKeyboardCode.current = undefined;
      void stopContinuousRef.current();
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
      activeKeyboardCode.current = undefined;
      void interactor.stopContinuous();
    };
  }, [interactor, keyboardJogEnabled]);

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
          <span>Управление движением</span>
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

      <div className="jog-input-modes">
        <div aria-label="Режим jog" className="jog-mode-switch" role="group">
          <button
            aria-pressed={jogMode === "step"}
            className={jogMode === "step" ? "is-selected" : undefined}
            disabled={continuousBusy}
            onClick={() => setJogMode("step")}
            type="button"
          >
            Шаг
          </button>
          <button
            aria-pressed={jogMode === "continuous"}
            className={jogMode === "continuous" ? "is-selected" : undefined}
            disabled={continuousBusy}
            onClick={() => setJogMode("continuous")}
            type="button"
          >
            Удержание
          </button>
        </div>
        <label className="keyboard-jog-toggle">
          <input
            checked={keyboardJogEnabled}
            disabled={!jogArmed || disabled}
            onChange={(event) => setKeyboardJogEnabled(event.target.checked)}
            type="checkbox"
          />
          <Keyboard aria-hidden="true" size={14} />
          <span>Клавиатура</span>
        </label>
      </div>

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

      <div className={`jog-pad-controls${rotaryAxis ? " has-rotary" : ""}`}>
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
        {rotaryAxis && (
          <div className="jog-pad-a" aria-label="Jog A">
            <span>A</span>
            {moveButton("a", 1, <RotateCw size={19} />, {
              distance: rotaryStepDegrees,
              feed: rotaryFeedDegreesPerMin,
              unit: "°",
            })}
            {moveButton("a", -1, <RotateCw className="is-reversed" size={19} />, {
              distance: rotaryStepDegrees,
              feed: rotaryFeedDegreesPerMin,
              unit: "°",
            })}
            <label>
              <input
                aria-label="Шаг оси A"
                max={rotaryAxis.maxJogDegrees}
                min="0.01"
                onChange={(event) => setRotaryStepDegrees(Number(event.target.value))}
                step="0.1"
                type="number"
                value={rotaryStepDegrees}
              />
              <small>°</small>
            </label>
            <label>
              <input
                aria-label="Скорость оси A"
                max={rotaryAxis.maxFeedDegreesPerMin}
                min="1"
                onChange={(event) => setRotaryFeedDegreesPerMin(Number(event.target.value))}
                step="10"
                type="number"
                value={rotaryFeedDegreesPerMin}
              />
              <small>°/min</small>
            </label>
          </div>
        )}
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
        {!busy && continuousBusy && <span>Движение с удержанием запускается...</span>}
        {!busy && !continuousBusy && activeContinuousControl && (
          <span>Движение активно: отпустите кнопку для остановки</span>
        )}
        {!busy && !continuousBusy && !activeContinuousControl && lastContinuousJog && (
          <span>
            Удержание: {lastContinuousJog.boundedDistance.toFixed(2)} · {lastContinuousJog.boundarySource === "machineCoordinates" ? "MPos" : "профиль"}
          </span>
        )}
        {!busy && lastJog && (
          <span>
            Принято: <code>{lastJog.command}</code>
          </span>
        )}
        {!busy && blockedCount !== undefined && (
          <span className="is-blocked">Движение заблокировано: {blockedCount}</span>
        )}
        {!busy && !continuousBusy && !activeContinuousControl && !lastJog && !lastContinuousJog && blockedCount === undefined && (
          <span>{jogMode === "step" ? "Нажатие выполняет выбранный шаг" : "Движение идёт только пока кнопка удерживается"}</span>
        )}
      </div>
    </section>
  );
}
