import { ChevronLeft, ChevronRight } from "lucide-react";
import { useEffect, useState } from "react";
import type { ProgramGateway } from "../../platform/program/ProgramGateway";
import type { GcodeProgram, ProgramLinePage } from "../../shared/program";
import { ProgramLineTable } from "./ProgramLineTable";
import { programDocumentRequest } from "./programDocumentRequest";

interface Props {
  readonly program: GcodeProgram;
  readonly source?: string;
  readonly gateway?: ProgramGateway;
  readonly motionSourceLines: ReadonlySet<number>;
  readonly selectedSourceLine?: number;
  readonly onSelect: (line: number) => void;
}

export function PagedProgramLineTable(props: Props) {
  const { program, gateway, selectedSourceLine, source = "" } = props;
  const document = program.document;
  const pageSize = document?.pageSize ?? program.lines.length;
  const total = program.summary.lineCount;
  const [pageIndex, setPageIndex] = useState(0);
  const [page, setPage] = useState<ProgramLinePage>();
  const [error, setError] = useState<string>();
  const [draft, setDraft] = useState("1");
  const pages = Math.max(1, Math.ceil(total / Math.max(1, pageSize)));
  const boundedIndex = Math.min(pageIndex, pages - 1);
  const startIndex = boundedIndex * pageSize;
  const paged = !!document && total > program.lines.length;

  useEffect(() => {
    setPageIndex(0);
    setPage(undefined);
    setError(undefined);
    setDraft("1");
  }, [program]);

  useEffect(() => {
    if (selectedSourceLine === undefined || !paged) return;
    setPageIndex(Math.floor(Math.max(0, Math.min(total, selectedSourceLine) - 1) / pageSize));
    setDraft(String(selectedSourceLine));
  }, [selectedSourceLine, paged, pageSize, total]);

  useEffect(() => {
    let current = true;
    setError(undefined);
    if (!document || !paged || startIndex === 0) { setPage(undefined); return; }
    if (!gateway?.linePage) { setError("Постраничное чтение недоступно"); return; }
    void gateway.linePage(programDocumentRequest({ program, source }), startIndex, pageSize).then((result) => {
      if (current && result.programId === document.id && result.startIndex === startIndex) setPage(result);
    }, (reason: unknown) => { if (current) setError(String(reason)); });
    return () => { current = false; };
  }, [document, gateway, paged, startIndex, pageSize, program, source]);

  const ready = startIndex === 0 || page?.programId === document?.id && page?.startIndex === startIndex;
  const lines = startIndex === 0 ? program.lines : ready ? page?.lines ?? [] : [];
  const jump = () => {
    const value = Number(draft);
    if (Number.isInteger(value) && value >= 1 && value <= total) props.onSelect(value);
    else setError(`Номер строки: от 1 до ${total}`);
  };
  return <>
    {paged && <div className="program-page-toolbar">
      <button type="button" title="Предыдущие строки" aria-label="Предыдущие строки" disabled={boundedIndex === 0} onClick={() => setPageIndex((index) => Math.max(0, index - 1))}><ChevronLeft size={16} /></button>
      <span>{startIndex + 1}–{Math.min(total, startIndex + pageSize)} / {total}</span>
      <button type="button" title="Следующие строки" aria-label="Следующие строки" disabled={boundedIndex + 1 >= pages} onClick={() => setPageIndex((index) => Math.min(pages - 1, index + 1))}><ChevronRight size={16} /></button>
      <label>Строка<input aria-label="Перейти к строке" type="number" min={1} max={total} value={draft} onChange={(event) => setDraft(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") jump(); }} onBlur={jump} /></label>
    </div>}
    {error && <div className="program-page-error" role="alert">{error}</div>}
    {!ready && !error ? <div className="program-line-table" aria-busy="true">Загрузка строк…</div> :
      <ProgramLineTable lines={lines} motionSourceLines={props.motionSourceLines} selectedSourceLine={selectedSourceLine} onSelect={props.onSelect} />}
  </>;
}
