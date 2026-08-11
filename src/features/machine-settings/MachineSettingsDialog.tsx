import {
  Check,
  CircleAlert,
  Clock3,
  History,
  RotateCcw,
  Search,
  Settings2,
  ShieldAlert,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import type { MachineProfile, MachineProfileState } from "../../shared/profile";
import type {
  ControllerSettingEditRequest,
  ControllerSettingsState,
  SettingGroup,
} from "../../shared/settings";
import {
  filterSettings,
  settingGroupLabels,
  settingGroupOrder,
  settingValuesEqual,
} from "./machineSettingsModel";

type WriteStatus = "idle" | "pending" | "saving" | "saved" | "error";
type SettingsView = "local" | "controller" | "history";

interface MachineSettingsDialogProps {
  open: boolean;
  profile?: MachineProfile;
  settings?: ControllerSettingsState;
  onClose: () => void;
  onLocalUpdate: (profile: MachineProfile) => Promise<MachineProfileState>;
  onRollback: (key: string, revision: number) => Promise<ControllerSettingsState>;
  onWrite: (
    request: ControllerSettingEditRequest,
  ) => Promise<ControllerSettingsState>;
}

const settingValue = (state: ControllerSettingsState, key: string): string | undefined =>
  state.snapshot.values.find((setting) => setting.key === key)?.value;

export function MachineSettingsDialog({
  open,
  profile,
  settings,
  onClose,
  onLocalUpdate,
  onRollback,
  onWrite,
}: MachineSettingsDialogProps) {
  const [view, setView] = useState<SettingsView>("local");
  const [query, setQuery] = useState("");
  const [controllerEditing, setControllerEditing] = useState(false);
  const [draftValues, setDraftValues] = useState<Record<string, string>>({});
  const [statuses, setStatuses] = useState<Record<string, WriteStatus>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [localDraft, setLocalDraft] = useState<MachineProfile>();
  const [localStatus, setLocalStatus] = useState<WriteStatus>("idle");
  const settingsRef = useRef(settings);
  const timers = useRef(new Map<string, number>());
  const localTimer = useRef<number | undefined>(undefined);
  const writeQueue = useRef(Promise.resolve());

  useEffect(() => {
    settingsRef.current = settings;
    if (!settings) return;
    setDraftValues((current) => {
      const next = { ...current };
      for (const setting of settings.snapshot.values) {
        if (!(["pending", "saving"] as WriteStatus[]).includes(statuses[setting.key])) {
          next[setting.key] = setting.value;
        }
      }
      return next;
    });
  }, [settings, statuses]);

  useEffect(() => setLocalDraft(profile ? { ...profile } : undefined), [profile]);

  useEffect(
    () => () => {
      for (const timer of timers.current.values()) window.clearTimeout(timer);
      if (localTimer.current !== undefined) window.clearTimeout(localTimer.current);
    },
    [],
  );

  const filtered = useMemo(
    () => filterSettings(settings?.snapshot.values ?? [], query),
    [query, settings],
  );
  const grouped = useMemo(
    () =>
      settingGroupOrder
        .map((group) => ({
          group,
          values: filtered.filter((setting) => setting.group === group),
        }))
        .filter(({ values }) => values.length > 0),
    [filtered],
  );

  if (!open) return null;

  const mark = (key: string, status: WriteStatus, error?: string) => {
    setStatuses((current) => ({ ...current, [key]: status }));
    setErrors((current) => ({ ...current, [key]: error ?? "" }));
  };

  const enqueue = (
    key: string,
    value: string,
    operation: "write" | "rollback" = "write",
  ) => {
    mark(key, "saving");
    writeQueue.current = writeQueue.current
      .catch(() => undefined)
      .then(async () => {
        const current = settingsRef.current;
        if (!current) throw new Error("Контроллер не синхронизирован");
        const currentValue = settingValue(current, key);
        if (operation === "write" && currentValue !== undefined && settingValuesEqual(currentValue, value)) {
          mark(key, "saved");
          return;
        }
        try {
          const next =
            operation === "rollback"
              ? await onRollback(key, current.snapshot.revision)
              : await onWrite({
                  key,
                  value,
                  confirmed: true,
                  expectedValue: currentValue ?? "",
                  expectedRevision: current.snapshot.revision,
                });
          settingsRef.current = next;
          setDraftValues((draft) => ({
            ...draft,
            [key]: settingValue(next, key) ?? value,
          }));
          mark(key, "saved");
        } catch (error) {
          mark(key, "error", String(error));
        }
      });
  };

  const scheduleWrite = (key: string, value: string, delay = 650) => {
    const previous = timers.current.get(key);
    if (previous !== undefined) window.clearTimeout(previous);
    mark(key, "pending");
    timers.current.set(
      key,
      window.setTimeout(() => {
        timers.current.delete(key);
        enqueue(key, value);
      }, delay),
    );
  };

  const flushWrite = (key: string) => {
    const timer = timers.current.get(key);
    if (timer === undefined || statuses[key] !== "pending") return;
    window.clearTimeout(timer);
    timers.current.delete(key);
    enqueue(key, draftValues[key]);
  };

  const updateLocalDraft = (next: MachineProfile) => {
    setLocalDraft(next);
    setLocalStatus("pending");
    if (localTimer.current !== undefined) window.clearTimeout(localTimer.current);
    localTimer.current = window.setTimeout(async () => {
      setLocalStatus("saving");
      try {
        await onLocalUpdate(next);
        setLocalStatus("saved");
      } catch {
        setLocalStatus("error");
      }
    }, 500);
  };

  const statusIcon = (status: WriteStatus | undefined) => {
    if (status === "pending") return <Clock3 aria-label="Ожидает записи" size={14} />;
    if (status === "saving") return <Settings2 aria-label="Запись и проверка" size={14} />;
    if (status === "saved") return <Check aria-label="Проверено" size={14} />;
    if (status === "error") return <CircleAlert aria-label="Ошибка записи" size={14} />;
    return null;
  };

  return (
    <div className="machine-dialog-backdrop" role="presentation">
      <section
        aria-labelledby="settings-dialog-title"
        aria-modal="true"
        className="machine-dialog machine-settings-dialog"
        role="dialog"
      >
        <header>
          <div>
            <span>Machine settings</span>
            <h2 id="settings-dialog-title">{profile?.name ?? "Настройки станка"}</h2>
          </div>
          <button aria-label="Закрыть" onClick={onClose} title="Закрыть" type="button">
            <X aria-hidden="true" size={18} />
          </button>
        </header>

        <nav className="settings-tabs" aria-label="Раздел настроек">
          {(["local", "controller", "history"] as const).map((tab) => (
            <button
              aria-selected={view === tab}
              key={tab}
              onClick={() => setView(tab)}
              role="tab"
              type="button"
            >
              {tab === "local" ? "Основное" : tab === "controller" ? "Контроллер" : "Ревизии"}
            </button>
          ))}
        </nav>

        <div className="machine-dialog-body settings-dialog-body">
          {view === "local" && localDraft && (
            <section className="local-machine-settings">
              <div className={`autosave-state is-${localStatus}`}>
                {statusIcon(localStatus)}
                <span>Локальные данные сохраняются автоматически</span>
              </div>
              <label className="machine-name-field">
                <span>Название</span>
                <input
                  maxLength={80}
                  onChange={(event) => updateLocalDraft({ ...localDraft, name: event.target.value })}
                  type="text"
                  value={localDraft.name}
                />
              </label>
              <div className="controller-travel-summary">
                <span>Рабочая область из контроллера</span>
                <strong>
                  {localDraft.travelMm.x} × {localDraft.travelMm.y} × {localDraft.travelMm.z} mm
                </strong>
                <small>$130 · $131 · $132 редактируются во вкладке «Контроллер»</small>
              </div>
              <fieldset className="spindle-mode">
                <legend>Управление шпинделем</legend>
                {(["manual", "controller"] as const).map((mode) => (
                  <label key={mode}>
                    <input
                      checked={localDraft.spindleControl === mode}
                      name="settings-spindle"
                      onChange={() => updateLocalDraft({ ...localDraft, spindleControl: mode })}
                      type="radio"
                    />
                    {mode === "manual" ? "Вручную" : "Контроллером"}
                  </label>
                ))}
              </fieldset>
              <fieldset className="hardware-flags">
                <legend>Физически установлено</legend>
                {[
                  ["limitSwitchesInstalled", "Концевики"],
                  ["homingInstalled", "Homing"],
                  ["probeInstalled", "Датчик касания"],
                  ["emergencyStopInstalled", "Аварийная кнопка"],
                ].map(([key, label]) => (
                  <label key={key}>
                    <input
                      checked={Boolean(localDraft[key as keyof MachineProfile])}
                      onChange={(event) =>
                        updateLocalDraft({ ...localDraft, [key]: event.target.checked })
                      }
                      type="checkbox"
                    />
                    {label}
                  </label>
                ))}
              </fieldset>
            </section>
          )}

          {view === "controller" && (
            <section className="controller-settings">
              {settings ? (
                <>
                  <div className="settings-toolbar">
                    <label className="settings-search">
                      <Search aria-hidden="true" size={15} />
                      <input
                        onChange={(event) => setQuery(event.target.value)}
                        placeholder="Найти $120, acceleration, mm..."
                        type="search"
                        value={query}
                      />
                    </label>
                    <label className="controller-edit-toggle">
                      <input
                        checked={controllerEditing}
                        onChange={(event) => setControllerEditing(event.target.checked)}
                        type="checkbox"
                      />
                      <span>Разрешить запись в GRBL</span>
                    </label>
                  </div>
                  {!controllerEditing && (
                    <div className="controller-write-warning">
                      <ShieldAlert aria-hidden="true" size={17} />
                      <span>Поля доступны только после явного разрешения. Каждая запись проверяется повторным `$$`.</span>
                    </div>
                  )}
                  <div className="settings-groups">
                    {grouped.map(({ group, values }) => (
                      <section className="settings-group" key={group}>
                        <header>
                          <h3>{settingGroupLabels[group as SettingGroup]}</h3>
                          <span>{values.length}</span>
                        </header>
                        <div>
                          {values.map((setting) => {
                            const baseline = settings.sessionBaseline[setting.key];
                            const previous = settings.previousBaseline?.[setting.key];
                            const changed = baseline !== undefined && !settingValuesEqual(setting.value, baseline);
                            return (
                              <label className={`setting-row ${changed ? "is-changed" : ""}`} key={setting.key}>
                                <code>{setting.key}</code>
                                <span>
                                  <strong>{setting.title}</strong>
                                  <small>{setting.known ? setting.unit ?? "GRBL" : "Unknown firmware setting"}</small>
                                </span>
                                {setting.kind === "boolean" ? (
                                  <input
                                    checked={(draftValues[setting.key] ?? setting.value) === "1"}
                                    disabled={!controllerEditing}
                                    onChange={(event) => {
                                      const value = event.target.checked ? "1" : "0";
                                      setDraftValues((current) => ({ ...current, [setting.key]: value }));
                                      scheduleWrite(setting.key, value);
                                    }}
                                    type="checkbox"
                                  />
                                ) : (
                                  <input
                                    disabled={!controllerEditing}
                                    inputMode="decimal"
                                    onBlur={() => flushWrite(setting.key)}
                                    onChange={(event) => {
                                      const value = event.target.value;
                                      setDraftValues((current) => ({ ...current, [setting.key]: value }));
                                      scheduleWrite(setting.key, value);
                                    }}
                                    type="text"
                                    value={draftValues[setting.key] ?? setting.value}
                                  />
                                )}
                                <span className={`setting-write-state is-${statuses[setting.key] ?? "idle"}`}>
                                  {statusIcon(statuses[setting.key])}
                                </span>
                                <button
                                  aria-label={`Откатить ${setting.key} к ${baseline}`}
                                  disabled={!controllerEditing || !changed || statuses[setting.key] === "saving"}
                                  onClick={() => enqueue(setting.key, baseline ?? setting.value, "rollback")}
                                  title={changed ? `К baseline сессии: ${baseline}` : "Значение не менялось"}
                                  type="button"
                                >
                                  <RotateCcw aria-hidden="true" size={14} />
                                </button>
                                <button
                                  aria-label={`Восстановить ${setting.key} из предыдущей сессии`}
                                  className="restore-previous-setting"
                                  disabled={
                                    !controllerEditing ||
                                    previous === undefined ||
                                    settingValuesEqual(setting.value, previous) ||
                                    statuses[setting.key] === "saving"
                                  }
                                  onClick={() => enqueue(setting.key, previous ?? setting.value)}
                                  title={previous === undefined ? "Нет предыдущей ревизии" : `Предыдущая сессия: ${previous}`}
                                  type="button"
                                >
                                  <History aria-hidden="true" size={14} />
                                </button>
                                {errors[setting.key] && <small className="setting-error">{errors[setting.key]}</small>}
                              </label>
                            );
                          })}
                        </div>
                      </section>
                    ))}
                  </div>
                </>
              ) : (
                <div className="inspector-empty">
                  <strong>Контроллер не подключён</strong>
                  <span>Настройки всегда считываются заново при подключении</span>
                </div>
              )}
            </section>
          )}

          {view === "history" && (
            <section className="settings-history">
              <History aria-hidden="true" size={22} />
              <div>
                <strong>Baseline текущего подключения</strong>
                <span>
                  Откат у поля всегда возвращает значение, считанное при этом подключении.
                </span>
              </div>
              <dl>
                <div><dt>Fingerprint</dt><dd>{settings?.fingerprint.label ?? "Нет соединения"}</dd></div>
                <div><dt>Надёжность ID</dt><dd>{settings?.fingerprint.confidence ?? "--"}</dd></div>
                <div><dt>Архивных ревизий</dt><dd>{settings?.revisionCount ?? 0}</dd></div>
                <div><dt>Параметров</dt><dd>{settings?.snapshot.values.length ?? 0}</dd></div>
              </dl>
              {settings?.previousBaseline && (
                <div className="previous-revision">
                  <strong>Предыдущая сессия сохранена</strong>
                  <span>Её значения доступны как резервная копия и не заменяют данные контроллера.</span>
                </div>
              )}
            </section>
          )}
        </div>
      </section>
    </div>
  );
}
