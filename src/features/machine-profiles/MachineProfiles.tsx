import { Gauge, Plus, Router, ScanLine, Settings, X } from "lucide-react";
import { useEffect, useState } from "react";

import type {
  MachineProfile,
  MachineProfileDraft,
  MachineProfileState,
} from "../../shared/profile";
import { selectedMachineProfile } from "../../shared/profile";
import {
  emptyMachineProfileDraft,
  formatMachineTravel,
  validateMachineProfileDraft,
} from "./machineProfileModel";

interface MachineProfilesProps {
  state: MachineProfileState;
  locked: boolean;
  busy: boolean;
  canDetect: boolean;
  onCreate: (draft: MachineProfileDraft) => Promise<void>;
  onDetect: () => Promise<MachineProfileDraft>;
  onEdit: () => void;
  onOnboardingDismiss: () => void;
  onSelect: (profileId: string) => Promise<void>;
  onboardingDraft?: MachineProfileDraft;
}

const copyDraft = (draft: MachineProfileDraft): MachineProfileDraft => ({
  ...draft,
  travelMm: { ...draft.travelMm },
  probeSettings: { ...draft.probeSettings },
  connection: draft.connection
    ? {
        ...draft.connection,
        fingerprint: draft.connection.fingerprint
          ? { ...draft.connection.fingerprint }
          : undefined,
      }
    : undefined,
  detectedController: draft.detectedController
    ? { ...draft.detectedController }
    : undefined,
});

