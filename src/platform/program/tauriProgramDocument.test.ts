import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ProgramParseRequest } from "../../shared/program";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
let sequence = 0;
const request = (blockDelete = true): ProgramParseRequest => ({
  sourceName: "large.nc", source: "G21 G90 G94\n/G1 X100 F100\nG1 X0 F100", programId: `doc-${++sequence}`,
  parseOptions: { blockDelete },
});
const expired = "[PROGRAM_DOCUMENT_EXPIRED] expired";

describe("native program document transport", () => {
  beforeEach(() => { invoke.mockReset(); });

  it("does not serialize the retained large source or local parse policy in normal IPC", async () => {
    const { invokeProgramDocument } = await import("./tauriProgramDocument");
    const input = request();
    invoke.mockResolvedValue({ ready: true });
    await invokeProgramDocument("preflight_real_run", input, { intent: "cutting" });
    expect(invoke).toHaveBeenCalledExactlyOnceWith("preflight_real_run", {
      intent: "cutting", request: { sourceName: input.sourceName, source: "", programId: input.programId },
    });
  });

  it("reopens an evicted processed export with its original Block Delete selection", async () => {
    const { tauriProgramGateway } = await import("./tauriProgramGateway");
    const input = request();
    invoke.mockRejectedValueOnce(expired).mockResolvedValueOnce({ document: { id: "reopened-export" } }).mockResolvedValueOnce({ path: "processed.nc" });
    await tauriProgramGateway.saveProcessed!(input, "processed.nc");
    expect(invoke).toHaveBeenNthCalledWith(2, "open_gcode_document", {
      request: { sourceName: input.sourceName, source: input.source }, options: { blockDelete: true },
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "save_processed_gcode_document", {
      sourceName: "processed.nc", request: { sourceName: input.sourceName, source: "", programId: "reopened-export" },
    });
  });

  it("recovers exact pages and selected geometry without changing the UI revision ID", async () => {
    const { tauriProgramGateway } = await import("./tauriProgramGateway");
    const input = request(false);
    invoke.mockRejectedValueOnce(expired).mockResolvedValueOnce({ document: { id: "reopened-page" } })
      .mockResolvedValueOnce({ programId: "reopened-page", startIndex: 999_936, totalLines: 1_000_000, lines: [] });
    const page = await tauriProgramGateway.linePage!(input, 999_936, 512);
    expect(page.programId).toBe(input.programId);
    invoke.mockResolvedValueOnce({ programId: "reopened-page", line: { sourceLine: 1_000_000 }, toolpath: [] });
    const detail = await tauriProgramGateway.lineDetail!(input, 1_000_000);
    expect(detail.programId).toBe(input.programId);
    expect(invoke).toHaveBeenLastCalledWith("program_line_detail", { programId: "reopened-page", sourceLine: 1_000_000 });
    expect(invoke.mock.calls.filter(([command]) => command === "open_gcode_document")).toHaveLength(1);
  });

  it("coalesces concurrent cache misses for the same immutable revision", async () => {
    const { readProgramDocument } = await import("./tauriProgramDocument");
    const input = request();
    invoke.mockImplementation((command, args) => {
      if (command === "open_gcode_document") return Promise.resolve({ document: { id: "concurrent" } });
      return args.programId === input.programId ? Promise.reject(expired) : Promise.resolve({});
    });
    await Promise.all([readProgramDocument("program_line_page", input, {}), readProgramDocument("program_line_detail", input, {})]);
    expect(invoke.mock.calls.filter(([command]) => command === "open_gcode_document")).toHaveLength(1);
  });

  it("never retries uncertain dispatch failures or guesses missing parse options", async () => {
    const { invokeProgramDocument } = await import("./tauriProgramDocument");
    for (const failure of ["transport I/O failed", "authorization already consumed"]) {
      invoke.mockReset().mockRejectedValue(failure);
      await expect(invokeProgramDocument("start_program_run", request())).rejects.toBe(failure);
      expect(invoke).toHaveBeenCalledTimes(1);
    }
    invoke.mockReset().mockRejectedValue(expired);
    await expect(invokeProgramDocument("start_program_run", { ...request(), parseOptions: undefined })).rejects.toBe(expired);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("preserves the nested selected-run contract when reopening", async () => {
    const { invokeProgramDocument } = await import("./tauriProgramDocument");
    const input = request(false);
    invoke.mockRejectedValueOnce(expired).mockResolvedValueOnce({ document: { id: "nested" } }).mockResolvedValueOnce({});
    await invokeProgramDocument("prepare_selected_program_run", input, { sourceLine: 900_000, safeZMm: 8, rotaryClearanceConfirmed: true }, true);
    expect(invoke).toHaveBeenLastCalledWith("prepare_selected_program_run", { request: {
      request: { sourceName: input.sourceName, source: "", programId: "nested" },
      sourceLine: 900_000, safeZMm: 8, rotaryClearanceConfirmed: true,
    } });
  });
});
