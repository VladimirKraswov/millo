import type {
  GcodeProgram,
  ProgramParseOptions,
  ProgramParseRequest,
} from "../../shared/program";

export interface ProgramGateway {
  parse(
    request: ProgramParseRequest,
    options?: ProgramParseOptions,
  ): Promise<GcodeProgram>;
}
