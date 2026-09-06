import type { ProgramParseRequest } from "../../shared/program";
import type { LoadedProgram } from "./ProgramLoader";

export function programDocumentRequest(document: LoadedProgram): ProgramParseRequest {
  const programId = document.program.document?.id;
  return programId
    ? { sourceName: document.program.sourceName, source: document.source, programId,
        parseOptions: { blockDelete: document.program.blockDeleteEnabled ?? false } }
    : { sourceName: document.program.sourceName, source: document.source };
}
