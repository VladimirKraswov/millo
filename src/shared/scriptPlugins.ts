import type { ControllerSnapshot } from "./machine";
import type { GeneratedJob } from "./jobs";

export const scriptCapabilityCatalog = [
  "ui.contribute",
  "machine.read",
  "machine.jog",
  "machine.coordinates",
  "machine.commands",
  "jobs.create",
] as const;

export type ScriptCapability = (typeof scriptCapabilityCatalog)[number];
export type ScriptCommandSurface = "workspaceTools" | "machinePanel";
export type ScriptFieldKind = "number" | "boolean" | "text";

export interface ScriptPluginManifest {
  readonly manifestVersion: 1;
  readonly apiVersion: 1;
  readonly id: string;
  readonly name: string;
  readonly version: string;
  readonly description: string;
  readonly capabilities: {
    readonly required: readonly ScriptCapability[];
    readonly optional: readonly ScriptCapability[];
  };
}

export interface ScriptCommandField {
  readonly id: string;
  readonly label: string;
  readonly kind: ScriptFieldKind;
  readonly defaultValue: string | number | boolean;
  readonly min?: number;
  readonly max?: number;
  readonly step?: number;
  readonly unit?: string;
}

export interface ScriptPluginCommand {
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly icon: string;
  readonly surface: ScriptCommandSurface;
  readonly fields: readonly ScriptCommandField[];
  readonly requiredCapabilities: readonly ScriptCapability[];
  readonly unavailableReason?: string;
}

export interface ScriptPluginPackage {
  readonly packageVersion: 1;
  readonly manifest: ScriptPluginManifest;
  readonly commands: readonly ScriptPluginCommand[];
  readonly source: string;
}

export interface InstalledScriptPlugin {
  readonly package: ScriptPluginPackage;
  readonly digest: string;
  readonly enabled: boolean;
  readonly bundled: boolean;
  readonly grantedCapabilities: readonly ScriptCapability[];
}

export type ScriptPluginExecutionOutcome =
  | { readonly kind: "job"; readonly job: GeneratedJob }
  | {
      readonly kind: "machine";
      readonly action: string;
      readonly message: string;
      readonly snapshot: ControllerSnapshot;
    }
  | {
      readonly kind: "notice";
      readonly title: string;
      readonly message: string;
      readonly tone: "info" | "success" | "warning";
    };

export interface ScriptPluginExecutionRequest {
  readonly pluginId: string;
  readonly digest: string;
  readonly commandId: string;
  readonly input: Readonly<Record<string, unknown>>;
  readonly operatorConfirmed: boolean;
}

export const capabilityLabels: Readonly<Record<ScriptCapability, string>> = {
  "ui.contribute": "Добавлять элементы интерфейса",
  "machine.read": "Читать состояние станка",
  "machine.jog": "Запрашивать jog-движение",
  "machine.coordinates": "Работать с рабочим нулём",
  "machine.commands": "Отправлять экспертные GRBL-команды",
  "jobs.create": "Создавать G-code задания",
};

export const commandNeedsMachineConfirmation = (
  command: ScriptPluginCommand,
): boolean =>
  command.requiredCapabilities.some(
    (capability) =>
      capability === "machine.jog" ||
      capability === "machine.coordinates" ||
      capability === "machine.commands",
  );
