import { describe, expect, it, vi } from "vitest";

import type { ProgramGateway } from "../../platform/program/ProgramGateway";
import { previewFixtureProgram } from "./previewFixtureProgram";
import {
  MAX_PROGRAM_FILE_BYTES,
  ProgramLoader,
  type ProgramSourceFile,
} from "./ProgramLoader";

const file = (overrides: Partial<ProgramSourceFile> = {}): ProgramSourceFile => ({
  name: "profile.nc",
  size: 12,
  text: async () => "G21\nG0 X1",
  ...overrides,
});

describe("ProgramLoader", () => {
  it("reads one supported file and delegates a typed parse request", async () => {
    const parse = vi.fn(async () => previewFixtureProgram);
    const loader = new ProgramLoader({ parse });

    await expect(loader.load(file())).resolves.toBe(previewFixtureProgram);
    expect(parse).toHaveBeenCalledOnce();
    expect(parse).toHaveBeenCalledWith({
      sourceName: "profile.nc",
      source: "G21\nG0 X1",
    });
  });

  it("rejects unsupported extensions before reading or invoking", async () => {
    const parse = vi.fn(async () => previewFixtureProgram);
    const text = vi.fn(async () => "G0 X1");
    const loader = new ProgramLoader({ parse });

    await expect(loader.load(file({ name: "profile.pdf", text }))).rejects.toThrow(
      "поддерживаются файлы",
    );
    expect(text).not.toHaveBeenCalled();
    expect(parse).not.toHaveBeenCalled();
  });

  it("rejects empty and oversized files before invoking", async () => {
    const gateway: ProgramGateway = { parse: vi.fn(async () => previewFixtureProgram) };
    const loader = new ProgramLoader(gateway);

    await expect(loader.load(file({ size: 0 }))).rejects.toThrow("пуст");
    await expect(
      loader.load(file({ size: MAX_PROGRAM_FILE_BYTES + 1 })),
    ).rejects.toThrow("2 MB");
    expect(gateway.parse).not.toHaveBeenCalled();
  });
});
