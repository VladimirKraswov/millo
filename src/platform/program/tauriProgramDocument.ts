import { invoke } from "@tauri-apps/api/core";
import type { GcodeProgram, ProgramParseRequest } from "../../shared/program";

const replacements = new Map<string, string>();
const reopening = new Map<string, Promise<string>>();
const nativeProgramId = (id: string): string => replacements.get(id) ?? id;

async function withDocument<T>(request: ProgramParseRequest, call: (id?: string) => Promise<T>): Promise<T> {
  const originalId = request.programId;
  const attemptedId = originalId ? nativeProgramId(originalId) : undefined;
  try {
    return await call(attemptedId);
  } catch (reason) {
    // Retry only a pre-dispatch cache miss, never a command/transport error.
    if (!originalId || !request.source || !request.parseOptions || !String(reason).startsWith("[PROGRAM_DOCUMENT_EXPIRED]")) throw reason;
    const currentId = nativeProgramId(originalId);
    if (currentId !== attemptedId) return call(currentId);
    let pending = reopening.get(originalId);
    if (!pending) {
      pending = invoke<GcodeProgram>("open_gcode_document", {
        request: { sourceName: request.sourceName, source: request.source },
        options: request.parseOptions,
      }).then((document) => {
        const id = document.document?.id;
        if (!id) throw reason;
        replacements.set(originalId, id);
        if (replacements.size > 64) replacements.delete(replacements.keys().next().value!);
        return id;
      }).finally(() => { reopening.delete(originalId); });
      reopening.set(originalId, pending);
    }
    return call(await pending);
  }
}

// Source and parseOptions stay local; normal IPC carries only a handle.
export function invokeProgramDocument<T>(command: string, request: ProgramParseRequest, args: Record<string, unknown> = {}, nested = false): Promise<T> {
  return withDocument(request, (id) => {
    const program = id
      ? { sourceName: request.sourceName, source: "", programId: id }
      : { sourceName: request.sourceName, source: request.source };
    return invoke<T>(command, nested ? { request: { ...args, request: program } } : { ...args, request: program });
  });
}

export function readProgramDocument<T>(command: string, request: ProgramParseRequest, args: Record<string, unknown>): Promise<T> {
  return withDocument(request, (id) => {
    if (!id) return Promise.reject(new Error("Для чтения строк нужна загруженная программа"));
    return invoke<T>(command, { ...args, programId: id });
  });
}
