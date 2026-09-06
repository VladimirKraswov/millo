import { invoke } from "@tauri-apps/api/core";

import type {
  GcodeProgram,
  ProgramParseOptions,
  ProgramParseRequest,
  ProgramSaveOutcome,
  ProgramLinePage,
  ProgramLineDetail,
} from "../../shared/program";
import type { ProgramGateway } from "./ProgramGateway";
import { invokeProgramDocument, readProgramDocument } from "./tauriProgramDocument";

export const tauriProgramGateway: ProgramGateway = {
  parse: (request: ProgramParseRequest, options?: ProgramParseOptions) =>
    invokeProgramDocument<GcodeProgram>("open_gcode_document", request, { options }),
  linePage: async (request, startIndex, count) =>
    ({ ...await readProgramDocument<ProgramLinePage>("program_line_page", request, { startIndex, count }), programId: request.programId! }),
  lineDetail: async (request, sourceLine) =>
    ({ ...await readProgramDocument<ProgramLineDetail>("program_line_detail", request, { sourceLine }), programId: request.programId! }),
  save: (request: ProgramParseRequest) =>
    invoke<ProgramSaveOutcome | undefined>("save_gcode_program", { request }),
  saveProcessed: (request, sourceName) =>
    invokeProgramDocument<ProgramSaveOutcome | undefined>("save_processed_gcode_document", request, { sourceName }),
};
