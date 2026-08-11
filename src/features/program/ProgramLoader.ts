import type { ProgramGateway } from "../../platform/program/ProgramGateway";
import type { GcodeProgram } from "../../shared/program";

export const MAX_PROGRAM_FILE_BYTES = 2 * 1024 * 1024;
const supportedExtensions = new Set(["nc", "ngc", "gcode", "tap", "cnc"]);

export interface ProgramSourceFile {
  readonly name: string;
  readonly size: number;
  text(): Promise<string>;
}

export class ProgramLoader {
  constructor(private readonly gateway: ProgramGateway) {}

  async load(file: ProgramSourceFile): Promise<GcodeProgram> {
    const name = file.name.trim();
    const extension = name.split(".").pop()?.toLowerCase();
    if (!name || !extension || !supportedExtensions.has(extension)) {
      throw new Error("поддерживаются файлы .nc, .ngc, .gcode, .tap и .cnc");
    }
    if (file.size <= 0) {
      throw new Error("G-code файл пуст");
    }
    if (file.size > MAX_PROGRAM_FILE_BYTES) {
      throw new Error("G-code файл превышает лимит 2 MB");
    }
    const source = await file.text();
    if (!source.trim()) {
      throw new Error("G-code файл не содержит команд");
    }
    return this.gateway.parse({ sourceName: name, source });
  }
}
