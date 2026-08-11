import { invoke } from "@tauri-apps/api/core";

import type {
  GcodeProgram,
  ProgramParseOptions,
  ProgramParseRequest,
} from "../../shared/program";
import type { ProgramGateway } from "./ProgramGateway";

export const tauriProgramGateway: ProgramGateway = {
  parse: (request: ProgramParseRequest, options?: ProgramParseOptions) =>
    invoke<GcodeProgram>("parse_gcode_program", { request, options }),
};
