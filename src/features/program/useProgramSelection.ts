import { useEffect, useMemo, useState } from "react";
import type { ProgramGateway } from "../../platform/program/ProgramGateway";
import type { GcodeProgram, ProgramLineDetail } from "../../shared/program";
import { programSourceIndex } from "./programSourceIndex";
import { programDocumentRequest } from "./programDocumentRequest";

export function useProgramSelection(program: GcodeProgram | undefined, sourceLine: number | undefined, gateway: ProgramGateway, source = "") {
  const index = useMemo(() => program ? programSourceIndex(program) : undefined, [program]);
  const [detail, setDetail] = useState<ProgramLineDetail>();
  const [error, setError] = useState<string>();
  const documentId = program?.document?.id;
  const inlineLine = sourceLine === undefined ? undefined : index?.lines.get(sourceLine);
  const inlineMotions = sourceLine === undefined ? undefined : index?.motions.get(sourceLine);
  const needsDetail = !!documentId && sourceLine !== undefined && (!inlineLine || program?.document?.previewSampled);

  useEffect(() => {
    let current = true;
    setError(undefined);
    if (!needsDetail || !documentId || !program || sourceLine === undefined || !gateway.lineDetail) return;
    void gateway.lineDetail(programDocumentRequest({ program, source }), sourceLine).then((next) => {
      if (current && next.programId === documentId && next.line.sourceLine === sourceLine) setDetail(next);
    }, (reason: unknown) => { if (current) setError(String(reason)); });
    return () => { current = false; };
  }, [documentId, sourceLine, needsDetail, gateway, program, source]);

  const currentDetail = needsDetail && detail?.programId === documentId && detail.line.sourceLine === sourceLine ? detail : undefined;
  return {
    sourceIndex: index,
    selectedProgramLine: currentDetail?.line ?? inlineLine,
    selectedToolpath: currentDetail?.toolpath ?? inlineMotions,
    selectionError: error,
  };
}
