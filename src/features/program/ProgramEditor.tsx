import { DialogSurface } from "../../components/DialogSurface";
import {
  Check,
  ChevronLeft,
  ChevronRight,
  ClipboardPaste,
  Copy,
  CornerDownLeft,
  FileOutput,
  ListPlus,
  LoaderCircle,
  Redo2,
  Save,
  Scissors,
  Trash2,
  TriangleAlert,
  Undo2,
  X,
} from "lucide-react";
import {
  lazy,
  Suspense,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type UIEvent,
} from "react";

import type { ProgramGateway } from "../../platform/program/ProgramGateway";
import type { GcodeProgram } from "../../shared/program";
import type { LoadedProgram } from "./ProgramLoader";
import {
  formatProgramDiagnostics,
  programDiagnosticsSummary,
  programWarningPresentation,
} from "./programDiagnosticsModel";
import {
  PROGRAM_EDITOR_HISTORY_BYTES,
  commitProgramEditorSource,
  createProgramEditorHistory,
  deleteProgramLines,
  insertProgramLine,
  redoProgramEditorSource,
  replaceTextSelection,
  selectedTextOrLines,
  tokenizeGcodeLine,
  undoProgramEditorSource,
  type TextEdit,
} from "./programEditorModel";
import { PROGRAM_EDITOR_PAGE_LINES } from "./programEditorPaging";
import { saveProgramEditorDocument } from "./programEditorSave";
import { useProgramEditorPaging } from "./useProgramEditorPaging";
import { useProgramSelection } from "./useProgramSelection";

const ToolpathPreview = lazy(async () => {
  const module = await import("./ToolpathPreview");
  return { default: module.ToolpathPreview };
});

const EDITOR_LINE_HEIGHT = 20;
const EDITOR_OVERSCAN = 8;
const PARSE_DELAY_MS = 140;

interface ProgramEditorProps {
  readonly blockDelete: boolean;
  readonly document: LoadedProgram;
  readonly gateway: ProgramGateway;
  readonly onApply: (document: LoadedProgram) => void;
  readonly onClose: () => void;
}

type ParseState = "ready" | "parsing" | "invalid";

const formatParseError = (reason: unknown): string =>
  String(reason).replace(/^Error:\s*/, "");

