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
  Fan,
  FilePlus2,
  Focus,
  FolderOpen,
  Grid2X2,
  Hand,
  MousePointer2,
  Pentagon,
  Redo2,
  Ruler,
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
import { SketchInspector, SketchNumber } from "./SketchInspector";
import {
  createShape,
  emptySketch,
  fanShapes,
  grilleShapes,
  operationLabels,
  preferredTool,
  validateSketch,
  type DrawMode,
  type FanTemplate,
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
  const [selectedId, setSelectedId] = useState<string>();
  const [mode, setMode] = useState<DrawMode>("select");
  const [drawing, setDrawing] = useState(false);
  const [cancelDrawing, setCancelDrawing] = useState(0);
  const [grid, setGrid] = useState(1),
    [resetView, setResetView] = useState(0);
  const [template, setTemplate] = useState(false);
  const [fan, setFan] = useState<FanTemplate>({
    opening: 70,
    pitch: 71.5,
    hole: 4.2,
    plate: 100,
  });
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
    dispatch({ type: "edit", document });
    setNotice(undefined);
    setError(undefined);
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
    setSelectedId(shapes[0]?.id);
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
    if (selected) {
      edit({ ...doc, shapes: doc.shapes.filter((s) => s.id !== selected.id) });
      setSelectedId(undefined);
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
      setSelectedId(document.shapes[0]?.id);
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
  const center = { x: doc.stock.widthMm / 2, y: doc.stock.heightMm / 2 };
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
                  aria-label="Название чертежа"
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
                    ["select", "Выделение и перемещение", MousePointer2],
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
                <div className="sketch-template-tools">
                  <button
                    type="button"
                    onClick={() => setTemplate((v) => !v)}
                    aria-expanded={template}
                  >
                    <Fan size={16} />
                    Вентилятор
                  </button>
                  <button
                    type="button"
                    onClick={() =>
                      add(grilleShapes(center, preferredTool(library.tools)))
                    }
                  >
                    <Grid2X2 size={16} />
                    Решётка
                  </button>
                  <button
                    type="button"
                    title="Новый чертёж"
                    aria-label="Новый чертёж"
                    onClick={() =>
                      doc.shapes.length
                        ? setNewConfirm(true)
                        : edit(emptySketch())
                    }
                  >
                    <FilePlus2 size={19} />
                  </button>
                </div>
              </div>
              {template && (
                <div className="sketch-template">
                  <SketchNumber
                    label="Диаметр воздуховода"
                    value={fan.opening}
                    min={5}
                    onChange={(opening) => setFan({ ...fan, opening })}
                  />
                  <SketchNumber
                    label="Межосевое креплений"
                    value={fan.pitch}
                    min={5}
                    onChange={(pitch) => setFan({ ...fan, pitch })}
                  />
                  <SketchNumber
                    label="Диаметр креплений"
                    value={fan.hole}
                    min={1}
                    onChange={(hole) => setFan({ ...fan, hole })}
                  />
                  <SketchNumber
                    label="Размер панели"
                    value={fan.plate}
                    min={10}
                    onChange={(plate) => setFan({ ...fan, plate })}
                  />
                  <button
                    type="button"
                    onClick={() => {
                      add(fanShapes(fan, center, preferredTool(library.tools)));
                      setTemplate(false);
                    }}
                  >
                    Добавить в чертёж
                  </button>
                </div>
              )}
              {newConfirm && (
                <div className="sketch-new-confirm" role="alert">
                  <span>
                    Начать новый чертёж? Текущий останется в истории отмены.
                  </span>
                  <button
                    type="button"
                    onClick={() => {
                      edit(emptySketch());
                      setSelectedId(undefined);
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
                    selectedId={selectedId}
                    mode={mode}
                    grid={grid}
                    resetView={resetView}
                    cancelDrawing={cancelDrawing}
                    onDrawingChange={setDrawing}
                    generated={job}
                    onSelect={setSelectedId}
                    onCreate={create}
                    onMove={(id, p) => {
                      const s = doc.shapes.find((s) => s.id === id);
                      if (s) updateShape({ ...s, xMm: p.x, yMm: p.y });
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
                        aria-pressed={s.id === selectedId}
                        onClick={() => setSelectedId(s.id)}
                      >
                        <span>{s.name}</span>
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
                  <section className="sketch-stock">
                    <h3>Заготовка</h3>
                    <div className="sketch-fields">
                      <SketchNumber
                        label="Ширина листа"
                        min={1}
                        value={doc.stock.widthMm}
                        onChange={(widthMm) =>
                          edit({ ...doc, stock: { ...doc.stock, widthMm } })
                        }
                      />
                      <SketchNumber
                        label="Высота листа"
                        min={1}
                        value={doc.stock.heightMm}
                        onChange={(heightMm) =>
                          edit({ ...doc, stock: { ...doc.stock, heightMm } })
                        }
                      />
                      <SketchNumber
                        label="Толщина листа"
                        min={0.05}
                        max={100}
                        value={doc.stock.thicknessMm}
                        onChange={(thicknessMm) =>
                          edit({ ...doc, stock: { ...doc.stock, thicknessMm } })
                        }
                      />
                      <SketchNumber
                        label="Безопасный Z"
                        min={0.5}
                        max={100}
                        value={doc.stock.safeZMm}
                        onChange={(safeZMm) =>
                          edit({ ...doc, stock: { ...doc.stock, safeZMm } })
                        }
                      />
                    </div>
                    <span className="sketch-datum">Z0 · верх материала</span>
                    <details>
                      <summary>Подложка и управление шпинделем</summary>
                      <SketchNumber
                        label="Выход в подложку"
                        value={doc.stock.breakthroughMm}
                        max={1}
                        onChange={(breakthroughMm) =>
                          edit({
                            ...doc,
                            stock: { ...doc.stock, breakthroughMm },
                          })
                        }
                      />
                      <label className="sketch-select">
                        <span>Шпиндель</span>
                        <select
                          aria-label="Управление шпинделем"
                          value={doc.stock.spindleMode}
                          onChange={(e) =>
                            edit({
                              ...doc,
                              stock: {
                                ...doc.stock,
                                spindleMode: e.target.value as
                                  | "manual"
                                  | "controller",
                              },
                            })
                          }
                        >
                          <option value="manual">Включаю вручную</option>
                          <option value="controller">
                            Управляется контроллером · M3/M5
                          </option>
                        </select>
                      </label>
                    </details>
                  </section>
                  <SketchInspector
                    shape={selected}
                    stock={doc.stock}
                    tools={library.tools}
                    onChange={updateShape}
                    onDelete={remove}
                    onDuplicate={() => copies(1, 10, 10)}
                    onArray={copies}
                  />
                </aside>
              </div>
              <div
                className={`sketch-feedback${error ? " is-error" : ""}`}
                aria-live="polite"
              >
                {error ??
                  notice ??
                  (job
                    ? `${job.summary.operations.length} операций · смен инструмента: ${job.summary.toolChangeCount} · траектория рассчитана`
                    : (validation ??
                      "Параметры реза из библиотеки инструмента. Проверьте режим для своего материала."))}
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
