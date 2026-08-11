import type { GcodeProgram, ProgramParseRequest } from "../../shared/program";

export interface ProgramGateway {
  parse(request: ProgramParseRequest): Promise<GcodeProgram>;
}