export function ProgramEditor({
  blockDelete,
  document,
  gateway,
  onApply,
  onClose,
}: ProgramEditorProps) {
  const [history, setHistory] = useState(() =>
    createProgramEditorHistory(document.source),
  );
  const [preview, setPreview] = useState<GcodeProgram>(document.program);
  const [previewSource, setPreviewSource] = useState(document.source);
  const [previewBlockDelete, setPreviewBlockDelete] = useState(blockDelete);
  const [previewGateway, setPreviewGateway] = useState(() => gateway);
  const [parseState, setParseState] = useState<ParseState>("ready");
  const [parseError, setParseError] = useState<string>();
  const [scroll, setScroll] = useState({ left: 0, top: 0 });
  const [viewportHeight, setViewportHeight] = useState(480);
  const [jumpLine, setJumpLine] = useState("1");
  const [saveBusy, setSaveBusy] = useState(false);
  const [saveNotice, setSaveNotice] = useState<string>();
  const [operationError, setOperationError] = useState<string>();
  const [discardArmed, setDiscardArmed] = useState(false);
  const editorViewportRef = useRef<HTMLDivElement>(null);
  const parseSequence = useRef(0);
  const source = history.source;
  const dirty = source !== document.source;
  const currentRevisionReady =
    parseState === "ready" && previewSource === source &&
    previewBlockDelete === blockDelete && previewGateway === gateway;
  const paging = useProgramEditorPaging(source, (edit) => commit(edit), setScroll);
  const { page, offsets, textareaRef, selectedSourceLine, selection, focusSourceLine } = paging;
  const { selectedToolpath } = useProgramSelection(currentRevisionReady ? preview : undefined, selectedSourceLine, gateway, source);
  const paged = offsets.length > PROGRAM_EDITOR_PAGE_LINES;
  const sourceLines = useMemo(() => page.text.split("\n"), [page.text]);
  const firstVisibleLine = Math.min(
    Math.max(0, sourceLines.length - 1),
    Math.max(0, Math.floor(scroll.top / EDITOR_LINE_HEIGHT) - EDITOR_OVERSCAN),
  );
  const visibleLineCount =
    Math.ceil(viewportHeight / EDITOR_LINE_HEIGHT) + EDITOR_OVERSCAN * 2;
  const lastVisibleLine = Math.min(
    sourceLines.length,
    firstVisibleLine + visibleLineCount,
  );
  const visibleLines = sourceLines.slice(firstVisibleLine, lastVisibleLine);

  useEffect(() => {
    const viewport = editorViewportRef.current;
    if (!viewport) return;
    const observer = new ResizeObserver(([entry]) => {
      setViewportHeight(entry.contentRect.height);
    });
    observer.observe(viewport);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const sequence = ++parseSequence.current;
    if (currentRevisionReady) return;
    setParseState("parsing");
    setParseError(undefined);
    const timer = window.setTimeout(() => {
      void gateway
        .parse(
          { sourceName: document.program.sourceName, source },
          { blockDelete },
        )
        .then((program) => {
          if (parseSequence.current !== sequence) return;
          setPreview(program);
          setPreviewSource(source);
          setPreviewBlockDelete(blockDelete);
          setPreviewGateway(gateway);
          setParseState("ready");
        })
        .catch((reason: unknown) => {
          if (parseSequence.current !== sequence) return;
          setParseState("invalid");
          setParseError(formatParseError(reason));
        });
    }, PARSE_DELAY_MS);
    return () => {
      window.clearTimeout(timer);
      parseSequence.current += 1;
    };
  }, [blockDelete, document.program.sourceName, gateway, source]);

  useEffect(() => setJumpLine(String(selectedSourceLine)), [selectedSourceLine]);

  useEffect(() => {
    if (!discardArmed) return;
    const timer = window.setTimeout(() => setDiscardArmed(false), 4_000);
    return () => window.clearTimeout(timer);
  }, [discardArmed]);

  const commit = (edit: TextEdit) => {
    const before = selection();
    setHistory((current) => commitProgramEditorSource(current, edit.source, edit.selection, before, edit.change));
    paging.select(edit.selection);
    setSaveNotice(undefined);
    const change = edit.change;
    const clearsHistory = change && (change.end - change.start + change.replacement.length) * 2 + 64 > PROGRAM_EDITOR_HISTORY_BYTES;
    setOperationError(clearsHistory ? "История отмены очищена: крупная правка" : undefined);
    setDiscardArmed(false);
  };

  const undo = () => {
    const next = undoProgramEditorSource(history);
    setHistory(next);
    paging.select(next.selection);
    setSaveNotice(undefined);
  };

  const redo = () => {
    const next = redoProgramEditorSource(history);
    setHistory(next);
    paging.select(next.selection);
    setSaveNotice(undefined);
  };

  const copy = async (cut: boolean) => {
    const selected = selectedTextOrLines(source, selection());
    try {
      await navigator.clipboard.writeText(selected.text);
      if (cut) commit(replaceTextSelection(source, selected.selection, ""));
    } catch (reason) {
      setOperationError(`Буфер обмена недоступен: ${formatParseError(reason)}`);
    }
  };

  const paste = async () => {
    try {
      const text = await navigator.clipboard.readText();
      commit(replaceTextSelection(source, selection(), text));
    } catch (reason) {
      setOperationError(`Буфер обмена недоступен: ${formatParseError(reason)}`);
    }
  };

  const save = async (transformed: boolean) => {
    if (!currentRevisionReady || saveBusy) return;
    setSaveBusy(true);
    setOperationError(undefined);
    setSaveNotice(undefined);
    try {
      const outcome = await saveProgramEditorDocument(gateway, { program: preview, source }, transformed);
      if (outcome) setSaveNotice(`Сохранено · ${outcome.bytesWritten} байт`);
    } catch (reason) {
      setOperationError(`Не удалось сохранить: ${formatParseError(reason)}`);
    } finally {
      setSaveBusy(false);
    }
  };

  const requestClose = () => {
    if (!dirty || discardArmed) {
      onClose();
      return;
    }
    setDiscardArmed(true);
  };

  const onEditorKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (paging.keyDown(event)) return;
    const modifier = event.metaKey || event.ctrlKey;
    if (modifier && event.key.toLowerCase() === "z") {
      event.preventDefault();
      if (event.shiftKey) redo();
      else undo();
    }
    if (modifier && event.key.toLowerCase() === "y") {
      event.preventDefault();
      redo();
    }
    if (modifier && event.key.toLowerCase() === "s") {
      event.preventDefault();
      void save(false);
    }
    if (event.key === "Escape") requestClose();
  };

  const onEditorScroll = (event: UIEvent<HTMLTextAreaElement>) => {
    setScroll({
      left: event.currentTarget.scrollLeft,
      top: event.currentTarget.scrollTop,
    });
  };

  return (
    <div className="program-editor-backdrop" role="presentation">
      <DialogSurface
        onDismiss={requestClose}
        aria-labelledby="program-editor-title"
        className={`program-editor${paged ? " is-paged-editor" : ""}`}
      >
        <header>
          <div>
            <span>Program Editor</span>
            <h2 id="program-editor-title">{document.program.sourceName}</h2>
          </div>
          <div
            className="program-editor-parse-state"
            data-state={parseState}
            role="status"
          >
            {parseState === "parsing" ? (
              <LoaderCircle
                aria-hidden="true"
                className="is-spinning"
                size={15}
              />
            ) : parseState === "ready" ? (
              <Check aria-hidden="true" size={15} />
            ) : (
              <TriangleAlert aria-hidden="true" size={15} />
            )}
            <span>
              {parseState === "parsing"
                ? "Обновляем preview"
                : parseState === "ready"
                  ? `${preview.summary.motionCount} движений · ${formatProgramDiagnostics(programDiagnosticsSummary(preview)) || "без замечаний"}`
                  : "Правка не разобрана"}
            </span>
          </div>
          <button
            aria-label="Закрыть редактор"
            onClick={requestClose}
            title="Закрыть редактор"
            type="button"
          >
            <X aria-hidden="true" size={18} />
          </button>
        </header>

        <div
          className="program-editor-toolbar"
          role="toolbar"
          aria-label="Редактирование G-code"
        >
          <div>
            <button
              aria-label="Отменить"
              disabled={history.past.length === 0}
              onClick={undo}
              title="Отменить"
              type="button"
            >
              <Undo2 aria-hidden="true" size={15} />
            </button>
            <button
              aria-label="Повторить"
              disabled={history.future.length === 0}
              onClick={redo}
              title="Повторить"
              type="button"
            >
              <Redo2 aria-hidden="true" size={15} />
            </button>
          </div>
          <div>
            <button
              aria-label="Копировать"
              onClick={() => void copy(false)}
              title="Копировать"
              type="button"
            >
              <Copy aria-hidden="true" size={15} />
            </button>
            <button
              aria-label="Вырезать"
              onClick={() => void copy(true)}
              title="Вырезать"
              type="button"
            >
              <Scissors aria-hidden="true" size={15} />
            </button>
            <button
              aria-label="Вставить"
              onClick={() => void paste()}
              title="Вставить"
              type="button"
            >
              <ClipboardPaste aria-hidden="true" size={15} />
            </button>
          </div>
          <div>
            <button
              aria-label="Вставить строку"
              onClick={() => commit(insertProgramLine(source, selection()))}
              title="Вставить строку"
              type="button"
            >
              <ListPlus aria-hidden="true" size={15} />
            </button>
            <button
              aria-label="Удалить выбранные строки"
              onClick={() => commit(deleteProgramLines(source, selection()))}
              title="Удалить выбранные строки"
              type="button"
            >
              <Trash2 aria-hidden="true" size={15} />
            </button>
          </div>
          <code>
            L{selectedSourceLine} · {offsets.length} строк
          </code>
        </div>

        <div className="program-editor-body">
          <div className={`program-editor-code${paged ? " is-paged" : ""}`} ref={editorViewportRef}>
            {paged && (
              <div className="program-editor-pages" role="group" aria-label="Страницы исходного кода">
                <button aria-label="Предыдущая страница" title="Предыдущая страница" type="button"
                  disabled={page.firstLine === 1} onClick={() => focusSourceLine(page.firstLine - 1)}>
                  <ChevronLeft aria-hidden="true" size={15} />
                </button>
                <code>L{page.firstLine}–{page.lastLine}</code>
                <button aria-label="Следующая страница" title="Следующая страница" type="button"
                  disabled={page.lastLine === offsets.length} onClick={() => focusSourceLine(page.lastLine + 1)}>
                  <ChevronRight aria-hidden="true" size={15} />
                </button>
                <input aria-label="Перейти к строке" type="number" min={1} max={offsets.length} step={1}
                  value={jumpLine} onChange={(event) => setJumpLine(event.target.value)}
                  onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); focusSourceLine(Number(jumpLine)); } }} />
                <button aria-label="Перейти к строке" title="Перейти к строке" type="button"
                  onClick={() => focusSourceLine(Number(jumpLine))}>
                  <CornerDownLeft aria-hidden="true" size={15} />
                </button>
              </div>
            )}
            <div
              aria-hidden="true"
              className="program-editor-gutter"
              style={{
                transform: `translateY(${firstVisibleLine * EDITOR_LINE_HEIGHT - scroll.top}px)`,
              }}
            >
              {visibleLines.map((_, index) => (
                <span key={firstVisibleLine + index}>
                  {page.firstLine + firstVisibleLine + index}
                </span>
              ))}
            </div>
            <pre
              aria-hidden="true"
              className="program-editor-highlight"
              style={{
                transform: `translate(${-scroll.left}px, ${firstVisibleLine * EDITOR_LINE_HEIGHT - scroll.top}px)`,
              }}
            >
              {visibleLines.map((line, index) => {
                const sourceLine = page.firstLine + firstVisibleLine + index;
                return (
                  <span
                    className={
                      sourceLine === selectedSourceLine
                        ? "is-current"
                        : undefined
                    }
                    key={sourceLine}
                  >
                    {tokenizeGcodeLine(line).map((token, tokenIndex) => (
                      <i
                        className={`syntax-${token.kind}`}
                        key={`${sourceLine}-${tokenIndex}`}
                      >
                        {token.text}
                      </i>
                    ))}
                    {"\n"}
                  </span>
                );
              })}
            </pre>
            <textarea
              aria-label="Исходный G-code"
              autoCapitalize="off"
              autoCorrect="off"
              onChange={(event) =>
                paging.change(event.currentTarget.value, {
                  start: event.currentTarget.selectionStart,
                  end: event.currentTarget.selectionEnd,
                  direction: event.currentTarget.selectionDirection,
                })
              }
              onClick={() => paging.updateCaret(true)}
              onKeyDown={onEditorKeyDown}
              onKeyUp={() => paging.updateCaret()}
              onScroll={onEditorScroll}
              onSelect={() => paging.updateCaret()}
              onCopy={(event) => {
                event.preventDefault();
                event.clipboardData.setData("text/plain", selectedTextOrLines(source, selection()).text);
              }}
              onCut={(event) => {
                event.preventDefault();
                const selected = selectedTextOrLines(source, selection());
                event.clipboardData.setData("text/plain", selected.text);
                commit(replaceTextSelection(source, selected.selection, ""));
              }}
              onPaste={(event) => {
                event.preventDefault();
                commit(replaceTextSelection(source, selection(), event.clipboardData.getData("text/plain")));
              }}
              ref={textareaRef}
              spellCheck={false}
              value={page.text}
              wrap="off"
            />
          </div>

          <aside
            className="program-editor-preview"
            aria-label="Preview текущей ревизии"
          >
            <div className="program-editor-scene">
              <Suspense
                fallback={
                  <div className="toolpath-preview is-loading">
                    Загрузка траектории...
                  </div>
                }
              >
                <ToolpathPreview
                  onSelectSourceLine={focusSourceLine}
                  program={preview}
                  selectedSourceLine={selectedSourceLine}
                  selectedToolpath={selectedToolpath}
                  toolVisualization={{
                    state: "removed",
                    showCutter: false,
                    spinning: false,
                  }}
                  view="iso"
                />
              </Suspense>
              {parseState === "invalid" && (
                <div className="program-editor-stale-preview">
                  <TriangleAlert aria-hidden="true" size={14} />
                  Preview последней корректной ревизии
                </div>
              )}
              {parseState !== "invalid" && preview.document?.previewSampled && (
                <div className="program-editor-stale-preview is-sampled" title="Показана выборка траектории; границы и выполнение относятся ко всей программе.">
                  Обзорная траектория
                </div>
              )}
            </div>
            <div className="program-editor-warnings">
              <div>
                <span>Диагностика parser</span>
                <strong>
                  {currentRevisionReady ? programDiagnosticsSummary(preview).totalCount : "--"}
                </strong>
              </div>
              {parseError ? (
                <p>{parseError}</p>
              ) : programDiagnosticsSummary(preview).totalCount === 0 ? (
                <p className="is-clear">Ошибок и предупреждений нет</p>
              ) : (
                preview.warnings.slice(0, 4).map((warning, index) => {
                  const presentation = programWarningPresentation(warning);
                  return (
                    <button
                      className={`is-${presentation.kind}`}
                      key={`${warning.sourceLine}-${warning.code}-${index}`}
                      onClick={() => focusSourceLine(warning.sourceLine)}
                      type="button"
                    >
                      <code>L{warning.sourceLine}</code>
                      <span>{presentation.detail}</span>
                    </button>
                  );
                })
              )}
            </div>
          </aside>
        </div>

        <footer>
          <div className="program-editor-save-actions">
            <button
              disabled={!gateway.save || !currentRevisionReady || saveBusy}
              onClick={() => void save(false)}
              title="Сохранить текущий текст G-code"
              type="button"
            >
              <Save aria-hidden="true" size={14} />
              Сохранить как
            </button>
            <button
              disabled={(!gateway.save && !gateway.saveProcessed) || !currentRevisionReady || saveBusy}
              onClick={() => void save(true)}
              title="Сохранить нормализованную копию с применённой политикой optional block"
              type="button"
            >
              <FileOutput aria-hidden="true" size={14} />
              Обработанная копия
            </button>
          </div>
          <div className="program-editor-footer-status" role="status">
            {operationError ??
              saveNotice ??
              (dirty ? "Есть неприменённые правки" : "Без изменений")}
          </div>
          <div className="program-editor-primary-actions">
            <button
              className={discardArmed ? "is-danger" : undefined}
              onClick={requestClose}
              type="button"
            >
              {discardArmed ? "Ещё раз: отменить" : "Отмена"}
            </button>
            <button
              className="is-primary"
              disabled={!dirty || !currentRevisionReady}
              onClick={() => onApply({ program: preview, source })}
              type="button"
            >
              <Check aria-hidden="true" size={14} />
              Применить к заданию
            </button>
          </div>
        </footer>
      </DialogSurface>
    </div>
  );
}
