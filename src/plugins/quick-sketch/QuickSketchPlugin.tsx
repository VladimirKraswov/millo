import {
  useEffect,
  useReducer,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import { createPortal } from "react-dom";
import {
  Circle,
  Download,
  FilePlus2,
  Focus,
  FolderOpen,
  Grid2X2,
  Hand,
  MousePointer2,
  Pentagon,
  Redo2,
  Ruler,
  Move,
  LockKeyhole,
  Link2,
  ListChecks,
  Save,
  Square,
  Undo2,
  X,
} from "lucide-react";
import {
  DialogSurface,
  type PluginJobsCapability,
  type PluginToolsCapability,
} from "../../plugin-sdk";
import type {
  GeneratedSketchJob,
  SketchGeometry,
  SketchJobRequest,
  SketchPoint,
  SketchShape,
} from "../../shared/sketch";
import { usePluginToolLibrary } from "../usePluginToolLibrary";
import { SketchCanvas } from "./SketchCanvas";
import { SketchInspector } from "./SketchInspector";
import { SketchStockPanel } from "./SketchStockPanel";
import { SketchAlignmentPanel } from "./SketchAlignmentPanel";
import {
  arrangeShapes,
  moveSketchShape,
  removeSketchShapes,
  resolveSketch,
} from "./sketchConstraints";
import {
  createShape,
  emptySketch,
  operationLabels,
  preferredTool,
  validateSketch,
  type DrawMode,
} from "./sketchModel";
import { sketchHistory } from "./sketchHistory";
import { decodeSketch, loadDraft, saveDraft } from "./sketchStorage";
import "./sketch.css";

interface Props {
  readonly jobs: PluginJobsCapability;
  readonly tools: PluginToolsCapability;
  readonly initialOpen?: boolean;
}
export function QuickSketchPlugin({ jobs, tools, initialOpen = false }: Props) {
  const [open, setOpen] = useState(initialOpen);
  const [history, dispatch] = useReducer(sketchHistory, undefined, () => ({
    past: [],
    present: loadDraft(),
    future: [],
  }));
  const doc = history.present;
  const [selection, setSelection] = useState<readonly string[]>([]);
  const [multiSelect, setMultiSelect] = useState(false);
  const selectedId = selection[selection.length - 1];
  const selectedShapes = doc.shapes.filter((s) => selection.includes(s.id));
  const select = (id?: string, additive = false) =>
    setSelection((current) =>
      !id
        ? []
        : additive || multiSelect
          ? current.includes(id)
            ? current.filter((value) => value !== id)
            : [...current, id]
          : [id],
    );
  const [mode, setMode] = useState<DrawMode>("select");
  const [dragEnabled, setDragEnabled] = useState(false);
  const [showDimensions, setShowDimensions] = useState(true);
  const [drawing, setDrawing] = useState(false);
  const [cancelDrawing, setCancelDrawing] = useState(0);
  const [grid, setGrid] = useState(1),
    [resetView, setResetView] = useState(0);
  const [newConfirm, setNewConfirm] = useState(false);
  const [generated, setGenerated] = useState<{
    job: GeneratedSketchJob;
    doc: SketchJobRequest;
    revision: number;
  }>();
  const [busy, setBusy] = useState<"generate" | "save" | "project">();
  const projectInput = useRef<HTMLInputElement>(null);
  const [notice, setNotice] = useState<string>();
  const [error, setError] = useState<string>();
  const library = usePluginToolLibrary(tools);
  const mounted = useRef(true);
  const current = useRef({ doc, revision: library.revision });
  current.current = { doc, revision: library.revision };
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  useEffect(() => {
    const timer = window.setTimeout(() => {
      if (!saveDraft(doc))
        setError("Не удалось сохранить черновик. Чертёж остаётся открыт");
    }, 400);
    return () => window.clearTimeout(timer);
  }, [doc]);
  const job =
    generated?.doc === doc && generated.revision === library.revision
      ? generated.job
      : undefined;
  const selected = doc.shapes.find((s) => s.id === selectedId);
  const validation = validateSketch(doc, library.tools);
  const edit = (document: SketchJobRequest) => {
    try {
      dispatch({ type: "edit", document: resolveSketch(document) });
      setNotice(undefined);
      setError(undefined);
    } catch (reason) {
      setError(String(reason).replace(/^Error:\s*/, ""));
    }
  };
  const updateShape = (shape: SketchShape) =>
    edit({
      ...doc,
      shapes: doc.shapes.map((s) => (s.id === shape.id ? shape : s)),
    });
  const add = (shapes: readonly SketchShape[]) => {
    if (doc.shapes.length + shapes.length > 200) {
      setError("В одном чертеже не больше 200 фигур");
      return;
    }
    edit({ ...doc, shapes: [...doc.shapes, ...shapes] });
    setSelection(shapes[0] ? [shapes[0].id] : []);
    setMultiSelect(false);
    setMode("select");
  };
  const create = (geometry: SketchGeometry, point: SketchPoint) =>
    add([
      createShape(
        geometry,
        point.x,
        point.y,
        preferredTool(library.tools),
        doc.shapes.length + 1,
      ),
    ]);
  const remove = () => {
    if (selection.length) {
      edit(removeSketchShapes(doc, selection));
      setSelection([]);
    }
  };
  const copies = (count: number, dx: number, dy: number) => {
    if (!selected) return;
    add(
      Array.from({ length: count }, (_, i) => ({
        ...selected,
        id: crypto.randomUUID(),
        name: `${selected.name} · ${i + 2}`,
        xMm: selected.xMm + dx * (i + 1),
        yMm: selected.yMm + dy * (i + 1),
        locked: false,
        constraints: {
          x: {
            referenceId: selected.id,
            referenceAnchor: "center",
            ownAnchor: "center",
            offsetMm: dx * (i + 1),
          },
          y: {
            referenceId: selected.id,
            referenceAnchor: "center",
            ownAnchor: "center",
            offsetMm: dy * (i + 1),
          },
        },
      })),
    );
  };
  const generate = async () => {
    if (busy || validation) return;
    setBusy("generate");
    setError(undefined);
    setNotice(undefined);
    const started = current.current;
    try {
      const result = await jobs.generateSketch(started.doc);
      if (!mounted.current) return;
      if (
        started.doc !== current.current.doc ||
        started.revision !== current.current.revision
      ) {
        setNotice("Чертёж изменился во время расчёта. Пересчитайте траекторию");
        return;
      }
      setGenerated({
        job: result,
        doc: started.doc,
        revision: started.revision,
      });
    } catch (reason) {
      if (mounted.current) setError(String(reason).replace(/^Error:\s*/, ""));
    } finally {
      if (mounted.current) setBusy(undefined);
    }
  };
  const save = async () => {
    if (!job || busy) return;
    setBusy("save");
    setError(undefined);
    try {
      const result = await jobs.save(job);
      if (mounted.current)
        setNotice(result ? `Сохранено: ${result.path}` : "Сохранение отменено");
    } catch (reason) {
      if (mounted.current) setError(String(reason));
    } finally {
      if (mounted.current) setBusy(undefined);
    }
  };
  const saveProject = async () => {
    if (busy) return;
    const snapshot = doc;
    setBusy("project");
    setError(undefined);
    try {
      const result = await jobs.saveSketchProject(snapshot);
      if (mounted.current)
        setNotice(
          result
            ? `Проект сохранён: ${result.path}${snapshot !== current.current.doc ? " · есть новые изменения" : ""}`
            : "Сохранение проекта отменено",
        );
    } catch (reason) {
      if (mounted.current) setError(String(reason));
    } finally {
      if (mounted.current) setBusy(undefined);
    }
  };
  const loadProject = async (file?: File) => {
    if (!file) return;
    const started = current.current.doc;
    try {
      if (file.size > 512_000) throw new Error("Проект больше 512 КБ");
      const document = decodeSketch(await file.text());
      if (!mounted.current) return;
      if (started !== current.current.doc)
        throw new Error(
          "Чертёж изменился во время загрузки. Откройте файл ещё раз",
        );
      edit(document);
      setSelection(document.shapes[0] ? [document.shapes[0].id] : []);
      setMultiSelect(false);
      setMode("select");
      setResetView((v) => v + 1);
      setNotice(`Проект открыт: ${file.name}`);
    } catch (reason) {
      if (mounted.current) setError(String(reason));
    }
  };
  const keyDown = (event: KeyboardEvent) => {
    if (
      (event.target as Element).closest(
        "input,select,textarea,[contenteditable=true]",
      )
    )
      return;
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "z") {
      event.preventDefault();
      dispatch({ type: event.shiftKey ? "redo" : "undo" });
    }
    if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      remove();
    }
  };
  const launch = () => {
    if (!job || busy) return;
    try {
      jobs.open(job);
      setOpen(false);
    } catch (reason) {
      setError(String(reason));
    }
  };
  return (
    <>
      <button
        className="sketch-launcher"
        onClick={() => setOpen(true)}
        type="button"
      >
        <Ruler size={16} />
        Чертёж и раскрой
      </button>
      {open &&
        createPortal(
          <div className="sketch-backdrop">
            <DialogSurface
              className="sketch-dialog"
              aria-labelledby="sketch-title"
              onDismiss={() =>
                drawing ? setCancelDrawing((v) => v + 1) : setOpen(false)
              }
              onKeyDown={keyDown}
            >
              <header className="sketch-header">
                <div>
                  <span>2D CAD / CAM</span>
                  <h2 id="sketch-title">Чертёж и раскрой</h2>
                </div>
                <input
                  className="sketch-name"
                  aria-label="Название проекта"
                  title="Название проекта"
                  value={doc.sourceName}
                  maxLength={100}
                  onChange={(e) => edit({ ...doc, sourceName: e.target.value })}
                />
                <button
                  title="Закрыть чертёж"
                  aria-label="Закрыть чертёж"
                  onClick={() => setOpen(false)}
                  type="button"
                >
                  <X size={20} />
                </button>
              </header>
              <div
                className="sketch-toolbar"
                role="toolbar"
                aria-label="Инструменты чертежа"
              >
                <input
                  ref={projectInput}
                  className="sketch-file-input"
                  type="file"
                  accept=".json,.millo-sketch"
                  aria-label="Файл проекта Millo"
                  onChange={(e) => {
                    void loadProject(e.target.files?.[0]);
                    e.currentTarget.value = "";
                  }}
                />
                <button
                  type="button"
                  title="Открыть проект"
                  aria-label="Открыть проект"
                  disabled={Boolean(busy)}
                  onClick={() => projectInput.current?.click()}
                >
                  <FolderOpen size={19} />
                </button>
                <button
                  type="button"
                  title="Сохранить проект"
                  aria-label="Сохранить проект"
                  disabled={Boolean(busy)}
                  onClick={() => void saveProject()}
                >
                  <Save size={19} />
                </button>
                <span className="sketch-toolbar-divider" />
                {(
                  [
                    ["select", "Выделение", MousePointer2],
                    ["pan", "Перемещение вида", Hand],
                    ["rectangle", "Прямоугольник: два угла", Square],
                    ["circle", "Круг: центр и радиус", Circle],
                    ["polygon", "Замкнутый контур", Pentagon],
                  ] as const
                ).map(([value, label, Icon]) => (
                  <button
                    key={value}
                    type="button"
                    title={label}
                    aria-label={label}
                    aria-pressed={mode === value}
                    onClick={() => setMode(value)}
                  >
                    <Icon size={19} />
                  </button>
                ))}
                <button
                  type="button"
                  title="Выбирать несколько фигур"
                  aria-label="Выбирать несколько фигур"
                  aria-pressed={multiSelect}
                  onClick={() => setMultiSelect((v) => !v)}
                >
                  <ListChecks size={19} />
                </button>
                <button
                  type="button"
                  title="Разрешить перетаскивание фигур"
                  aria-label="Разрешить перетаскивание фигур"
                  aria-pressed={dragEnabled}
                  onClick={() => setDragEnabled((v) => !v)}
                >
                  <Move size={19} />
                </button>
                <button
                  type="button"
                  title="Показать размеры"
                  aria-label="Показать размеры"
                  aria-pressed={showDimensions}
                  onClick={() => setShowDimensions((v) => !v)}
                >
                  <Ruler size={19} />
                </button>
                <span className="sketch-toolbar-divider" />
                <button
                  type="button"
                  title="Отменить изменение"
                  aria-label="Отменить изменение"
                  disabled={!history.past.length}
                  onClick={() => dispatch({ type: "undo" })}
                >
                  <Undo2 size={19} />
                </button>
                <button
                  type="button"
                  title="Повторить изменение"
                  aria-label="Повторить изменение"
                  disabled={!history.future.length}
                  onClick={() => dispatch({ type: "redo" })}
                >
                  <Redo2 size={19} />
                </button>
                <button
                  type="button"
                  title="Вписать лист"
                  aria-label="Вписать лист"
                  onClick={() => setResetView((v) => v + 1)}
                >
                  <Focus size={19} />
                </button>
                <label className="sketch-snap">
                  <Grid2X2 size={15} />
                  <select
                    aria-label="Привязка к сетке"
                    value={grid}
                    onChange={(e) => setGrid(Number(e.target.value))}
                  >
                    <option value={0}>Без привязки</option>
                    {[0.1, 0.5, 1, 5].map((v) => (
                      <option key={v} value={v}>
                        {v} мм
                      </option>
                    ))}
                  </select>
                </label>
                <button
                  type="button"
                  className="sketch-new-button"
                  title="Новый проект"
                  aria-label="Новый проект"
                  onClick={() =>
                    doc.shapes.length
                      ? setNewConfirm(true)
                      : edit(emptySketch())
                  }
                >
                  <FilePlus2 size={19} />
                </button>
              </div>
              {newConfirm && (
                <div className="sketch-new-confirm" role="alert">
                  <span>
                    Начать новый чертёж? Текущий останется в истории отмены.
                  </span>
                  <button
                    type="button"
                    onClick={() => {
                      edit(emptySketch());
                      setSelection([]);
                      setNewConfirm(false);
                    }}
                  >
                    Новый чертёж
                  </button>
                  <button type="button" onClick={() => setNewConfirm(false)}>
                    Отмена
                  </button>
                </div>
              )}
              <div className="sketch-body">
                <div className="sketch-workarea">
                  <SketchCanvas
                    document={doc}
                    selection={selection}
                    dragEnabled={dragEnabled && !multiSelect}
                    showDimensions={showDimensions}
                    mode={mode}
                    grid={grid}
                    resetView={resetView}
                    cancelDrawing={cancelDrawing}
                    onDrawingChange={setDrawing}
                    generated={job}
                    onSelect={select}
                    onCreate={create}
                    onMove={(id, p) => {
                      const s = doc.shapes.find((s) => s.id === id);
                      if (s) updateShape(moveSketchShape(s, p));
                    }}
                  />
                  <div
                    className="sketch-operations"
                    aria-label="Фигуры и операции"
                  >
                    {doc.shapes.map((s) => (
                      <button
                        key={s.id}
                        type="button"
                        className={`is-${s.operation.kind}`}
                        aria-pressed={selection.includes(s.id)}
                        onClick={(e) =>
                          select(s.id, e.shiftKey || e.metaKey || e.ctrlKey)
                        }
                      >
                        <span>
                          {s.locked && <LockKeyhole size={12} />}{" "}
                          {(s.constraints?.x || s.constraints?.y) && (
                            <Link2 size={12} />
                          )}{" "}
                          {s.name}
                        </span>
                        <small>
                          {operationLabels[s.operation.kind]} ·{" "}
                          {s.operation.through
                            ? "насквозь"
                            : `${s.operation.depthMm} мм`}
                        </small>
                      </button>
                    ))}
                  </div>
                </div>
                <aside className="sketch-sidebar">
                  <SketchStockPanel document={doc} onChange={edit} />
                  {selectedShapes.length > 1 ? (
                    <SketchAlignmentPanel
                      shapes={selectedShapes}
                      onAlign={(ref, axis, step) => {
                        try {
                          edit(arrangeShapes(doc, selection, ref, axis, step));
                        } catch (reason) {
                          setError(String(reason).replace(/^Error:\s*/, ""));
                        }
                      }}
                    />
                  ) : (
                    <SketchInspector
                      shape={selected}
                      document={doc}
                      tools={library.tools}
                      onChange={updateShape}
                      onDelete={remove}
                      onDuplicate={() => copies(1, 10, 10)}
                      onArray={copies}
                    />
                  )}
                </aside>
              </div>
              <div
                className={`sketch-feedback${error ? " is-error" : ""}`}
                aria-live="polite"
              >
                {busy === "project"
                  ? "Сохранение проекта…"
                  : busy === "save"
                    ? "Сохранение G-code…"
                    : busy === "generate"
                      ? "Расчёт траектории…"
                      : (error ??
                        notice ??
                        (job
                          ? `${job.summary.operations.length} операций · смен инструмента: ${job.summary.toolChangeCount} · траектория рассчитана`
                          : (validation ??
                            "Параметры реза из библиотеки инструмента. Проверьте режим для своего материала.")))}
                {job && (
                  <details>
                    <summary>
                      Порядок обработки
                      {job.summary.warnings.length
                        ? ` · ${job.summary.warnings.length} замечаний`
                        : ""}
                    </summary>
                    <ol>
                      {job.summary.operations.map((op) => (
                        <li key={op.shapeId}>
                          T{op.toolNumber} · {op.name} · Z −{op.depthMm} ·{" "}
                          {op.passCount} проходов
                        </li>
                      ))}
                    </ol>
                    {job.summary.warnings.map((warning, i) => (
                      <p key={i}>{warning}</p>
                    ))}
                    <p>
                      Безопасный Z должен быть выше креплений. Фреза должна
                      допускать осевое погружение. При ручном шпинделе
                      остановите его перед сменой инструмента.
                    </p>
                  </details>
                )}
              </div>
              <footer className="sketch-footer">
                <span>
                  {doc.shapes.length} фигур · {doc.stock.widthMm} ×{" "}
                  {doc.stock.heightMm} × {doc.stock.thicknessMm} мм
                </span>
                <button
                  type="button"
                  disabled={!job || Boolean(busy)}
                  onClick={() => void save()}
                >
                  <Download size={16} />
                  Сохранить .nc
                </button>
                <button
                  type="button"
                  disabled={Boolean(validation) || Boolean(busy)}
                  onClick={() => void generate()}
                >
                  <Ruler size={16} />
                  {busy === "generate"
                    ? "Расчёт…"
                    : job
                      ? "Пересчитать"
                      : "Создать G-code"}
                </button>
                <button
                  className="primary-action"
                  type="button"
                  disabled={!job || Boolean(busy)}
                  onClick={launch}
                >
                  <FolderOpen size={16} />
                  Открыть в задании
                </button>
              </footer>
            </DialogSurface>
          </div>,
          document.body,
        )}
    </>
  );
}
