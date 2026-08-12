import { Check, LoaderCircle, Play, X } from "lucide-react";
import { useMemo, useState } from "react";

import type { GeneratedJobStore } from "../../platform/jobs/GeneratedJobStore";
import type { MachineSnapshotStore } from "../../platform/machine/MachineStateSource";
import type { ScriptPluginGateway } from "../../platform/plugins/ScriptPluginGateway";
import {
  commandNeedsMachineConfirmation,
  type InstalledScriptPlugin,
  type ScriptPluginCommand,
} from "../../shared/scriptPlugins";
import { scriptIcon } from "./scriptIcons";

interface ScriptCommandLauncherProps {
  readonly command: ScriptPluginCommand;
  readonly gateway: ScriptPluginGateway;
  readonly jobs: GeneratedJobStore;
  readonly machine: MachineSnapshotStore;
  readonly onError: (error?: string) => void;
  readonly plugin: InstalledScriptPlugin;
  readonly compact?: boolean;
}

export function ScriptCommandLauncher({
  command,
  compact = false,
  gateway,
  jobs,
  machine,
  onError,
  plugin,
}: ScriptCommandLauncherProps) {
  const Icon = scriptIcon(command.icon);
  const initialInput = useMemo(
    () =>
      Object.fromEntries(
        command.fields.map((field) => [field.id, field.defaultValue]),
      ),
    [command],
  );
  const [open, setOpen] = useState(false);
  const [input, setInput] = useState<Record<string, unknown>>(initialInput);
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<string>();
  const [localError, setLocalError] = useState<string>();
  const needsConfirmation = commandNeedsMachineConfirmation(command);

  const execute = async () => {
    setBusy(true);
    setResult(undefined);
    setLocalError(undefined);
    onError(undefined);
    try {
      const outcome = await gateway.execute({
        pluginId: plugin.package.manifest.id,
        digest: plugin.digest,
        commandId: command.id,
        input,
        operatorConfirmed: confirmed,
      });
      if (outcome.kind === "job") {
        jobs.publish(outcome.job);
        setOpen(false);
      } else if (outcome.kind === "machine") {
        machine.publish(outcome.snapshot);
        setResult(outcome.message);
      } else {
        setResult(`${outcome.title}: ${outcome.message}`);
      }
    } catch (error) {
      const message = String(error);
      setLocalError(message);
      onError(message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <button
        className={compact ? "script-command-chip" : "script-command-launcher"}
        disabled={Boolean(command.unavailableReason)}
        onClick={() => {
          setInput(initialInput);
          setConfirmed(false);
          setResult(undefined);
          setLocalError(undefined);
          setOpen(true);
        }}
        role={compact ? undefined : "menuitem"}
        title={command.unavailableReason ?? command.description}
        type="button"
      >
        <Icon aria-hidden="true" size={compact ? 15 : 17} />
        <span>{command.title}</span>
      </button>

      {open && (
        <div className="script-dialog-backdrop" role="presentation">
          <section
            aria-labelledby={`script-command-${plugin.package.manifest.id}-${command.id}`}
            aria-modal="true"
            className="script-command-dialog"
            role="dialog"
          >
            <header>
              <div className="script-command-heading">
                <span className="script-command-icon"><Icon aria-hidden="true" size={19} /></span>
                <div>
                  <small>{plugin.package.manifest.name}</small>
                  <h2 id={`script-command-${plugin.package.manifest.id}-${command.id}`}>
                    {command.title}
                  </h2>
                </div>
              </div>
              <button
                aria-label="Закрыть"
                className="icon-button"
                onClick={() => setOpen(false)}
                type="button"
              >
                <X aria-hidden="true" size={19} />
              </button>
            </header>

            <p className="script-command-description">{command.description}</p>
            {command.fields.length > 0 && (
              <div className="script-command-fields">
                {command.fields.map((field) => (
                  <label key={field.id}>
                    <span>{field.label}</span>
                    {field.kind === "boolean" ? (
                      <input
                        checked={Boolean(input[field.id])}
                        onChange={(event) =>
                          setInput((current) => ({
                            ...current,
                            [field.id]: event.target.checked,
                          }))
                        }
                        type="checkbox"
                      />
                    ) : (
                      <span className="script-field-input">
                        <input
                          max={field.max}
                          min={field.min}
                          onChange={(event) =>
                            setInput((current) => ({
                              ...current,
                              [field.id]:
                                field.kind === "number"
                                  ? event.target.valueAsNumber
                                  : event.target.value,
                            }))
                          }
                          step={field.step}
                          type={field.kind === "number" ? "number" : "text"}
                          value={String(input[field.id] ?? "")}
                        />
                        {field.unit && <em>{field.unit}</em>}
                      </span>
                    )}
                  </label>
                ))}
              </div>
            )}

            {needsConfirmation && (
              <label className="script-motion-confirmation">
                <input
                  checked={confirmed}
                  onChange={(event) => setConfirmed(event.target.checked)}
                  type="checkbox"
                />
                <span>
                  <strong>Станок готов</strong>
                  <small>Зона свободна, инструмент и направление проверены</small>
                </span>
              </label>
            )}

            <div
              className={`script-command-result${result ? " is-visible" : ""}${localError ? " is-error" : ""}`}
              role={localError ? "alert" : undefined}
            >
              <Check aria-hidden="true" size={15} />
              <span>{localError ?? result ?? "Результат появится здесь"}</span>
            </div>
            <footer>
              <button className="secondary-action" onClick={() => setOpen(false)} type="button">
                Закрыть
              </button>
              <button
                className="primary-action"
                disabled={busy || (needsConfirmation && !confirmed)}
                onClick={() => void execute()}
                type="button"
              >
                {busy ? (
                  <LoaderCircle aria-hidden="true" className="is-spinning" size={16} />
                ) : (
                  <Play aria-hidden="true" size={16} />
                )}
                Выполнить
              </button>
            </footer>
          </section>
        </div>
      )}
    </>
  );
}
