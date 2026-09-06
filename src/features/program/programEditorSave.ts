import type { ProgramGateway } from "../../platform/program/ProgramGateway";
import type { LoadedProgram } from "./ProgramLoader";
import { programDocumentRequest } from "./programDocumentRequest";
import { buildProcessedProgramSource, processedProgramName } from "./programEditorModel";

export async function saveProgramEditorDocument(
  gateway: ProgramGateway, document: LoadedProgram, transformed: boolean,
) {
  const { program, source } = document;
  if (transformed && program.document) {
    if (!gateway.saveProcessed) throw new Error("Сохранение полной обработанной копии недоступно для постраничной программы");
    return gateway.saveProcessed(programDocumentRequest(document), processedProgramName(program.sourceName));
  }
  if (!gateway.save) throw new Error("Сохранение файла недоступно");
  const saveSource = transformed ? buildProcessedProgramSource(program) : source;
  if (!/\S/.test(saveSource)) throw new Error("Обработанная копия не содержит исполняемых строк");
  return gateway.save({
    sourceName: transformed ? processedProgramName(program.sourceName) : program.sourceName,
    source: saveSource,
  });
}
