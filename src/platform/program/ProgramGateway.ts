import type {
  GcodeProgram,
  ProgramParseOptions,
  ProgramParseRequest,
  ProgramSaveOutcome,
  ProgramLinePage,
  ProgramLineDetail,
} from "../../shared/program";

export interface ProgramGateway {
  parse(
    request: ProgramParseRequest,
    options?: ProgramParseOptions,
  ): Promise<GcodeProgram>;
  save?(request: ProgramParseRequest): Promise<ProgramSaveOutcome | undefined>;
  saveProcessed?(request: ProgramParseRequest, sourceName: string): Promise<ProgramSaveOutcome | undefined>;
  linePage?(request: ProgramParseRequest, startIndex: number, count: number): Promise<ProgramLinePage>;
  lineDetail?(request: ProgramParseRequest, sourceLine: number): Promise<ProgramLineDetail>;
}
