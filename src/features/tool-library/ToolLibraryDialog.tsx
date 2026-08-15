import {
  BookOpen,
  Plus,
  RotateCcw,
  Save,
  Trash2,
  X,
} from "lucide-react";
import {
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
  type ChangeEvent,
} from "react";
import { createPortal } from "react-dom";

import type { ToolLibraryService } from "../../platform/tooling/ToolLibraryService";
import {
  draftFromTool,
  newToolDraft,
  toolKindLabels,
  type CuttingTool,
  type CuttingToolDraft,
  type ToolKind,
} from "../../shared/tooling";
import { ToolKnowledgePanel } from "./ToolKnowledgePanel";
import { ToolSchematic } from "./ToolSchematic";

interface ToolLibraryDialogProps {
  readonly open: boolean;
  readonly onClose: () => void;
  readonly service: ToolLibraryService;
}

const toolKinds = Object.keys(toolKindLabels) as ToolKind[];

export function ToolLibraryDialog({ open, onClose, service }: ToolLibraryDialogProps) {
  const library = useSyncExternalStore(service.subscribe, service.current, service.current);
  const [selectedId, setSelectedId] = useState<string>();
  const [draft, setDraft] = useState<CuttingToolDraft>(newToolDraft());
  const [creating, setCreating] = useState(false);
  const [busy, setBusy] = useState(false);
  const [deleteConfirming, setDeleteConfirming] = useState(false);
  const [status, setStatus] = useState<string>();
  const selected = useMemo(
    () => library.tools.find((tool) => tool.id === selectedId),
    [library.tools, selectedId],
  );

  useEffect(() => {
    if (!open) return;
    if (!selected && library.tools[0]) setSelectedId(library.tools[0].id);
  }, [library.tools, open, selected]);

  useEffect(() => {
    if (!selected || creating) return;
    setDraft(draftFromTool(selected));
    setDeleteConfirming(false);
    setStatus(undefined);
  }, [creating, selected]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [busy, onClose, open]);

  if (!open) return null;

  const select = (tool: CuttingTool) => {
    if (busy) return;
    setCreating(false);
    setSelectedId(tool.id);
  };
  const startCreate = () => {
    setCreating(true);
    setSelectedId(undefined);
    setDraft(newToolDraft());
    setDeleteConfirming(false);
    setStatus(undefined);
  };
  const mutateDraft = <K extends keyof CuttingToolDraft>(
    key: K,
    value: CuttingToolDraft[K],
  ) => setDraft((current) => ({ ...current, [key]: value }));
  const updateKind = (kind: ToolKind) => setDraft((current) => ({
    ...current,
    kind,
    tipDiameterMm:
      kind === "vBit" || kind === "engraving"
        ? (current.tipDiameterMm ?? Math.min(current.diameterMm, 0.1))
        : undefined,
    includedAngleDegrees:
      kind === "vBit" || kind === "engraving"
        ? (current.includedAngleDegrees ?? (kind === "engraving" ? 30 : 60))
        : undefined,
  }));
  const save = async () => {
    if (busy) return;
    setBusy(true);
    setStatus(undefined);
    try {
      const next = creating
        ? await service.create(draft)
        : selected
          ? await service.update(selected.id, draft)
          : undefined;
      const saved = next?.tools.find((tool) => tool.name === draft.name.trim());
      if (saved) setSelectedId(saved.id);
      setCreating(false);
      setStatus("Сохранено в библиотеке");
    } catch (reason) {
      setStatus(readableError(reason));
    } finally {
      setBusy(false);
    }
  };
  const remove = async () => {
    if (!selected || busy) return;
    if (!deleteConfirming) {
      setDeleteConfirming(true);
      return;
    }
    setBusy(true);
    try {
      const next = await service.delete(selected.id);
      setSelectedId(next.tools[0]?.id);
      setDeleteConfirming(false);
      setStatus("Инструмент удалён");
    } catch (reason) {
      setStatus(readableError(reason));
    } finally {
      setBusy(false);
    }
  };
  const restore = async () => {
    if (busy) return;
    setBusy(true);
    try {
      await service.restorePresets();
      setStatus("Отсутствующие пресеты восстановлены");
    } catch (reason) {
      setStatus(readableError(reason));
    } finally {
      setBusy(false);
    }
  };

  return createPortal(
    <div className="tool-library-backdrop">
      <section aria-labelledby="tool-library-title" aria-modal="true" className="tool-library-dialog" role="dialog">
        <header>
          <div>
            <span>Оснастка</span>
            <h2 id="tool-library-title">Библиотека инструментов</h2>
          </div>
          <button aria-label="Закрыть" disabled={busy} onClick={onClose} title="Закрыть" type="button">
            <X aria-hidden="true" size={18} />
          </button>
        </header>

        <div className="tool-library-body">
          <aside className="tool-library-list">
            <div className="tool-library-list-actions">
              <button onClick={startCreate} type="button"><Plus aria-hidden="true" size={14} />Добавить</button>
              <button aria-label="Восстановить стандартные инструменты" onClick={() => void restore()} title="Вернуть отсутствующие пресеты" type="button">
                <RotateCcw aria-hidden="true" size={14} />
              </button>
            </div>
            <div className="tool-library-scroll">
              {library.tools.map((tool) => (
                <button
                  aria-pressed={!creating && tool.id === selectedId}
                  className="tool-library-item"
                  key={tool.id}
                  onClick={() => select(tool)}
                  type="button"
                >
                  <ToolSchematic compact tool={tool} />
                  <span><strong>{tool.name}</strong><small>{toolKindLabels[tool.kind]} · {tool.tipDiameterMm !== undefined ? `кончик ${tool.tipDiameterMm} mm` : `Ø ${tool.diameterMm} mm`}</small></span>
                  {tool.factoryPreset && <em>preset</em>}
                </button>
              ))}
            </div>
          </aside>

          <main className="tool-library-editor">
            <div className="tool-editor-heading">
              <ToolSchematic tool={draft} />
              <div><span>{creating ? "Новый инструмент" : toolKindLabels[draft.kind]}</span><strong>{draft.name}</strong></div>
            </div>
            <div className="tool-editor-scroll">
              <section className="tool-form-section">
                <h3>Инструмент</h3>
                <div className="tool-form-grid">
                  <TextField label="Название" onChange={(value) => mutateDraft("name", value)} value={draft.name} wide />
                  <label className="tool-field"><span>Геометрия</span><select onChange={(event) => updateKind(event.target.value as ToolKind)} value={draft.kind}>{toolKinds.map((kind) => <option key={kind} value={kind}>{toolKindLabels[kind]}</option>)}</select></label>
                  <NumberField label={draft.tipDiameterMm !== undefined ? "Макс. диаметр" : "Диаметр"} max={500} min={0.01} onChange={(value) => mutateDraft("diameterMm", value)} step={0.001} suffix="mm" value={draft.diameterMm} />
                  {(draft.kind === "vBit" || draft.kind === "engraving") && <NumberField label="Диаметр кончика" max={draft.diameterMm} min={0.001} onChange={(value) => mutateDraft("tipDiameterMm", value)} step={0.01} suffix="mm" value={draft.tipDiameterMm ?? 0.1} />}
                  <NumberField label="Хвостовик" max={100} min={0.1} onChange={(value) => mutateDraft("shankDiameterMm", value)} step={0.001} suffix="mm" value={draft.shankDiameterMm} />
                  <NumberField label="Режущая длина" max={1000} min={0.1} onChange={(value) => mutateDraft("cuttingLengthMm", value)} step={0.1} suffix="mm" value={draft.cuttingLengthMm} />
                  <NumberField label="Режущие кромки" max={20} min={1} onChange={(value) => mutateDraft("fluteCount", Math.round(value))} step={1} suffix="шт" value={draft.fluteCount} />
                  {(draft.kind === "vBit" || draft.kind === "engraving") && <OptionalNumberField label="Угол" max={179} min={1} onChange={(value) => mutateDraft("includedAngleDegrees", value)} placeholder="Неизвестен" step={1} suffix="°" value={draft.includedAngleDegrees} />}
                </div>
              </section>

              <section className="tool-form-section">
                <h3>Стартовые режимы</h3>
                <div className="tool-form-grid">
                  <NumberField label="Подача XY" max={100000} min={1} onChange={(value) => mutateDraft("feedMmPerMin", value)} step={50} suffix="mm/min" value={draft.feedMmPerMin} />
                  <NumberField label="Подача Z" max={50000} min={1} onChange={(value) => mutateDraft("plungeMmPerMin", value)} step={25} suffix="mm/min" value={draft.plungeMmPerMin} />
                  <NumberField label="Шпиндель" max={100000} min={1000} onChange={(value) => mutateDraft("spindleRpm", Math.round(value))} step={1000} suffix="rpm" value={draft.spindleRpm} />
                  <NumberField label="Съём за проход" max={draft.cuttingLengthMm} min={0.001} onChange={(value) => mutateDraft("stepdownMm", value)} step={0.05} suffix="mm" value={draft.stepdownMm} />
                  <NumberField label="Перекрытие шага" max={95} min={1} onChange={(value) => mutateDraft("stepoverPercent", value)} step={1} suffix="% Ø" value={draft.stepoverPercent} />
                </div>
              </section>

              <section className="tool-form-section">
                <h3>Описание</h3>
                <label className="tool-field is-wide"><textarea maxLength={2000} onChange={(event) => mutateDraft("description", event.target.value)} value={draft.description} /></label>
              </section>

              {selected && !creating && <ToolKnowledgePanel tool={{ ...selected, ...draft }} />}
            </div>
            <footer>
              <div className={`tool-editor-status${status?.toLowerCase().includes("invalid") ? " is-error" : ""}`} aria-live="polite"><BookOpen aria-hidden="true" size={14} />{status ?? "Режимы зависят от материала, станка и вылета инструмента."}</div>
              <div>
                {!creating && selected && <button className={deleteConfirming ? "is-danger" : ""} disabled={busy} onClick={() => void remove()} type="button"><Trash2 aria-hidden="true" size={14} />{deleteConfirming ? "Подтвердить" : "Удалить"}</button>}
                <button className="primary-action" disabled={busy} onClick={() => void save()} type="button"><Save aria-hidden="true" size={14} />{busy ? "Сохранение" : "Сохранить"}</button>
              </div>
            </footer>
          </main>
        </div>
      </section>
    </div>,
    document.body,
  );
}

