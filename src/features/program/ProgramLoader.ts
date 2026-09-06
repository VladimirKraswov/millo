import type { ProgramGateway } from "../../platform/program/ProgramGateway";
import type { GcodeProgram } from "../../shared/program";

export const MAX_PROGRAM_FILE_BYTES = 64 * 1024 * 1024;
const supportedExtensions = new Set(["nc", "ngc", "gcode", "tap", "cnc"]);

export interface ProgramSourceFile {
  readonly name: string;
  readonly size: number;
  text(): Promise<string>;
}

export interface LoadedProgram {
  readonly program: GcodeProgram;
  readonly source: string;
}

export class ProgramLoader {
  constructor(private readonly gateway: ProgramGateway) {}

  async load(file: ProgramSourceFile): Promise<LoadedProgram> {
    const name = file.name.trim();
    const extension = name.split(".").pop()?.toLowerCase();
    if (!name || !extension || !supportedExtensions.has(extension)) {
      throw new Error("поддерживаются файлы .nc, .ngc, .gcode, .tap и .cnc");
    }
    if (file.size <= 0) {
      throw new Error("G-code файл пуст");
    }
    if (file.size > MAX_PROGRAM_FILE_BYTES) {
      throw new Error("G-code файл превышает лимит 64 MiB");
    }
    const source = await file.text();
    if (!source.trim()) {
      throw new Error("G-code файл не содержит команд");
    }
    return {
      program: await this.gateway.parse({ sourceName: name, source }),
      source,
    };
  }
}
