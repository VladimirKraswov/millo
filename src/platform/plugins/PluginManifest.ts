export const PLUGIN_MANIFEST_VERSION = 1 as const;
export const PLUGIN_API_VERSION = 1 as const;

export const pluginCapabilityCatalog = [
  "ui.contribute",
  "machine.read",
  "machine.jog",
  "jobs.create",
  "tools.read",
] as const;

export type PluginCapability = (typeof pluginCapabilityCatalog)[number];

export interface PluginManifestV1 {
  readonly manifestVersion: typeof PLUGIN_MANIFEST_VERSION;
  readonly apiVersion: number;
  readonly id: string;
  readonly name: string;
  readonly version: string;
  readonly capabilities: {
    readonly required: readonly PluginCapability[];
    readonly optional: readonly PluginCapability[];
  };
}

export class PluginManifestError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "PluginManifestError";
  }
}

export function parsePluginManifest(json: string): PluginManifestV1 {
  let value: unknown;
  try {
    value = JSON.parse(json);
  } catch (error) {
    throw new PluginManifestError(`plugin manifest is not valid JSON: ${error}`);
  }
  return validatePluginManifest(value);
}

export function validatePluginManifest(value: unknown): PluginManifestV1 {
  if (!isRecord(value)) {
    throw new PluginManifestError("plugin manifest must be an object");
  }
  if (value.manifestVersion !== PLUGIN_MANIFEST_VERSION) {
    throw new PluginManifestError(
      `unsupported plugin manifest version: ${String(value.manifestVersion)}`,
    );
  }
  if (!Number.isInteger(value.apiVersion) || Number(value.apiVersion) < 1) {
    throw new PluginManifestError("plugin apiVersion must be a positive integer");
  }

  const id = requiredString(value.id, "id");
  if (!/^[a-z0-9]+(?:[.-][a-z0-9]+)+$/.test(id)) {
    throw new PluginManifestError(
      "plugin id must contain lowercase dot- or dash-separated segments",
    );
  }
  const name = requiredString(value.name, "name");
  const version = requiredString(value.version, "version");
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new PluginManifestError("plugin version must use semantic versioning");
  }
  if (!isRecord(value.capabilities)) {
    throw new PluginManifestError("plugin capabilities must be an object");
  }

  const required = capabilityList(value.capabilities.required, "required", false);
  const optional = capabilityList(value.capabilities.optional, "optional", true);
  const duplicate = required.find((capability) => optional.includes(capability));
  if (duplicate) {
    throw new PluginManifestError(
      `plugin capability cannot be both required and optional: ${duplicate}`,
    );
  }

  return Object.freeze({
    manifestVersion: PLUGIN_MANIFEST_VERSION,
    apiVersion: Number(value.apiVersion),
    id,
    name,
    version,
    capabilities: Object.freeze({
      required: Object.freeze(required),
      optional: Object.freeze(optional),
    }),
  });
}

function capabilityList(
  value: unknown,
  field: string,
  optional: boolean,
): PluginCapability[] {
  if (value === undefined && optional) return [];
  if (!Array.isArray(value)) {
    throw new PluginManifestError(`plugin capability ${field} must be an array`);
  }

  const capabilities = value.map((capability) => {
    if (
      typeof capability !== "string" ||
      !pluginCapabilityCatalog.includes(capability as PluginCapability)
    ) {
      throw new PluginManifestError(
        `unknown plugin capability in ${field}: ${String(capability)}`,
      );
    }
    return capability as PluginCapability;
  });
  if (new Set(capabilities).size !== capabilities.length) {
    throw new PluginManifestError(
      `plugin capability ${field} contains duplicates`,
    );
  }
  return capabilities;
}

function requiredString(value: unknown, field: string): string {
  if (typeof value !== "string" || !value.trim() || value !== value.trim()) {
    throw new PluginManifestError(
      `plugin manifest ${field} must be a non-empty trimmed string`,
    );
  }
  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