function TextField({ label, value, onChange, wide = false }: { readonly label: string; readonly value: string; readonly onChange: (value: string) => void; readonly wide?: boolean }) {
  return <label className={`tool-field${wide ? " is-wide" : ""}`}><span>{label}</span><input maxLength={100} onChange={(event) => onChange(event.target.value)} type="text" value={value} /></label>;
}

function NumberField({ label, value, min, max, step, suffix, onChange }: { readonly label: string; readonly value: number; readonly min: number; readonly max: number; readonly step: number; readonly suffix: string; readonly onChange: (value: number) => void }) {
  const change = (event: ChangeEvent<HTMLInputElement>) => onChange(Number(event.target.value));
  return <label className="tool-field"><span>{label}</span><div><input max={max} min={min} onChange={change} step={step} type="number" value={value} /><small>{suffix}</small></div></label>;
}

function OptionalNumberField({ label, value, min, max, step, suffix, placeholder, onChange }: { readonly label: string; readonly value?: number; readonly min: number; readonly max: number; readonly step: number; readonly suffix: string; readonly placeholder: string; readonly onChange: (value?: number) => void }) {
  return <label className="tool-field"><span>{label}</span><div><input max={max} min={min} onChange={(event) => onChange(event.target.value === "" ? undefined : event.target.valueAsNumber)} placeholder={placeholder} step={step} type="number" value={value ?? ""} /><small>{suffix}</small></div></label>;
}

const readableError = (reason: unknown): string => String(reason).replace(/^Error:\s*/, "");
