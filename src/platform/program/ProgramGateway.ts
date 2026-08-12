import type {
  GcodeProgram,
  ProgramParseOptions,
  ProgramParseRequest,
  ProgramSaveOutcome,
} from "../../shared/program";

export interface ProgramGateway {
  parse(
    request: ProgramParseRequest,
    options?: ProgramParseOptions,
  ): Promise<GcodeProgram>;
  save?(request: ProgramParseRequest): Promise<ProgramSaveOutcome | undefined>;
}
