import { invoke } from "@tauri-apps/api/core";

import type {
  GcodeProgram,
  ProgramParseOptions,
  ProgramParseRequest,
  ProgramSaveOutcome,
} from "../../shared/program";
import type { ProgramGateway } from "./ProgramGateway";

export const tauriProgramGateway: ProgramGateway = {
  parse: (request: ProgramParseRequest, options?: ProgramParseOptions) =>
    invoke<GcodeProgram>("parse_gcode_program", { request, options }),
  save: (request: ProgramParseRequest) =>
    invoke<ProgramSaveOutcome | undefined>("save_gcode_program", { request }),
};
