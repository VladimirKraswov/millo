import { describe, expect, it } from "vitest";

import {
  consolePolicyMessage,
  normalizeConsoleCommand,
  safeConsoleCommand,
  safeConsoleCommands,
} from "./operatorConsoleModel";

describe("operatorConsoleModel", () => {
  it("normalizes and accepts only the read-only command palette", () => {
    expect(normalizeConsoleCommand("  $i ")).toBe("$I");
    expect(safeConsoleCommands.map(({ command }) => command)).toEqual([
      "?",
      "$I",
      "$$",
      "$G",
      "$#",
    ]);
    expect(safeConsoleCommand("$g")?.kind).toBe("modalState");
  });

  it.each(["G0 X1", "$100=1", "$X", "$H", "M3", "!", "~"])(
    "rejects state-changing input %s before invoking Tauri",
    (command) => {
      expect(safeConsoleCommand(command)).toBeUndefined();
      expect(consolePolicyMessage(command)).toContain("безопасный список");
    },
  );
});
