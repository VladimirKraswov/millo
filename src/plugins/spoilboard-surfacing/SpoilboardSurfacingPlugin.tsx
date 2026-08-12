import {
  Check,
  Download,
  FolderOpen,
  Grid2X2,
  Layers3,
  Sparkles,
  X,
} from "lucide-react";
import {
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
} from "react";
import { createPortal } from "react-dom";

import type {
  PluginJobsCapability,
  PluginToolsCapability,
} from "../../plugin-sdk";
import type {
  GeneratedSurfacingJob,
  SurfacingJobSettings,
} from "../../shared/jobs";
import {
  supportsSurfacing,
  toolKindLabels,
} from "../../shared/tooling";
import { ToolKnowledgePanel } from "../../features/tool-library/ToolKnowledgePanel";
import { ToolSchematic } from "../../features/tool-library/ToolSchematic";

interface SpoilboardSurfacingPluginProps {
  readonly initialOpen?: boolean;
  readonly jobs: PluginJobsCapability;
  readonly tools: PluginToolsCapability;
}

const defaultSettings: SurfacingJobSettings = Object.freeze({
  originXMm: 0,
  originYMm: 0,
  widthMm: 100,
  heightMm: 100,
  edgeOverrunMm: 0,
  surfaceZMm: 0,
  removalMm: 0.2,
  depthPerPassMm: 0.1,
  safeZMm: 5,
  stepoverPercent: 45,
  feedMmPerMin: 800,
  plungeMmPerMin: 200,
  rasterAxis: "x",
});

