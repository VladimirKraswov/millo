import { describe, expect, it } from "vitest";

import {
  consoleCommandAllowed,
  consolePolicyMessage,
  normalizeConsoleCommand,
  normalizeSubmittedConsoleCommand,
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

  it("allows one actor-owned line in expert mode without changing its case", () => {
    expect(consoleCommandAllowed("G0 X1.25", false)).toBe(true);
    expect(consoleCommandAllowed("$100=1600", false)).toBe(true);
    expect(normalizeSubmittedConsoleCommand("  $SD/Job.nc  ", false)).toBe(
      "$SD/Job.nc",
    );
    expect(consolePolicyMessage("G0 X1.25", false)).toContain("Rust actor");
  });

  it.each(["!", "~", "G0 X1\nG0 X2", "\u0018"])(
    "keeps realtime or multiline input %s outside the expert line channel",
    (command) => {
      expect(consoleCommandAllowed(command, false)).toBe(false);
    },
  );
});
