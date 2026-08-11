import { describe, expect, it } from "vitest";

import {
  PLUGIN_API_VERSION,
  PLUGIN_MANIFEST_VERSION,
  PluginManifestError,
  parsePluginManifest,
  validatePluginManifest,
} from "./PluginManifest";

const validManifest = {
  manifestVersion: PLUGIN_MANIFEST_VERSION,
  apiVersion: PLUGIN_API_VERSION,
  id: "dev.millo.sample",
  name: "Sample plugin",
  version: "1.2.3",
  capabilities: {
    required: ["ui.contribute"],
    optional: ["machine.jog"],
  },
};

describe("PluginManifest v1", () => {
  it("parses and freezes a valid versioned manifest", () => {
    const manifest = parsePluginManifest(JSON.stringify(validManifest));

    expect(manifest).toEqual(validManifest);
    expect(Object.isFrozen(manifest)).toBe(true);
    expect(Object.isFrozen(manifest.capabilities.required)).toBe(true);
  });

  it("rejects malformed JSON and unsupported manifest versions", () => {
    expect(() => parsePluginManifest("{"))
      .toThrow(PluginManifestError);
    expect(() =>
      validatePluginManifest({ ...validManifest, manifestVersion: 2 }),
    ).toThrow("unsupported plugin manifest version");
  });

  it("rejects unknown, duplicated, or overlapping capabilities", () => {
    expect(() =>
      validatePluginManifest({
        ...validManifest,
        capabilities: { required: ["serial.raw"] },
      }),
    ).toThrow("unknown plugin capability");
    expect(() =>
      validatePluginManifest({
        ...validManifest,
        capabilities: {
          required: ["ui.contribute", "ui.contribute"],
        },
      }),
    ).toThrow("contains duplicates");
    expect(() =>
      validatePluginManifest({
        ...validManifest,
        capabilities: {
          required: ["machine.jog"],
          optional: ["machine.jog"],
        },
      }),
    ).toThrow("both required and optional");
  });

  it("rejects ambiguous IDs and non-semantic versions", () => {
    expect(() =>
      validatePluginManifest({ ...validManifest, id: "Sample Plugin" }),
    ).toThrow("lowercase dot- or dash-separated segments");
    expect(() =>
      validatePluginManifest({ ...validManifest, version: "latest" }),
    ).toThrow("semantic versioning");
  });
});