export function MachineProfiles({
  state,
  locked,
  busy,
  canDetect,
  onCreate,
  onDetect,
  onEdit,
  onOnboardingDismiss,
  onSelect,
  onboardingDraft,
}: MachineProfilesProps) {
  const selected = selectedMachineProfile(state);
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState<MachineProfileDraft>(
    emptyMachineProfileDraft,
  );
  const [dialogBusy, setDialogBusy] = useState(false);
  const [formError, setFormError] = useState<string>();

  useEffect(() => {
    if (!onboardingDraft) return;
    setDraft(copyDraft(onboardingDraft));
    setFormError(undefined);
    setOpen(true);
  }, [onboardingDraft]);

  useEffect(() => {
    if (!open) {
      setDraft(emptyMachineProfileDraft());
      setFormError(undefined);
    }
  }, [open]);

  const updateFlag = (
    key:
      | "homingInstalled"
      | "limitSwitchesInstalled"
      | "probeInstalled"
      | "emergencyStopInstalled",
    value: boolean,
  ) => setDraft((current) => ({ ...current, [key]: value }));

  const detect = async () => {
    setDialogBusy(true);
    setFormError(undefined);
    try {
      setDraft(copyDraft(await onDetect()));
    } catch (error) {
      setFormError(String(error));
    } finally {
      setDialogBusy(false);
    }
  };

  const submit = async () => {
    const error = validateMachineProfileDraft(draft);
    if (error) {
      setFormError(error);
      return;
    }
    setDialogBusy(true);
    setFormError(undefined);
    try {
      await onCreate(copyDraft(draft));
      setOpen(false);
      onOnboardingDismiss();
    } catch (submitError) {
      setFormError(String(submitError));
    } finally {
      setDialogBusy(false);
    }
  };

  return (
    <>
      <section className="machine-switcher" aria-label="Активный станок">
        <Router aria-hidden="true" size={18} />
        <label>
          <span>Станок</span>
          <select
            aria-label="Выбрать станок"
            disabled={locked || busy || state.profiles.length === 0}
            onChange={(event) => void onSelect(event.target.value)}
            value={selected?.id ?? ""}
            title={locked
              ? "Профиль определяется подключённым контроллером. Для ручной смены сначала отключите станок."
              : "Выбрать профиль станка"}
          >
            {!selected && <option value="">Не выбран</option>}
            {state.profiles.map((profile) => (
              <option key={profile.id} value={profile.id}>
                {profile.name}
              </option>
            ))}
          </select>
        </label>
        <small title={locked
          ? "Профиль привязан к текущему подключению"
          : undefined}
        >
          {selected
            ? `${locked ? "Привязан · " : ""}${formatMachineTravel(selected)}`
            : "Нужен профиль"}
        </small>
        <button
          aria-label="Настройки станка"
          disabled={busy || !selected || Boolean(onboardingDraft)}
          onClick={onEdit}
          title="Настройки станка"
          type="button"
        >
          <Settings aria-hidden="true" size={16} />
        </button>
        <button
          aria-label="Добавить станок"
          disabled={locked || busy}
          onClick={() => setOpen(true)}
          title="Добавить станок"
          type="button"
        >
          <Plus aria-hidden="true" size={17} />
        </button>
      </section>

      {open && (
        <div className="machine-dialog-backdrop" role="presentation">
          <section
            aria-labelledby="machine-dialog-title"
            aria-modal="true"
            className="machine-dialog"
            role="dialog"
          >
            <header>
              <div>
                <span>Machine profile</span>
                <h2 id="machine-dialog-title">
                  {onboardingDraft ? "Подключён новый станок" : "Новый станок"}
                </h2>
              </div>
              <button
                aria-label="Закрыть"
                disabled={dialogBusy}
                onClick={() => {
                  setOpen(false);
                  onOnboardingDismiss();
                }}
                title="Закрыть"
                type="button"
              >
                <X aria-hidden="true" size={18} />
              </button>
            </header>

            <div className="machine-dialog-body">
              {!onboardingDraft && <button
                className="detect-machine-action"
                disabled={!canDetect || dialogBusy}
                onClick={() => void detect()}
                type="button"
              >
                <ScanLine aria-hidden="true" size={17} />
                {dialogBusy ? "Чтение контроллера" : "Считать из выбранного GRBL"}
              </button>}

              {onboardingDraft && (
                <div className="onboarding-machine-note">
                  <ScanLine aria-hidden="true" size={17} />
                  <span>Параметры считаны из GRBL. Задайте локальное имя и фактическое оснащение.</span>
                </div>
              )}

              <label className="machine-name-field">
                <span>Название</span>
                <input
                  autoFocus
                  maxLength={80}
                  onChange={(event) =>
                    setDraft((current) => ({
                      ...current,
                      name: event.target.value,
                    }))
                  }
                  placeholder="Например, настольный CNC"
                  value={draft.name}
                />
              </label>

              <fieldset className="travel-fields">
                <legend>
                  <Gauge aria-hidden="true" size={15} />
                  Ход осей, mm
                </legend>
                {(["x", "y", "z"] as const).map((axis) => (
                  <label key={axis}>
                    <span>{axis.toUpperCase()}</span>
                    <input
                      inputMode="decimal"
                      min="0.001"
                      readOnly={Boolean(onboardingDraft)}
                      onChange={(event) =>
                        setDraft((current) => ({
                          ...current,
                          travelMm: {
                            ...current.travelMm,
                            [axis]: Number(event.target.value),
                          },
                        }))
                      }
                      step="0.001"
                      type="number"
                      value={draft.travelMm[axis] || ""}
                    />
                  </label>
                ))}
              </fieldset>

              <label className="machine-jog-limit-field">
                <span>
                  <strong>Максимальный jog</strong>
                  <small>Предел одного ручного перемещения для этой машины</small>
                </span>
                <span>
                  <input
                    max={Math.max(draft.travelMm.x, draft.travelMm.y, draft.travelMm.z) || undefined}
                    min="0.01"
                    onChange={(event) =>
                      setDraft((current) => ({
                        ...current,
                        maxJogDistanceMm: Number(event.target.value),
                      }))
                    }
                    step="0.01"
                    type="number"
                    value={draft.maxJogDistanceMm}
                  />
                  <code>mm</code>
                </span>
              </label>

              <fieldset className="spindle-mode">
                <legend>Шпиндель</legend>
                <label>
                  <input
                    checked={draft.spindleControl === "manual"}
                    name="spindle-control"
                    onChange={() =>
                      setDraft((current) => ({
                        ...current,
                        spindleControl: "manual",
                      }))
                    }
                    type="radio"
                  />
                  Вручную
                </label>
                <label>
                  <input
                    checked={draft.spindleControl === "controller"}
                    name="spindle-control"
                    onChange={() =>
                      setDraft((current) => ({
                        ...current,
                        spindleControl: "controller",
                      }))
                    }
                    type="radio"
                  />
                  Контроллером
                </label>
              </fieldset>

              <fieldset className="hardware-flags">
                <legend>Оборудование</legend>
                {[
                  ["limitSwitchesInstalled", "Концевики"],
                  ["homingInstalled", "Homing"],
                  ["probeInstalled", "Датчик касания"],
                  ["emergencyStopInstalled", "Аварийная кнопка"],
                ].map(([key, label]) => (
                  <label key={key}>
                    <input
                      checked={Boolean(draft[key as keyof MachineProfileDraft])}
                      onChange={(event) =>
                        updateFlag(
                          key as
                            | "homingInstalled"
                            | "limitSwitchesInstalled"
                            | "probeInstalled"
                            | "emergencyStopInstalled",
                          event.target.checked,
                        )
                      }
                      type="checkbox"
                    />
                    {label}
                  </label>
                ))}
              </fieldset>

              {draft.detectedController && (
                <div className="detected-machine-meta">
                  <ScanLine aria-hidden="true" size={14} />
                  <span>
                    GRBL {draft.detectedController.firmwareVersion ?? "detected"}
                  </span>
                  <code>$130 · $131 · $132</code>
                </div>
              )}

              <p
                aria-hidden={!formError}
                className={`machine-form-error${formError ? "" : " is-empty"}`}
              >
                {formError ?? "Нет ошибок"}
              </p>
            </div>

            <footer>
              <button
                disabled={dialogBusy}
                onClick={() => {
                  setOpen(false);
                  onOnboardingDismiss();
                }}
                type="button"
              >
                Отмена
              </button>
              <button
                className="save-machine-action"
                disabled={dialogBusy}
                onClick={() => void submit()}
                type="button"
              >
                {onboardingDraft ? "Добавить и привязать" : "Добавить и выбрать"}
              </button>
            </footer>
          </section>
        </div>
      )}
    </>
  );
}

export type { MachineProfile };
