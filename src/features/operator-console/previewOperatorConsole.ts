import type {
  ControllerSnapshot,
  OperatorConsoleCommandKind,
  OperatorConsoleExchange,
} from "../../shared/machine";
import { safeConsoleCommand } from "./operatorConsoleModel";

const fixtureLines: Record<OperatorConsoleCommandKind, readonly string[]> = {
  status: ["<Idle|MPos:152.400,91.200,-4.750|WPos:12.400,8.200,5.250|FS:0,0>"],
  buildInfo: ["[VER:1.1h.20260814:Millo VMC-3]", "[OPT:VMZHL,35,254]"],
  settings: ["$0=10", "$1=25", "$100=1600.000", "$101=1600.000", "$102=400.000"],
  modalState: ["[GC:G0 G54 G17 G21 G90 G94 M5 M9 T0 F0 S0]"],
  parameters: ["[G54:140.000,83.000,-10.000]", "[PRB:0.000,0.000,0.000:0]"],
  raw: ["ok", "[MSG:Expert command accepted by the controller actor]"],
};

export const previewOperatorConsole = async (
  command: string,
  snapshot: ControllerSnapshot,
  safeCommandMode = true,
): Promise<OperatorConsoleExchange> => {
  const descriptor = safeConsoleCommand(command);
  if (!descriptor && safeCommandMode) {
    throw new Error("Команда заблокирована безопасным режимом");
  }
  if (!descriptor) {
    return {
      command: command.trim(),
      kind: "raw",
      completion: "ok",
      lines: [...fixtureLines.raw],
      snapshot,
    };
  }
  return {
    command: descriptor.command,
    kind: descriptor.kind,
    completion: "ok",
    lines: [...fixtureLines[descriptor.kind]],
    snapshot,
  };
};