export function SpoilboardSurfacingPlugin({
  initialOpen = false,
  jobs,
  tools,
}: SpoilboardSurfacingPluginProps) {
  const [open, setOpen] = useState(initialOpen);
  const toolLibrary = useSyncExternalStore(
    (notify) => tools.subscribe(() => notify()),
    tools.current,
    tools.current,
  );
  const compatibleTools = useMemo(
    () => [...toolLibrary.tools]
      .filter(supportsSurfacing)
      .sort((left, right) => {
        const kindOrder = Number(right.kind === "surfacing") - Number(left.kind === "surfacing");
        return kindOrder || right.diameterMm - left.diameterMm;
      }),
    [toolLibrary.tools],
  );
  const [toolId, setToolId] = useState<string>();
  const [settings, setSettings] = useState<SurfacingJobSettings>(defaultSettings);
  const [generated, setGenerated] = useState<GeneratedSurfacingJob>();
  const [busy, setBusy] = useState<"generate" | "save">();
  const [notice, setNotice] = useState<string>();
  const [error, setError] = useState<string>();
  const selectedTool = compatibleTools.find((tool) => tool.id === toolId);

  useEffect(() => {
    if (!selectedTool && compatibleTools[0]) setToolId(compatibleTools[0].id);
  }, [compatibleTools, selectedTool]);

  useEffect(() => {
    if (!selectedTool) return;
    setSettings((current) => ({
      ...current,
      depthPerPassMm: Math.min(current.removalMm, selectedTool.stepdownMm),
      feedMmPerMin: selectedTool.feedMmPerMin,
      plungeMmPerMin: selectedTool.plungeMmPerMin,
      stepoverPercent: selectedTool.stepoverPercent,
      edgeOverrunMm: Math.min(current.edgeOverrunMm, selectedTool.diameterMm / 2),
    }));
    setGenerated(undefined);
  }, [selectedTool]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) setOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [busy, open]);

  const updateNumber = (key: keyof SurfacingJobSettings, value: number) => {
    setSettings((current) => ({ ...current, [key]: value }));
    setGenerated(undefined);
    setNotice(undefined);
    setError(undefined);
  };
  const validation = validate(selectedTool?.diameterMm, settings);
  const generate = async () => {
    if (!selectedTool || validation || busy) return;
    setBusy("generate");
    setError(undefined);
    setNotice(undefined);
    try {
      const job = await jobs.generateSurfacing({
        sourceName: `surface-${number(settings.widthMm)}x${number(settings.heightMm)}.nc`,
        toolId: selectedTool.id,
        settings,
      });
      setGenerated(job);
      setNotice("Траектория создана и проверена ядром Millo");
    } catch (reason) {
      setError(readableError(reason));
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
      setError(readableError(reason));
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
      <button className="surfacing-launcher" onClick={() => setOpen(true)} type="button">
        <Layers3 aria-hidden="true" size={15} />
        <span>Выравнивание</span>
      </button>
      {open && createPortal(
        <div className="surfacing-backdrop">
          <section aria-labelledby="surfacing-title" aria-modal="true" className="surfacing-dialog" role="dialog">
            <header>
              <div><span>Системный плагин</span><h2 id="surfacing-title">Выравнивание поверхности</h2></div>
              <button aria-label="Закрыть" disabled={Boolean(busy)} onClick={() => setOpen(false)} title="Закрыть" type="button"><X aria-hidden="true" size={18} /></button>
            </header>

            <div className="surfacing-body">
              <section className="surfacing-preview">
                <div className={`surfacing-map is-${settings.rasterAxis}`}>
                  <div className="surfacing-map-area"><span className="surfacing-raster-lines" /></div>
                  <span className="surfacing-width">{number(settings.widthMm)} mm</span>
                  <span className="surfacing-height">{number(settings.heightMm)} mm</span>
                  <span className="surfacing-origin">X{number(settings.originXMm)} · Y{number(settings.originYMm)}</span>
                </div>
                <div className="surfacing-tool-card">
                  {selectedTool ? <ToolSchematic compact tool={selectedTool} /> : <Grid2X2 aria-hidden="true" size={24} />}
                  <div>
                    <span>Инструмент</span>
                    <select onChange={(event) => setToolId(event.target.value)} value={toolId ?? ""}>
                      {compatibleTools.map((tool) => <option key={tool.id} value={tool.id}>{tool.name}</option>)}
                    </select>
                    {selectedTool && <small>{toolKindLabels[selectedTool.kind]} · Ø {selectedTool.diameterMm} mm · {selectedTool.fluteCount} кромки</small>}
                  </div>
                </div>
                {selectedTool && <ToolKnowledgePanel tool={selectedTool} />}
              </section>

              <section className="surfacing-settings">
                <fieldset>
                  <legend>Область от рабочего нуля</legend>
                  <div className="surfacing-fields">
                    <NumberField label="Ширина X" max={100000} min={0.1} onChange={(value) => updateNumber("widthMm", value)} step={1} suffix="mm" value={settings.widthMm} />
                    <NumberField label="Высота Y" max={100000} min={0.1} onChange={(value) => updateNumber("heightMm", value)} step={1} suffix="mm" value={settings.heightMm} />
                    <NumberField label="Начало X" max={100000} min={-100000} onChange={(value) => updateNumber("originXMm", value)} step={1} suffix="mm" value={settings.originXMm} />
                    <NumberField label="Начало Y" max={100000} min={-100000} onChange={(value) => updateNumber("originYMm", value)} step={1} suffix="mm" value={settings.originYMm} />
                    <NumberField label="Выход за край" max={selectedTool ? selectedTool.diameterMm / 2 : 0} min={0} onChange={(value) => updateNumber("edgeOverrunMm", value)} step={0.1} suffix="mm" value={settings.edgeOverrunMm} />
                  </div>
                </fieldset>
                <fieldset>
                  <legend>Снятие поверхности</legend>
                  <div className="surfacing-fields">
                    <NumberField label="Снять всего" max={100} min={0.001} onChange={(value) => updateNumber("removalMm", value)} step={0.05} suffix="mm" value={settings.removalMm} />
                    <NumberField label="За проход" max={20} min={0.001} onChange={(value) => updateNumber("depthPerPassMm", value)} step={0.05} suffix="mm" value={settings.depthPerPassMm} />
                    <NumberField label="Безопасный Z" max={10000} min={-10000} onChange={(value) => updateNumber("safeZMm", value)} step={0.5} suffix="mm" value={settings.safeZMm} />
                    <NumberField label="Перекрытие" max={95} min={1} onChange={(value) => updateNumber("stepoverPercent", value)} step={1} suffix="% Ø" value={settings.stepoverPercent} />
                  </div>
                </fieldset>
                <fieldset>
                  <legend>Движение</legend>
                  <div className="surfacing-fields">
                    <NumberField label="Подача XY" max={100000} min={1} onChange={(value) => updateNumber("feedMmPerMin", value)} step={50} suffix="mm/min" value={settings.feedMmPerMin} />
                    <NumberField label="Подача Z" max={50000} min={1} onChange={(value) => updateNumber("plungeMmPerMin", value)} step={25} suffix="mm/min" value={settings.plungeMmPerMin} />
                  </div>
                  <div className="surfacing-axis" role="group" aria-label="Направление проходов">
                    <button aria-pressed={settings.rasterAxis === "x"} onClick={() => { setSettings((current) => ({ ...current, rasterAxis: "x" })); setGenerated(undefined); }} type="button">Вдоль X</button>
                    <button aria-pressed={settings.rasterAxis === "y"} onClick={() => { setSettings((current) => ({ ...current, rasterAxis: "y" })); setGenerated(undefined); }} type="button">Вдоль Y</button>
                  </div>
                </fieldset>
                <dl className="surfacing-estimate">
                  <div><dt>Шаг</dt><dd>{selectedTool ? number(selectedTool.diameterMm * settings.stepoverPercent / 100) : "—"} mm</dd></div>
                  <div><dt>Проходов Z</dt><dd>{settings.depthPerPassMm > 0 ? Math.ceil(settings.removalMm / settings.depthPerPassMm) : "—"}</dd></div>
                  <div><dt>Шпиндель</dt><dd>{selectedTool?.spindleRpm.toLocaleString("ru-RU") ?? "—"} rpm</dd></div>
                </dl>
                <div className={`surfacing-status${error || validation ? " is-error" : generated ? " is-ready" : ""}`} aria-live="polite">
                  {error ?? validation ?? (notice ? <><Check aria-hidden="true" size={14} />{notice}</> : "Шпиндель запускается вручную; G-code не содержит M3/M4.")}
                </div>
              </section>
            </div>

            <footer>
              <button disabled={!generated || Boolean(busy)} onClick={() => void save()} type="button"><Download aria-hidden="true" size={15} />{busy === "save" ? "Сохранение" : "Сохранить .nc"}</button>
              <div>
                <button className="surfacing-generate" disabled={Boolean(validation) || !selectedTool || Boolean(busy)} onClick={() => void generate()} type="button"><Sparkles aria-hidden="true" size={15} />{busy === "generate" ? "Расчёт" : generated ? "Пересчитать" : "Создать G-code"}</button>
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

function NumberField({ label, value, min, max, step, suffix, onChange }: { readonly label: string; readonly value: number; readonly min: number; readonly max: number; readonly step: number; readonly suffix: string; readonly onChange: (value: number) => void }) {
  return <label className="surfacing-field"><span>{label}</span><div><input max={max} min={min} onChange={(event) => onChange(Number(event.target.value))} step={step} type="number" value={value} /><small>{suffix}</small></div></label>;
}

const validate = (diameter: number | undefined, settings: SurfacingJobSettings): string | undefined => {
  if (!diameter) return "Добавьте плоскую или торцевую фрезу в библиотеку.";
  if (Object.values(settings).some((value) => typeof value === "number" && !Number.isFinite(value))) return "Заполните все числовые поля.";
  if (settings.widthMm < diameter || settings.heightMm < diameter) return `Область должна быть не меньше диаметра фрезы ${diameter} mm.`;
  if (settings.edgeOverrunMm > diameter / 2) return "Выход за край не может быть больше радиуса фрезы.";
  if (settings.depthPerPassMm > settings.removalMm) return "Съём за проход не может быть больше общего съёма.";
  if (settings.safeZMm <= settings.surfaceZMm) return "Безопасный Z должен быть выше поверхности.";
  return undefined;
};

const number = (value: number): string => Number.isFinite(value) ? Number(value.toFixed(3)).toString() : "—";
const readableError = (reason: unknown): string => String(reason).replace(/^Error:\s*/, "");
