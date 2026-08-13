import { ArrowUp, CircleDot, MoveDown, Ruler, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import type { ZProbeGateway } from "../../platform/machine/ZProbeGateway";
import type { HeightmapGateway } from "../../platform/machine/HeightmapGateway";
import type {
  ControllerSnapshot,
  MachineTravel,
  WorkCoordinateSystem,
  ZProbeOutcome,
  ZProbeSettings,
} from "../../shared/machine";
import type { GcodeProgram } from "../../shared/program";
import { defaultZProbeSettings } from "../../shared/profile";
import { HeightmapPanel } from "../heightmap/HeightmapPanel";
import { describeProbeReadinessFailure } from "./probeReadinessModel";
import {
  validateZProbeRunSettings,
  validateZProbeSettings,
  zProbeFinalWorkZ,
} from "./zProbeModel";

interface ZProbeDialogProps {
  readonly activeCoordinateSystem: WorkCoordinateSystem;
  readonly desktopRuntime: boolean;
  readonly disabled?: boolean;
  readonly gateway: ZProbeGateway;
  readonly heightmapGateway: HeightmapGateway;
  readonly machineTravel?: MachineTravel;
  readonly onClose: () => void;
  readonly onAbort: () => Promise<ControllerSnapshot>;
  readonly onError: (error?: string) => void;
  readonly onSaveSettings: (settings: ZProbeSettings) => Promise<void>;
  readonly onSnapshot: (snapshot: ControllerSnapshot) => void;
  readonly onZeroEstablished?: (
    outcome: ZProbeOutcome,
    source: "probe" | "heightmap",
  ) => void;
  readonly onUnlock: () => Promise<ControllerSnapshot>;
  readonly open: boolean;
  readonly profileId?: string;
  readonly program?: GcodeProgram;
  readonly probeInstalled: boolean;
  readonly settings?: ZProbeSettings;
  readonly snapshot: ControllerSnapshot;
}

type ProbeStatus = "idle" | "saving" | "probing" | "complete" | "stopped" | "error";

const numeric = (value: string): number => (value === "" ? 0 : Number(value));

export function ZProbeDialog({
  activeCoordinateSystem,
  desktopRuntime,
  disabled = false,
  gateway,
  heightmapGateway,
  machineTravel,
  onClose,
  onAbort,
  onError,
  onSaveSettings,
  onSnapshot,
  onZeroEstablished,
  onUnlock,
  open,
  profileId,
  program,
  probeInstalled,
  settings,
  snapshot,
}: ZProbeDialogProps) {
  const [draft, setDraft] = useState<ZProbeSettings>(
    settings ?? defaultZProbeSettings(),
  );
  const [confirmed, setConfirmed] = useState(false);
  const [status, setStatus] = useState<ProbeStatus>("idle");
  const [heightmapActive, setHeightmapActive] = useState(false);
  const [localError, setLocalError] = useState<string>();
  const abortRequested = useRef(false);

  useEffect(() => {
    if (!open) return;
    setDraft(settings ?? defaultZProbeSettings());
    setConfirmed(false);
    setStatus("idle");
    setHeightmapActive(false);
    setLocalError(undefined);
    abortRequested.current = false;
  }, [open, profileId]);

  if (!open) return null;

  const validationError = validateZProbeSettings(draft);
  const runValidationError = validateZProbeRunSettings(draft);
  const activeRunValidationError = draft.mode === "workZero"
    ? runValidationError
    : undefined;
  const connected = snapshot.connection === "connected";
  const idle = snapshot.machine.mode === "idle";
  const inputActive = snapshot.machine.pins?.probe ?? false;
  const busy = disabled || status === "saving" || status === "probing" || heightmapActive;
  const canSave = desktopRuntime && !busy && !validationError;
  const canProbe =
    canSave &&
    draft.mode === "workZero" &&
    !activeRunValidationError &&
    connected &&
    idle &&
    !inputActive &&
    confirmed;

  const update = (key: keyof ZProbeSettings, value: string) => {
    setDraft((current) => ({ ...current, [key]: numeric(value) }));
    setStatus("idle");
    setLocalError(undefined);
  };

  const selectMode = (mode: ZProbeSettings["mode"]) => {
    const next = { ...draft, mode };
    setDraft(next);
    setConfirmed(false);
    setLocalError(undefined);
    setStatus("saving");
    onError(undefined);
    void onSaveSettings(next)
      .then(() => setStatus("idle"))
      .catch((error) => {
        const message = describeProbeReadinessFailure(error, "касанию") ?? String(error);
        setStatus("error");
        setLocalError(message);
        onError(message);
      });
  };

  const save = async () => {
    if (validationError) {
      setLocalError(validationError);
      return false;
    }
    setStatus("saving");
    setLocalError(undefined);
    onError(undefined);
    try {
      await onSaveSettings(draft);
      setStatus("idle");
      return true;
    } catch (error) {
      const message = describeProbeReadinessFailure(error, "касанию") ?? String(error);
      setStatus("error");
      setLocalError(message);
      onError(message);
      return false;
    }
  };

  const run = async () => {
    if (!(await save())) return;
    abortRequested.current = false;
    setStatus("probing");
    try {
      const outcome = await gateway.run({ settings: draft, setupConfirmed: true });
      onSnapshot(outcome.snapshot);
      onZeroEstablished?.(outcome, "probe");
      setStatus("complete");
      setConfirmed(false);
    } catch (error) {
      const message = describeProbeReadinessFailure(error, "касанию") ?? String(error);
      if (abortRequested.current && message.includes("interrupted by controller reset")) {
        setStatus("stopped");
        setConfirmed(false);
        setLocalError(undefined);
        onError(undefined);
        return;
      }
      setStatus("error");
      setLocalError(message);
      onError(message);
    }
  };

  const abort = async () => {
    abortRequested.current = true;
    setLocalError(undefined);
    try {
      onSnapshot(await onAbort());
      setStatus("stopped");
      setConfirmed(false);
      onError(undefined);
    } catch (error) {
      abortRequested.current = false;
      const message = describeProbeReadinessFailure(error, "касанию") ?? String(error);
      setLocalError(message);
      onError(message);
    }
  };

  return (
    <div className="machine-dialog-backdrop z-probe-backdrop" role="presentation">
      <section
        aria-labelledby="z-probe-title"
        aria-modal="true"
        className={`machine-dialog z-probe-dialog${draft.mode === "heightmap" ? " is-heightmap" : ""}`}
        role="dialog"
      >
        <header>
          <div>
            <span>Вход A5 · контактный щуп</span>
            <h2 id="z-probe-title">Измерение поверхности</h2>
          </div>
          <button aria-label="Закрыть" disabled={draft.mode === "workZero" && status === "probing"} onClick={onClose} title="Закрыть" type="button">
            <X aria-hidden="true" size={16} />
          </button>
        </header>

        <div className="z-probe-body">
          <div className={`z-probe-live is-${inputActive ? "triggered" : connected ? "open" : "unavailable"}`}>
            <CircleDot aria-hidden="true" size={18} />
            <span>
              <strong>{inputActive ? "Контакт замкнут" : connected ? "Контакт разомкнут" : "Нет данных входа P"}</strong>
              <small>{inputActive ? "Разомкните щуп перед запуском" : "При касании этот статус изменится"}</small>
            </span>
          </div>

          <div className="probe-workflow-selector" role="tablist" aria-label="Режим щупа">
            {(["off", "workZero", "heightmap"] as const).map((mode) => (
              <button
                aria-selected={draft.mode === mode}
                disabled={busy}
                key={mode}
                onClick={() => selectMode(mode)}
                role="tab"
                type="button"
              >
                <strong>{{ off: "Измерения выкл.", workZero: "Ноль Z", heightmap: "Карта поверхности" }[mode]}</strong>
                <small>{{ off: "Только индикатор A5", workZero: "Одно касание", heightmap: "Сетка касаний" }[mode]}</small>
              </button>
            ))}
          </div>

          {draft.mode === "heightmap" && (
            <HeightmapPanel
              activeCoordinateSystem={activeCoordinateSystem}
              key={profileId ?? "unbound"}
              desktopRuntime={desktopRuntime}
              disabled={disabled}
              gateway={heightmapGateway}
              zProbeGateway={gateway}
              machineProfileId={profileId}
              machineTravel={machineTravel}
              onError={onError}
              onActivityChange={setHeightmapActive}
              onSnapshot={onSnapshot}
              onZeroEstablished={onZeroEstablished}
              onSaveMode={() => onSaveSettings(draft)}
              onUnlock={onUnlock}
              program={program}
              snapshot={snapshot}
            />
          )}

          {draft.mode === "off" && (
            <div className="probe-mode-empty">
              <CircleDot aria-hidden="true" size={18} />
              <span><strong>Автоматические измерения выключены</strong><small>Индикатор A5 продолжает показывать электрический контакт. Сохранённая карта не удаляется и включается отдельно в панели запуска задания.</small></span>
            </div>
          )}

          {draft.mode === "workZero" && <>
          <p className="z-probe-workflow-note"><strong>Для ровной поверхности.</strong> Положите съёмную контактную пластину на заготовку. После касания Millo учтёт её толщину, установит Z0 на самой поверхности и поднимет фрезу. Применение старой карты будет выключено.</p>

          <div className="z-probe-fields">
            <label>
              <span><Ruler aria-hidden="true" size={15} /> Толщина пластины</span>
              <span className="z-probe-input">
                <input
                  max="100"
                  min="0.01"
                  onChange={(event) => update("plateThicknessMm", event.target.value)}
                  placeholder="Например, 19.10"
                  step="0.01"
                  type="number"
                  value={draft.plateThicknessMm || ""}
                />
                <code>mm</code>
              </span>
              <small>Измерьте пластину штангенциркулем вместе с рабочей контактной поверхностью.</small>
            </label>
            <label>
              <span><MoveDown aria-hidden="true" size={15} /> Искать вниз не дальше</span>
              <span className="z-probe-input">
                <input max="100" min="0.1" onChange={(event) => update("maxTravelMm", event.target.value)} step="0.1" type="number" value={draft.maxTravelMm} />
                <code>mm</code>
              </span>
            </label>
          </div>

          <details className="z-probe-advanced">
            <summary>Подача и отвод</summary>
            <div>
              <label><span>Подача касания</span><span className="z-probe-input"><input max="500" min="1" onChange={(event) => update("probeFeedMmPerMin", event.target.value)} step="1" type="number" value={draft.probeFeedMmPerMin} /><code>mm/min</code></span></label>
              <label><span>Поднять после касания</span><span className="z-probe-input"><input max="100" min="0.1" onChange={(event) => update("retractMm", event.target.value)} step="0.1" type="number" value={draft.retractMm} /><code>mm</code></span></label>
              <label><span>Подача отвода</span><span className="z-probe-input"><input max="2000" min="1" onChange={(event) => update("retractFeedMmPerMin", event.target.value)} step="10" type="number" value={draft.retractFeedMmPerMin} /><code>mm/min</code></span></label>
            </div>
          </details>

          <div className="z-probe-result">
            <ArrowUp aria-hidden="true" size={17} />
            <span>
              <strong>После касания: Z = {draft.plateThicknessMm > 0 ? draft.plateThicknessMm.toFixed(3) : "--"} mm</strong>
              <small>После отвода: Z = {draft.plateThicknessMm > 0 ? zProbeFinalWorkZ(draft).toFixed(3) : "--"} mm</small>
            </span>
          </div>

          {!probeInstalled && <p className="z-probe-profile-note">При сохранении щуп будет отмечен установленным в профиле станка.</p>}
          <label className="z-probe-confirmation">
            <input checked={confirmed} disabled={busy} onChange={(event) => setConfirmed(event.target.checked)} type="checkbox" />
            <span>Пластина лежит на заготовке, зажим подключён к фрезе, шпиндель остановлен</span>
          </label>

          <div className="z-probe-message" aria-live="polite">
            {localError ?? validationError ?? activeRunValidationError ??
              (status === "complete" ? "Поверхность найдена, рабочая Z установлена и фреза поднята." :
                status === "stopped" ? "Касание остановлено. Перед новой попыткой дождитесь Idle." :
                inputActive ? "Вход P уже активен: запуск заблокирован." :
                  !connected ? "Подключитесь к контроллеру, чтобы выполнить касание." :
                    !idle ? `Дождитесь Idle. Сейчас: ${snapshot.machine.reportedMode}.` : "Готово к проверке.")}
          </div>
          </>}
        </div>

        {draft.mode === "workZero" && <footer className="z-probe-actions">
          {status === "probing" ? (
            <button className="is-danger" onClick={() => void abort()} type="button">Остановить касание</button>
          ) : (
            <button disabled={!canSave} onClick={() => void save()} type="button">Сохранить параметры</button>
          )}
          <button className="is-primary" disabled={!canProbe} onClick={() => void run()} type="button">
            <CircleDot aria-hidden="true" size={16} />
            {status === "probing" ? "Ищу поверхность…" : "Найти поверхность и установить Z"}
          </button>
        </footer>}
      </section>
    </div>
  );
}
