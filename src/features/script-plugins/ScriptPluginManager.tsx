import {
  Braces,
  Check,
  Download,
  FileUp,
  LockKeyhole,
  Plus,
  Save,
  ShieldCheck,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import type { ScriptPluginGateway } from "../../platform/plugins/ScriptPluginGateway";
import {
  capabilityLabels,
  type InstalledScriptPlugin,
  type ScriptCapability,
  type ScriptPluginPackage,
} from "../../shared/scriptPlugins";

interface ScriptPluginManagerProps {
  readonly gateway: ScriptPluginGateway;
  readonly onChange: (plugins: readonly InstalledScriptPlugin[]) => void;
  readonly onClose: () => void;
  readonly onError: (error?: string) => void;
  readonly open: boolean;
  readonly plugins: readonly InstalledScriptPlugin[];
}

export function ScriptPluginManager({
  gateway,
  onChange,
  onClose,
  onError,
  open,
  plugins,
}: ScriptPluginManagerProps) {
  const [selectedId, setSelectedId] = useState<string>();
  const [source, setSource] = useState("");
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);
  const [localError, setLocalError] = useState<string>();
  const selected =
    plugins.find((plugin) => plugin.package.manifest.id === selectedId) ??
    plugins[0];
  const declaredCapabilities = useMemo(
    () =>
      selected
        ? [
            ...selected.package.manifest.capabilities.required,
            ...selected.package.manifest.capabilities.optional,
          ]
        : [],
    [selected],
  );

  useEffect(() => {
    if (!selected) return;
    setSelectedId(selected.package.manifest.id);
    setSource(selected.package.source);
    setSaved(false);
  }, [selected?.digest]);

  if (!open) return null;

  const refresh = async (preferredId?: string) => {
    const next = await gateway.list();
    onChange(next);
    if (preferredId) setSelectedId(preferredId);
  };
  const perform = async (action: () => Promise<void>) => {
    setBusy(true);
    setLocalError(undefined);
    onError(undefined);
    try {
      await action();
    } catch (error) {
      const message = String(error);
      setLocalError(message);
      onError(message);
    } finally {
      setBusy(false);
    }
  };
  const importPackage = () =>
    perform(async () => {
      const imported = await gateway.importPackage();
      if (imported) await refresh(imported.package.manifest.id);
    });
  const createMacro = () =>
    perform(async () => {
      const suffix = Date.now().toString(36);
      const pluginPackage = newMacroTemplate(suffix);
      const created = await gateway.savePackage(pluginPackage);
      await refresh(created.package.manifest.id);
    });
  const saveSource = () => {
    if (!selected || selected.bundled) return;
    void perform(async () => {
      const updated = await gateway.savePackage({
        ...selected.package,
        source,
      });
      setSaved(true);
      await refresh(updated.package.manifest.id);
    });
  };
  const toggleEnabled = () => {
    if (!selected) return;
    void perform(async () => {
      const grants = selected.enabled
        ? selected.grantedCapabilities
        : Array.from(
            new Set([
              ...selected.package.manifest.capabilities.required,
              ...selected.grantedCapabilities,
            ]),
          );
      await gateway.configure(
        selected.package.manifest.id,
        selected.digest,
        !selected.enabled,
        grants,
      );
      await refresh(selected.package.manifest.id);
    });
  };
  const toggleCapability = (capability: ScriptCapability) => {
    if (!selected || selected.enabled) return;
    const granted = selected.grantedCapabilities.includes(capability)
      ? selected.grantedCapabilities.filter((item) => item !== capability)
      : [...selected.grantedCapabilities, capability];
    void perform(async () => {
      await gateway.configure(
        selected.package.manifest.id,
        selected.digest,
        false,
        granted,
      );
      await refresh(selected.package.manifest.id);
    });
  };

  return (
    <div className="script-dialog-backdrop" role="presentation">
      <section aria-labelledby="script-manager-title" aria-modal="true" className="script-manager" role="dialog">
        <header>
          <div>
            <small>Расширения Millo</small>
            <h2 id="script-manager-title">Макросы и плагины</h2>
          </div>
          <button aria-label="Закрыть" className="icon-button" onClick={onClose} type="button">
            <X aria-hidden="true" size={20} />
          </button>
        </header>

        <div className="script-manager-toolbar">
          <button disabled={busy} onClick={() => void createMacro()} type="button">
            <Plus aria-hidden="true" size={15} /> Новый макрос
          </button>
          <button disabled={busy} onClick={() => void importPackage()} type="button">
            <FileUp aria-hidden="true" size={15} /> Импорт
          </button>
          <span className={localError ? "is-error" : ""} role={localError ? "alert" : undefined}>
            <LockKeyhole aria-hidden="true" size={14} />
            {localError ?? "Код работает в sandbox"}
          </span>
        </div>

        <div className="script-manager-body">
          <nav aria-label="Установленные плагины" className="script-plugin-list">
            {plugins.map((plugin) => (
              <button
                className={plugin.package.manifest.id === selected?.package.manifest.id ? "is-active" : ""}
                key={plugin.package.manifest.id}
                onClick={() => setSelectedId(plugin.package.manifest.id)}
                type="button"
              >
                <Braces aria-hidden="true" size={17} />
                <span>
                  <strong>{plugin.package.manifest.name}</strong>
                  <small>{plugin.bundled ? "Системный" : "Пользовательский"} · {plugin.enabled ? "Включён" : "Выключен"}</small>
                </span>
                <i className={plugin.enabled ? "is-enabled" : ""} />
              </button>
            ))}
          </nav>

          {selected ? (
            <div className="script-plugin-detail">
              <div className="script-plugin-summary">
                <div>
                  <span>{selected.package.manifest.id}</span>
                  <h3>{selected.package.manifest.name}</h3>
                  <p>{selected.package.manifest.description}</p>
                </div>
                <div className="script-plugin-actions">
                  <button
                    aria-label="Экспортировать пакет"
                    disabled={busy}
                    onClick={() => void perform(async () => {
                      await gateway.exportPackage(selected.package.manifest.id, selected.digest);
                    })}
                    title="Экспортировать .millo-plugin"
                    type="button"
                  >
                    <Download aria-hidden="true" size={15} />
                  </button>
                  <button
                    className={selected.enabled ? "plugin-toggle is-enabled" : "plugin-toggle"}
                    disabled={busy}
                    onClick={toggleEnabled}
                    type="button"
                  >
                    {selected.enabled ? <Check aria-hidden="true" size={15} /> : <ShieldCheck aria-hidden="true" size={15} />}
                    {selected.enabled ? "Включён" : "Проверить и включить"}
                  </button>
                </div>
              </div>

              <section className="script-capabilities">
                <header>
                  <strong>Разрешения</strong>
                  <code>{selected.digest.slice(0, 12)}</code>
                </header>
                <div>
                  {declaredCapabilities.map((capability) => (
                    <label key={capability}>
                      <input
                        checked={selected.grantedCapabilities.includes(capability)}
                        disabled={busy || selected.enabled || selected.package.manifest.capabilities.required.includes(capability)}
                        onChange={() => toggleCapability(capability)}
                        type="checkbox"
                      />
                      <span>{capabilityLabels[capability]}</span>
                      {selected.package.manifest.capabilities.required.includes(capability) && <small>обязательно</small>}
                    </label>
                  ))}
                </div>
              </section>

              <section className="script-command-catalog">
                <strong>Команды</strong>
                <div>
                  {selected.package.commands.map((command) => (
                    <span key={command.id}>{command.title}</span>
                  ))}
                </div>
              </section>

              <section className="script-editor">
                <header>
                  <div>
                    <strong>Rhai script</strong>
                    <small>{selected.bundled ? "Системный код доступен только для чтения" : "Изменение сбросит доверие и выключит плагин"}</small>
                  </div>
                  {!selected.bundled && (
                    <button disabled={busy || source === selected.package.source} onClick={saveSource} type="button">
                      <Save aria-hidden="true" size={14} /> {saved ? "Сохранено" : "Сохранить"}
                    </button>
                  )}
                </header>
                <textarea
                  aria-label="Исходный код плагина"
                  onChange={(event) => {
                    setSource(event.target.value);
                    setSaved(false);
                  }}
                  readOnly={selected.bundled}
                  spellCheck={false}
                  value={source}
                />
              </section>

              {!selected.bundled && (
                <button
                  className="script-delete"
                  disabled={busy}
                  onClick={() => void perform(async () => {
                    await gateway.delete(selected.package.manifest.id);
                    setSelectedId(undefined);
                    await refresh();
                  })}
                  type="button"
                >
                  <Trash2 aria-hidden="true" size={14} /> Удалить плагин
                </button>
              )}
            </div>
          ) : (
            <div className="script-plugin-empty">Плагины не установлены</div>
          )}
        </div>
      </section>
    </div>
  );
}

function newMacroTemplate(suffix: string): ScriptPluginPackage {
  return {
    packageVersion: 1,
    manifest: {
      manifestVersion: 1,
      apiVersion: 1,
      id: `local.macro-${suffix}`,
      name: "Новый макрос",
      version: "1.0.0",
      description: "Пользовательская команда Millo.",
      capabilities: {
        required: ["ui.contribute"],
        optional: [
          "machine.read",
          "machine.jog",
          "machine.coordinates",
          "machine.commands",
          "jobs.create",
        ],
      },
    },
    commands: [
      {
        id: "run",
        title: "Новый макрос",
        description: "Откройте редактор и замените пример своим действием.",
        icon: "braces",
        surface: "workspaceTools",
        fields: [],
        requiredCapabilities: [],
      },
    ],
    source:
      'fn run(command, input, machine) {\n  return #{ kind: "notice", title: "Макрос работает", message: "Измените script в редакторе", tone: "success" };\n}',
  };
}
