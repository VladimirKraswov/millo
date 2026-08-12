import { describe, expect, it } from "vitest";

import { CapabilityGrantStore } from "./CapabilityGrantStore";

describe("CapabilityGrantStore", () => {
  it("merges grants and returns them in catalog order", () => {
    const grants = new CapabilityGrantStore([
      {
        pluginId: "dev.millo.sample",
        capabilities: ["machine.jog"],
      },
      {
        pluginId: "dev.millo.sample",
        capabilities: ["ui.contribute", "machine.jog"],
      },
    ]);

    expect(grants.list("dev.millo.sample")).toEqual([
      "ui.contribute",
      "machine.jog",
    ]);
    expect(grants.has("dev.millo.sample", "jobs.create")).toBe(false);
  });

  it("rejects malformed plugin IDs and forged runtime capabilities", () => {
    expect(
      () => new CapabilityGrantStore([{ pluginId: "not valid", capabilities: [] }]),
    ).toThrow("valid plugin id");
    expect(
      () => new CapabilityGrantStore([
        {
          pluginId: "dev.millo.sample",
          capabilities: ["serial.raw" as never],
        },
      ]),
    ).toThrow("unknown plugin capability grant");
  });
});
