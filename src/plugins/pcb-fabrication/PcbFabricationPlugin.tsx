import {
  Check,
  CircuitBoard,
  Download,
  FilePlus2,
  FolderOpen,
  Grip,
  FlipHorizontal2,
  RotateCw,
  Sparkles,
  Trash2,
  X,
} from "lucide-react";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { createPortal } from "react-dom";

import "./pcb.css";

import type { PluginJobsCapability, PluginToolsCapability } from "../../plugin-sdk";
import type {
  GeneratedPcbJob,
  PcbInspection,
  PcbJobSettings,
  PcbLayerRole,
  PcbTransform,
} from "../../shared/jobs";
import type { CuttingTool } from "../../shared/tooling";
import { PcbPreview } from "./PcbPreview";
import {
  closestTool,
  drillMappings,
  pcbRoleLabels,
  readPcbFiles,
  readablePcbError,
  type LocalPcbFile,
} from "./pcbModel";

interface PcbFabricationPluginProps {
  readonly initialOpen?: boolean;
  readonly jobs: PluginJobsCapability;
  readonly tools: PluginToolsCapability;
}

const defaultTransform: PcbTransform = Object.freeze({
  offsetXMm: 0,
  offsetYMm: 0,
  rotationQuarterTurns: 0,
  mirrorX: false,
});

const defaultSettings: PcbJobSettings = Object.freeze({
  safeZMm: 3,
  surfaceZMm: 0,
  isolation: Object.freeze({ enabled: true, toolId: "", depthMm: 0.08, clearanceMm: 0.05, passes: 1 }),
  drilling: Object.freeze({ enabled: false, depthMm: 1.8, mappings: Object.freeze([]) }),
  outline: Object.freeze({ enabled: false, toolId: "", depthMm: 1.7, depthPerPassMm: 0.4, tabCount: 4, tabWidthMm: 2, tabHeightMm: 0.4 }),
  marking: Object.freeze({ enabled: false, toolId: "", depthMm: 0.04 }),
});

