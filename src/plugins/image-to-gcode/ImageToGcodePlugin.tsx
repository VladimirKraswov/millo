import {
  Check,
  ChevronDown,
  Download,
  FileImage,
  FolderOpen,
  ImagePlus,
  Sparkles,
  X,
} from "lucide-react";
import { useEffect, useRef, useState, type ChangeEvent, type DragEvent } from "react";
import { createPortal } from "react-dom";

import type { PluginJobsCapability } from "../../platform/plugins/InMemoryPluginLoader";
import {
  defaultImageJobSettings,
  type GeneratedImageJob,
  type ImageJobFormat,
  type ImageJobSettings,
} from "../../shared/jobs";

const MAX_IMAGE_BYTES = 8 * 1024 * 1024;

interface ImageToGcodePluginProps {
  readonly jobs: PluginJobsCapability;
  readonly initialOpen?: boolean;
}

interface SelectedImage {
  readonly file: File;
  readonly format: ImageJobFormat;
  readonly previewUrl: string;
}

export function ImageToGcodePlugin({ jobs, initialOpen = false }: ImageToGcodePluginProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(initialOpen);
  const [dragging, setDragging] = useState(false);
  const [selected, setSelected] = useState<SelectedImage>();
  const [settings, setSettings] = useState<ImageJobSettings>(defaultImageJobSettings);
  const [generated, setGenerated] = useState<GeneratedImageJob>();
  const [busy, setBusy] = useState<"generate" | "save">();
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [vectorPreviewUrl, setVectorPreviewUrl] = useState<string>();

  useEffect(() => {
    if (!generated) {
      setVectorPreviewUrl(undefined);
      return;
    }
    const url = URL.createObjectURL(new Blob([generated.vectorSvg], { type: "image/svg+xml" }));
    setVectorPreviewUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [generated]);

  useEffect(() => () => {
    if (selected) URL.revokeObjectURL(selected.previewUrl);
  }, [selected]);

  useEffect(() => {
    if (!open) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) setOpen(false);
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [busy, open]);

  const chooseFile = (file?: File) => {
    if (!file) return;
    const extension = file.name.split(".").pop()?.toLowerCase();
    const format = extension === "svg" ? "svg" : extension === "png" ? "png" : undefined;
    if (!format) {
      setError("Поддерживаются SVG и PNG");
      return;
    }
    if (file.size <= 0 || file.size > MAX_IMAGE_BYTES) {
      setError("Файл должен быть непустым и не больше 8 MB");
      return;
    }
    setSelected((current) => {
      if (current) URL.revokeObjectURL(current.previewUrl);
      return { file, format, previewUrl: URL.createObjectURL(file) };
    });
    setGenerated(undefined);
    setNotice(undefined);
    setError(undefined);
  };

  const updateNumber = (key: keyof ImageJobSettings, value: string) => {
    const number = Number(value);
    setSettings((current) => ({ ...current, [key]: number }));
    setGenerated(undefined);
    setNotice(undefined);
  };

  const generate = async () => {
    if (!selected || busy) return;
    setBusy("generate");
    setError(undefined);
    setNotice(undefined);
    try {
      const sourceBase64 = await fileToBase64(selected.file);
      const result = await jobs.generateImage({
        sourceName: selected.file.name,
        sourceBase64,
        format: selected.format,
        settings,
      });
      setGenerated(result);
      setNotice("Траектория построена и проверена парсером Millo");
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
    setNotice(undefined);
    try {
      const outcome = await jobs.save(generated);
      if (outcome) setNotice(`Сохранено: ${outcome.path}`);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusy(undefined);
    }
  };

  const openJob = () => {
    if (!generated) return;
    try {
      jobs.open(generated);
      setOpen(false);
    } catch (reason) {
      setError(readableError(reason));
    }
  };

  return (
    <>
      <button
        className="image-job-launcher"
        onClick={() => setOpen(true)}
        title="Создать G-code из SVG или PNG"
        type="button"
      >
        <ImagePlus aria-hidden="true" size={16} />
        <span>Изображение</span>
      </button>

      {open && typeof document !== "undefined" && createPortal(
        <div className="machine-dialog-backdrop image-job-backdrop" role="presentation">
          <section
            aria-labelledby="image-job-title"
            aria-modal="true"
            className="machine-dialog image-job-dialog"
            role="dialog"
          >
            <header>
              <div>
                <span>Создание задания</span>
                <h2 id="image-job-title">Изображение в G-code</h2>
              </div>
              <button
                aria-label="Закрыть"
                disabled={Boolean(busy)}
                onClick={() => setOpen(false)}
                title="Закрыть"
                type="button"
              >
                <X aria-hidden="true" size={18} />
              </button>
            </header>

            <div className="image-job-body">
              <section className="image-job-preview" aria-label="Предпросмотр изображения">
                <div
                  className={`image-drop-zone${dragging ? " is-dragging" : ""}${selected ? " has-image" : ""}`}
                  onDragEnter={(event) => { event.preventDefault(); setDragging(true); }}
                  onDragOver={(event) => event.preventDefault()}
                  onDragLeave={() => setDragging(false)}
                  onDrop={(event: DragEvent<HTMLDivElement>) => {
                    event.preventDefault();
                    setDragging(false);
                    chooseFile(event.dataTransfer.files[0]);
                  }}
                >
                  {selected ? (
                    <img
                      alt={`Предпросмотр ${selected.file.name}`}
                      src={vectorPreviewUrl ?? selected.previewUrl}
                    />
                  ) : (
                    <div>
                      <FileImage aria-hidden="true" size={32} />
                      <strong>Перетащите SVG или PNG</strong>
                      <span>.svg · .png · до 8 MB</span>
                    </div>
                  )}
                </div>
                <input
                  accept=".svg,.png,image/svg+xml,image/png"
                  hidden
                  onChange={(event: ChangeEvent<HTMLInputElement>) => {
                    chooseFile(event.target.files?.[0]);
                    event.target.value = "";
                  }}
                  ref={inputRef}
                  type="file"
                />
                <button className="image-file-action" onClick={() => inputRef.current?.click()} type="button">
                  <FolderOpen aria-hidden="true" size={15} />
                  {selected ? "Заменить изображение" : "Выбрать изображение"}
                </button>
                <div className="image-source-meta" aria-live="polite">
                  <span>{selected?.file.name ?? "Файл не выбран"}</span>
                  <code>{selected ? `${selected.format.toUpperCase()} · ${formatBytes(selected.file.size)}` : "SVG / PNG"}</code>
                </div>
                {generated && (
                  <dl className="image-job-result">
                    <div><dt>Размер</dt><dd>{generated.summary.widthMm.toFixed(1)} × {generated.summary.heightMm.toFixed(1)} mm</dd></div>
                    <div><dt>Контуры</dt><dd>{generated.summary.pathCount}</dd></div>
                    <div><dt>Точки</dt><dd>{generated.summary.pointCount}</dd></div>
                    <div><dt>G-code</dt><dd>{generated.program.summary.lineCount} строк</dd></div>
                  </dl>
                )}
              </section>

              <section className="image-job-settings" aria-label="Параметры гравировки">
                <div className="image-job-fields">
                  <NumberField label="Ширина" max={5000} min={0.1} onChange={(value) => updateNumber("widthMm", value)} step={1} suffix="mm" value={settings.widthMm} />
                  <NumberField label="Глубина" max={10} min={0.001} onChange={(value) => updateNumber("engravingDepthMm", value)} step={0.05} suffix="mm" value={settings.engravingDepthMm} />
                  <NumberField label="Безопасный Z" max={1000} min={-1000} onChange={(value) => updateNumber("safeZMm", value)} step={0.5} suffix="mm" value={settings.safeZMm} />
                  <NumberField label="Подача XY" max={20000} min={1} onChange={(value) => updateNumber("feedMmPerMin", value)} step={50} suffix="mm/min" value={settings.feedMmPerMin} />
                  <NumberField label="Подача Z" max={10000} min={1} onChange={(value) => updateNumber("plungeMmPerMin", value)} step={25} suffix="mm/min" value={settings.plungeMmPerMin} />
                  <NumberField label="Точность кривых" max={2} min={0.005} onChange={(value) => updateNumber("curveToleranceMm", value)} step={0.01} suffix="mm" value={settings.curveToleranceMm} />
                </div>

                {selected?.format === "png" && (
                  <details className="image-trace-settings">
                    <summary><span>Векторизация PNG</span><ChevronDown aria-hidden="true" size={14} /></summary>
                    <div>
                      <label className="image-range-field">
                        <span><strong>Порог яркости</strong><code>{settings.rasterThresholdPercent}%</code></span>
                        <input
                          max="99"
                          min="1"
                          onChange={(event) => updateNumber("rasterThresholdPercent", event.target.value)}
                          type="range"
                          value={settings.rasterThresholdPercent}
                        />
                      </label>
                      <NumberField label="Удалять шум до" max={64} min={1} onChange={(value) => updateNumber("traceSpecklePx", value)} step={1} suffix="px" value={settings.traceSpecklePx} />
                      <label className="image-invert-field">
                        <input
                          checked={settings.invert}
                          onChange={(event) => {
                            setSettings((current) => ({ ...current, invert: event.target.checked }));
                            setGenerated(undefined);
                          }}
                          type="checkbox"
                        />
                        <span>Инвертировать светлое и тёмное</span>
                      </label>
                    </div>
                  </details>
                )}

                <div className={`image-job-status${error ? " is-error" : generated ? " is-ready" : ""}`} aria-live="polite">
                  {error ? error : notice ? <><Check aria-hidden="true" size={15} />{notice}</> : "Выберите изображение и проверьте физический размер."}
                </div>
              </section>
            </div>

            <footer className="image-job-footer">
              <button disabled={!generated || Boolean(busy)} onClick={() => void save()} type="button">
                <Download aria-hidden="true" size={15} />
                {busy === "save" ? "Сохранение" : "Сохранить .nc"}
              </button>
              <div>
                <button
                  className="image-generate-action"
                  disabled={!selected || Boolean(busy)}
                  onClick={() => void generate()}
                  type="button"
                >
                  <Sparkles aria-hidden="true" size={15} />
                  {busy === "generate" ? "Векторизация…" : generated ? "Пересчитать" : "Создать G-code"}
                </button>
                <button className="primary-action" disabled={!generated || Boolean(busy)} onClick={openJob} type="button">
                  <FolderOpen aria-hidden="true" size={15} />
                  Открыть в задании
                </button>
              </div>
            </footer>
          </section>
        </div>,
        document.body,
      )}
    </>
  );
}

interface NumberFieldProps {
  readonly label: string;
  readonly value: number;
  readonly min: number;
  readonly max: number;
  readonly step: number;
  readonly suffix: string;
  readonly onChange: (value: string) => void;
}

function NumberField({ label, value, min, max, step, suffix, onChange }: NumberFieldProps) {
  return (
    <label className="image-number-field">
      <span>{label}</span>
      <div>
        <input max={max} min={min} onChange={(event) => onChange(event.target.value)} step={step} type="number" value={value} />
        <small>{suffix}</small>
      </div>
    </label>
  );
}

async function fileToBase64(file: File): Promise<string> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const chunkSize = 32_768;
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return window.btoa(binary);
}

const formatBytes = (bytes: number): string =>
  bytes >= 1024 * 1024
    ? `${(bytes / (1024 * 1024)).toFixed(1)} MB`
    : `${Math.max(1, Math.round(bytes / 1024))} KB`;

const readableError = (reason: unknown): string =>
  String(reason).replace(/^Error:\s*/, "");