export function PcbFabricationPlugin({
  initialOpen = false,
  jobs,
  tools,
}: PcbFabricationPluginProps) {
  const [open, setOpen] = useState(initialOpen);
  const inputRef = useRef<HTMLInputElement>(null);
  const revisionRef = useRef(0);
  const toolLibrary = useSyncExternalStore(
    (notify) => tools.subscribe(() => notify()),
    tools.current,
    tools.current,
  );
  const compatible = useMemo(() => toolLibrary.tools.filter((tool) => tool.kind !== "surfacing" && tool.kind !== "ballNose"), [toolLibrary.tools]);
  const engravingTools = useMemo(() => compatible
    .filter((tool) => tool.kind === "engraving" || tool.kind === "vBit")
    .sort((left, right) => Number(right.kind === "engraving") - Number(left.kind === "engraving") || left.diameterMm - right.diameterMm), [compatible]);
  const cuttingTools = useMemo(() => compatible.filter((tool) => tool.kind === "flatEndMill").sort((left, right) => left.diameterMm - right.diameterMm), [compatible]);
  const drillingTools = useMemo(() => [...compatible].sort((left, right) => Number(right.kind === "drill") - Number(left.kind === "drill") || left.diameterMm - right.diameterMm), [compatible]);
  const [files, setFiles] = useState<LocalPcbFile[]>([]);
  const [transform, setTransform] = useState<PcbTransform>(defaultTransform);
  const [settings, setSettings] = useState<PcbJobSettings>(defaultSettings);
  const [inspection, setInspection] = useState<PcbInspection>();
  const [generated, setGenerated] = useState<GeneratedPcbJob>();
  const [busy, setBusy] = useState<"inspect" | "generate" | "save">();
  const [dragging, setDragging] = useState(false);
  const [notice, setNotice] = useState<string>();
  const [error, setError] = useState<string>();

  useEffect(() => {
    if (!open || files.length === 0) {
      setInspection(undefined);
      return;
    }
    let active = true;
    const timer = window.setTimeout(() => {
      setBusy("inspect");
      setError(undefined);
      void jobs.inspectPcb({ files, transform }).then((result) => {
        if (!active) return;
        setInspection(result);
        setSettings((current) => ({
          ...current,
          drilling: {
            ...current.drilling,
            mappings: [...drillMappings(result.drillGroups, drillingTools, new Map(current.drilling.mappings.map((mapping) => [mapping.groupKey, mapping.toolId])))].map(([groupKey, toolId]) => ({ groupKey, toolId })),
          },
        }));
      }).catch((reason) => {
        if (active) {
          setInspection(undefined);
          setError(readablePcbError(reason));
        }
      }).finally(() => {
        if (active) setBusy(undefined);
      });
    }, 180);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [drillingTools, files, jobs, open, transform]);

  useEffect(() => {
    const engraving = engravingTools[0] ?? compatible[0];
    const outline = cuttingTools[0] ?? compatible[0];
    setSettings((current) => ({
      ...current,
      isolation: { ...current.isolation, toolId: validTool(current.isolation.toolId, compatible)?.id ?? engraving?.id ?? "" },
      outline: { ...current.outline, toolId: validTool(current.outline.toolId, compatible)?.id ?? outline?.id ?? "" },
      marking: { ...current.marking, toolId: validTool(current.marking.toolId, compatible)?.id ?? engraving?.id ?? "" },
    }));
    revisionRef.current += 1;
    setGenerated(undefined);
  }, [compatible, cuttingTools, engravingTools]);

  useEffect(() => {
    if (!open) return;
    const close = (event: KeyboardEvent) => {
      if (event.key === "Escape" && busy !== "generate" && busy !== "save") setOpen(false);
    };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, [busy, open]);

  const invalidate = () => {
    revisionRef.current += 1;
    setGenerated(undefined);
    setNotice(undefined);
    setError(undefined);
  };
  const addFiles = async (selected: FileList | readonly File[]) => {
    try {
      const next = await readPcbFiles(selected);
      setFiles((current) => {
        const byName = new Map(current.map((file) => [file.sourceName, file]));
        next.forEach((file) => byName.set(file.sourceName, file));
        return [...byName.values()];
      });
      setSettings((current) => ({
        ...current,
        isolation: { ...current.isolation, enabled: current.isolation.enabled || next.some((file) => file.role === "copper") },
        drilling: { ...current.drilling, enabled: current.drilling.enabled || next.some((file) => file.role === "drill") },
        outline: { ...current.outline, enabled: current.outline.enabled || next.some((file) => file.role === "outline") },
        marking: { ...current.marking, enabled: current.marking.enabled || next.some((file) => file.role === "marking") },
      }));
      invalidate();
    } catch (reason) {
      setError(readablePcbError(reason));
    }
  };
  const updateRole = (sourceName: string, role: PcbLayerRole) => {
    setFiles((current) => current.map((file) => file.sourceName === sourceName ? { ...file, role } : file));
    invalidate();
  };
  const updateTransform = (patch: Partial<PcbTransform>) => {
    setTransform((current) => ({ ...current, ...patch }));
    invalidate();
  };
  const updateSettings = (next: PcbJobSettings) => {
    setSettings(next);
    invalidate();
  };
  const validation = validate(files, inspection, settings, compatible);

  const generate = async () => {
    if (validation || busy) return;
    const requestedRevision = revisionRef.current;
    setBusy("generate");
    setError(undefined);
    try {
      const result = await jobs.generatePcb({
        sourceName: `${files[0]?.sourceName.replace(/\.[^.]+$/, "") || "board"}-pcb.nc`,
        board: { files, transform },
        settings,
      });
      if (requestedRevision !== revisionRef.current) {
        setNotice("Параметры изменились во время расчёта. Пересчитайте G-code");
        return;
      }
      setGenerated(result);
      setNotice("G-code рассчитан и проверен ядром Millo");
    } catch (reason) {
      setError(readablePcbError(reason));
    } finally {
      setBusy(undefined);
    }
  };
  const save = async () => {
    if (!generated || busy) return;
    setBusy("save");
    setError(undefined);
    try {
      const outcome = await jobs.save(generated);
      setNotice(outcome ? `Сохранено: ${outcome.path}` : "Сохранение отменено");
    } catch (reason) {
      setError(readablePcbError(reason));
    } finally {
      setBusy(undefined);
    }
  };
  const openJob = () => {
    if (!generated || busy) return;
    jobs.open(generated);
    setOpen(false);
  };

  return (
    <>
      <button className="pcb-launcher" onClick={() => setOpen(true)} type="button">
        <CircuitBoard aria-hidden="true" size={15} /><span>Плата из Gerber</span>
      </button>
      {open && createPortal(
        <div className="pcb-backdrop">
          <section aria-labelledby="pcb-title" aria-modal="true" className="pcb-dialog" role="dialog">
            <header>
              <div><span>Gerber / Excellon</span><h2 id="pcb-title">Печатная плата</h2></div>
              <button aria-label="Закрыть" disabled={busy === "generate" || busy === "save"} onClick={() => setOpen(false)} title="Закрыть" type="button"><X aria-hidden="true" size={18} /></button>
            </header>

            <div
              className={`pcb-body${dragging ? " is-dragging" : ""}`}
              onDragEnter={(event) => { event.preventDefault(); setDragging(true); }}
              onDragLeave={(event) => { if (event.currentTarget === event.target) setDragging(false); }}
              onDragOver={(event) => event.preventDefault()}
              onDrop={(event) => { event.preventDefault(); setDragging(false); void addFiles(event.dataTransfer.files); }}
            >
              <section className="pcb-stage">
                <div className="pcb-preview-shell">
                  <PcbPreview
                    inspection={inspection}
                    onMove={(x, y) => updateTransform({ offsetXMm: transform.offsetXMm + x, offsetYMm: transform.offsetYMm + y })}
                  />
                  <div className="pcb-stage-toolbar">
                    <span>{inspection ? `${format(inspection.bounds.widthMm)} × ${format(inspection.bounds.heightMm)} mm` : busy === "inspect" ? "Чтение слоёв" : "Перетащите Gerber и Excellon"}</span>
                    {inspection && <div>
                      <button aria-label="Повернуть на 90 градусов" onClick={() => updateTransform({ rotationQuarterTurns: (transform.rotationQuarterTurns + 1) % 4 })} title="Повернуть на 90°" type="button"><RotateCw aria-hidden="true" size={15} /></button>
                      <button aria-label="Отразить по X" aria-pressed={transform.mirrorX} onClick={() => updateTransform({ mirrorX: !transform.mirrorX })} title="Отразить по X" type="button"><FlipHorizontal2 aria-hidden="true" size={15} /></button>
                    </div>}
                  </div>
                </div>
                {inspection && <div className="pcb-placement">
                  <Grip aria-hidden="true" size={14} /><span>Тяните плату в preview или задайте позицию</span>
                  <NumberField label="X" onChange={(value) => updateTransform({ offsetXMm: value })} step={0.1} value={transform.offsetXMm} />
                  <NumberField label="Y" onChange={(value) => updateTransform({ offsetYMm: value })} step={0.1} value={transform.offsetYMm} />
                </div>}
              </section>

              <aside className="pcb-panel">
                <section className="pcb-files">
                  <div className="pcb-section-heading"><div><span>Файлы</span><small>{files.length ? `${files.length} сл.` : "Gerber + Excellon"}</small></div><button onClick={() => inputRef.current?.click()} type="button"><FilePlus2 aria-hidden="true" size={14} />Добавить</button></div>
                  <input accept=".art,.drl,.gbl,.gbo,.gbr,.gko,.gm1,.gml,.gto,.gtl,.txt,.xln" hidden multiple onChange={(event) => { if (event.target.files) void addFiles(event.target.files); event.target.value = ""; }} ref={inputRef} type="file" />
                  {files.length === 0 ? (
                    <button className="pcb-drop-action" onClick={() => inputRef.current?.click()} type="button"><CircuitBoard aria-hidden="true" size={22} /><strong>Выбрать файлы платы</strong><small>.gbr · .gtl · .gko · .drl</small></button>
                  ) : files.map((file) => (
                    <div className="pcb-file-row" key={file.sourceName}>
                      <div><strong title={file.sourceName}>{file.sourceName}</strong><small>{formatBytes(file.sizeBytes)}</small></div>
                      <select aria-label={`Роль ${file.sourceName}`} onChange={(event) => updateRole(file.sourceName, event.target.value as PcbLayerRole)} value={file.role}>{Object.entries(pcbRoleLabels).map(([role, label]) => <option key={role} value={role}>{label}</option>)}</select>
                      <button aria-label={`Удалить ${file.sourceName}`} onClick={() => { setFiles((current) => current.filter((item) => item.sourceName !== file.sourceName)); invalidate(); }} title="Удалить слой" type="button"><Trash2 aria-hidden="true" size={14} /></button>
                    </div>
                  ))}
                </section>

                {files.length > 0 && <section className="pcb-operations">
                  <div className="pcb-section-heading"><div><span>Операции</span><small>в порядке выполнения</small></div></div>
                  <OperationRow enabled={settings.isolation.enabled} label="Изоляция дорожек" onToggle={(enabled) => updateSettings({ ...settings, isolation: { ...settings.isolation, enabled } })} summary={`${settings.isolation.passes} пр. · Z −${format(settings.isolation.depthMm)}`}>
                    <ToolSelect label="Инструмент" onChange={(toolId) => updateSettings({ ...settings, isolation: { ...settings.isolation, toolId } })} tools={engravingTools.length ? engravingTools : compatible} value={settings.isolation.toolId} />
                    <NumberField label="Глубина" min={0.001} onChange={(depthMm) => updateSettings({ ...settings, isolation: { ...settings.isolation, depthMm } })} step={0.01} value={settings.isolation.depthMm} />
                    <NumberField label="Зазор" min={0} onChange={(clearanceMm) => updateSettings({ ...settings, isolation: { ...settings.isolation, clearanceMm } })} step={0.01} value={settings.isolation.clearanceMm} />
                    <NumberField label="Проходы" max={10} min={1} onChange={(passes) => updateSettings({ ...settings, isolation: { ...settings.isolation, passes: Math.round(passes) } })} step={1} suffix="шт." value={settings.isolation.passes} />
                  </OperationRow>

                  <OperationRow enabled={settings.drilling.enabled} label="Сверловка" onToggle={(enabled) => updateSettings({ ...settings, drilling: { ...settings.drilling, enabled } })} summary={`${inspection?.drillHits.length ?? 0} отв. · Z −${format(settings.drilling.depthMm)}`}>
                    <NumberField label="Глубина" min={0.001} onChange={(depthMm) => updateSettings({ ...settings, drilling: { ...settings.drilling, depthMm } })} step={0.1} value={settings.drilling.depthMm} />
                    {inspection?.drillGroups.map((group) => (
                      <ToolSelect key={group.key} label={`T${group.sourceToolNumber} · Ø${format(group.diameterMm)} · ${group.hitCount} шт.`} onChange={(toolId) => updateSettings({ ...settings, drilling: { ...settings.drilling, mappings: settings.drilling.mappings.map((mapping) => mapping.groupKey === group.key ? { ...mapping, toolId } : mapping) } })} tools={drillingTools} value={settings.drilling.mappings.find((mapping) => mapping.groupKey === group.key)?.toolId ?? closestTool(drillingTools, group.diameterMm)?.id ?? ""} />
                    ))}
                  </OperationRow>

                  <OperationRow enabled={settings.marking.enabled} label="Маркировка" onToggle={(enabled) => updateSettings({ ...settings, marking: { ...settings.marking, enabled } })} summary={`Z −${format(settings.marking.depthMm)}`}>
                    <ToolSelect label="Инструмент" onChange={(toolId) => updateSettings({ ...settings, marking: { ...settings.marking, toolId } })} tools={engravingTools.length ? engravingTools : compatible} value={settings.marking.toolId} />
                    <NumberField label="Глубина" min={0.001} onChange={(depthMm) => updateSettings({ ...settings, marking: { ...settings.marking, depthMm } })} step={0.01} value={settings.marking.depthMm} />
                  </OperationRow>

                  <OperationRow enabled={settings.outline.enabled} label="Вырезать плату" onToggle={(enabled) => updateSettings({ ...settings, outline: { ...settings.outline, enabled } })} summary={`${settings.outline.tabCount} перем. · Z −${format(settings.outline.depthMm)}`}>
                    <ToolSelect label="Инструмент" onChange={(toolId) => updateSettings({ ...settings, outline: { ...settings.outline, toolId } })} tools={cuttingTools.length ? cuttingTools : compatible} value={settings.outline.toolId} />
                    <NumberField label="Глубина" min={0.001} onChange={(depthMm) => updateSettings({ ...settings, outline: { ...settings.outline, depthMm } })} step={0.1} value={settings.outline.depthMm} />
                    <NumberField label="За проход" min={0.001} onChange={(depthPerPassMm) => updateSettings({ ...settings, outline: { ...settings.outline, depthPerPassMm } })} step={0.1} value={settings.outline.depthPerPassMm} />
                    <NumberField label="Перемычки" max={16} min={0} onChange={(tabCount) => updateSettings({ ...settings, outline: { ...settings.outline, tabCount: Math.round(tabCount) } })} step={1} suffix="шт." value={settings.outline.tabCount} />
                  </OperationRow>
                </section>}

                {files.length > 0 && <details className="pcb-machine-settings">
                  <summary>Общие высоты</summary>
                  <div><NumberField label="Безопасный Z" onChange={(safeZMm) => updateSettings({ ...settings, safeZMm })} step={0.5} value={settings.safeZMm} /><NumberField label="Поверхность Z" onChange={(surfaceZMm) => updateSettings({ ...settings, surfaceZMm })} step={0.1} value={settings.surfaceZMm} /></div>
                </details>}
                <div aria-live="polite" className={`pcb-status${error || validation ? " is-error" : generated ? " is-ready" : ""}`}>
                  {error ?? validation ?? (notice ? <><Check aria-hidden="true" size={14} />{notice}</> : inspection?.warnings[0] ?? "Шпиндель запускается вручную; смену инструмента проведёт sender")}
                </div>
              </aside>
            </div>

            <footer>
              <div className="pcb-result">{generated ? <><strong>{generated.program.lines.length} строк</strong><span>{generated.summary.operations.length} операций · {generated.summary.toolChangeCount} инструментов</span></> : <><strong>{inspection ? `${inspection.drillHits.length} отверстий` : "Нет расчёта"}</strong><span>{inspection ? `${inspection.paths.length} контуров` : "Загрузите плату"}</span></>}</div>
              <div>
                <button disabled={!generated || Boolean(busy)} onClick={() => void save()} type="button"><Download aria-hidden="true" size={15} />{busy === "save" ? "Сохранение" : "Сохранить .nc"}</button>
                <button className="pcb-generate" disabled={Boolean(validation) || Boolean(busy)} onClick={() => void generate()} type="button"><Sparkles aria-hidden="true" size={15} />{busy === "generate" ? "Расчёт" : generated ? "Пересчитать" : "Создать G-code"}</button>
                <button className="primary-action" disabled={!generated || Boolean(busy)} onClick={openJob} type="button"><FolderOpen aria-hidden="true" size={15} />Открыть в задании</button>
              </div>
            </footer>
          </section>
        </div>,
        document.body,
      )}
    </>
  );
}

function OperationRow({ children, enabled, label, onToggle, summary }: { readonly children: React.ReactNode; readonly enabled: boolean; readonly label: string; readonly onToggle: (value: boolean) => void; readonly summary: string }) {
  return <fieldset className={enabled ? "is-enabled" : ""}><label className="pcb-operation-toggle"><input checked={enabled} onChange={(event) => onToggle(event.target.checked)} type="checkbox" /><span><strong>{label}</strong><small>{summary}</small></span></label>{enabled && <div className="pcb-operation-fields">{children}</div>}</fieldset>;
}

function ToolSelect({ label, onChange, tools, value }: { readonly label: string; readonly onChange: (value: string) => void; readonly tools: readonly CuttingTool[]; readonly value: string }) {
  return <label className="pcb-field pcb-tool-select"><span>{label}</span><select onChange={(event) => onChange(event.target.value)} value={value}><option value="">Выберите</option>{tools.map((tool) => <option key={tool.id} value={tool.id}>{tool.name} · Ø{format(tool.diameterMm)}</option>)}</select></label>;
}

function NumberField({ label, max = 100_000, min = -100_000, onChange, step, suffix = "mm", value }: { readonly label: string; readonly max?: number; readonly min?: number; readonly onChange: (value: number) => void; readonly step: number; readonly suffix?: string; readonly value: number }) {
  return <label className="pcb-field pcb-number"><span>{label}</span><div><input max={max} min={min} onChange={(event) => { if (Number.isFinite(event.target.valueAsNumber)) onChange(event.target.valueAsNumber); }} step={step} type="number" value={value} /><small>{suffix}</small></div></label>;
}

function validate(files: readonly LocalPcbFile[], inspection: PcbInspection | undefined, settings: PcbJobSettings, tools: readonly CuttingTool[]): string | undefined {
  if (files.length === 0) return "Добавьте Gerber или Excellon";
  if (!inspection) return "Дождитесь разбора слоёв";
  if (!settings.isolation.enabled && !settings.drilling.enabled && !settings.outline.enabled && !settings.marking.enabled) return "Включите хотя бы одну операцию";
  const ids = new Set(tools.map((tool) => tool.id));
  if (settings.isolation.enabled && !ids.has(settings.isolation.toolId)) return "Выберите инструмент для изоляции";
  if (settings.drilling.enabled && (
    inspection.drillGroups.length === 0
    || inspection.drillGroups.some((group) => !ids.has(settings.drilling.mappings.find((mapping) => mapping.groupKey === group.key)?.toolId ?? ""))
  )) return "Выберите инструмент для каждой группы отверстий";
  if (settings.outline.enabled && !ids.has(settings.outline.toolId)) return "Выберите инструмент для контура";
  if (settings.marking.enabled && !ids.has(settings.marking.toolId)) return "Выберите инструмент для маркировки";
  if (settings.safeZMm <= settings.surfaceZMm) return "Безопасный Z должен быть выше поверхности";
  return undefined;
}

const validTool = (id: string, tools: readonly CuttingTool[]) => tools.find((tool) => tool.id === id);
const format = (value: number) => Number(value.toFixed(3)).toString();
const formatBytes = (bytes: number) => bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KB`;
