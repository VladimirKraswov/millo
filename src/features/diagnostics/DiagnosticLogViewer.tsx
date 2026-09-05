import { DialogSurface } from "../../components/DialogSurface";
import {
  AlertCircle,
  Bug,
  Download,
  FileText,
  RefreshCw,
  Search,
  TriangleAlert,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { exportDiagnosticLog, getDiagnosticLog } from "../../api/audit";
import type {
  AuditCategory,
  AuditExportFormat,
  AuditLevel,
  AuditLogSnapshot,
} from "../../shared/audit";
import {
  auditCounts,
  defaultAuditLevels,
  filterAuditEntries,
} from "./auditLogModel";

const categories: readonly (AuditCategory | "all")[] = [
  "all",
  "program",
  "sender",
  "controller",
  "transport",
  "safety",
  "storage",
  "application",
  "ui",
];

const levelLabels: Record<AuditLevel, string> = {
  debug: "Отладка",
  info: "События",
  warning: "Предупреждения",
  error: "Ошибки",
  critical: "Критические",
};

interface DiagnosticLogViewerProps {
  readonly desktopRuntime: boolean;
  readonly initialSnapshot?: AuditLogSnapshot;
  readonly onClose: () => void;
  readonly onError: (message: string) => void;
  readonly open: boolean;
}

const emptySnapshot: AuditLogSnapshot = {
  entries: [],
  droppedEntries: 0,
  writeFailures: 0,
  sessionId: "not-started",
};

export function DiagnosticLogViewer({
  desktopRuntime,
  initialSnapshot,
  onClose,
  onError,
  open,
}: DiagnosticLogViewerProps) {
  const [snapshot, setSnapshot] = useState(initialSnapshot ?? emptySnapshot);
  const [category, setCategory] = useState<AuditCategory | "all">("all");
  const [levels, setLevels] =
    useState<ReadonlySet<AuditLevel>>(defaultAuditLevels);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(false);
  const [exporting, setExporting] = useState(false);

  const refresh = async () => {
    if (!desktopRuntime) return;
    setLoading(true);
    try {
      setSnapshot(await getDiagnosticLog(1_000));
    } catch (error) {
      onError(String(error));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (!open || !desktopRuntime) return;
    void refresh();
    const timer = window.setInterval(() => void refresh(), 1_500);
    return () => window.clearInterval(timer);
  }, [desktopRuntime, open]);

  const visibleEntries = useMemo(
    () => filterAuditEntries(snapshot.entries, { category, levels, query }),
    [category, levels, query, snapshot.entries],
  );
  const counts = auditCounts(snapshot.entries);

  if (!open) return null;

  const toggleLevel = (level: AuditLevel) => {
    const next = new Set(levels);
    if (next.has(level)) next.delete(level);
    else next.add(level);
    setLevels(next);
  };
  const save = async (format: AuditExportFormat) => {
    setExporting(true);
    try {
      await exportDiagnosticLog(format);
      await refresh();
    } catch (error) {
      onError(String(error));
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="log-viewer-backdrop" role="presentation">
      <DialogSurface
        onDismiss={onClose}
        modal={false}
        aria-label="Журнал диагностики"
        className="log-viewer"
      >
        <header>
          <div>
            <span>Диагностика</span>
            <strong>Журнал событий</strong>
            <small>
              {snapshot.activePath ?? "Постоянный файл доступен в Tauri"}
            </small>
          </div>
          <dl>
            <div className={counts.errors > 0 ? "has-errors" : undefined}>
              <dt>Ошибки</dt>
              <dd>{counts.errors}</dd>
            </div>
            <div className={counts.warnings > 0 ? "has-warnings" : undefined}>
              <dt>Предупреждения</dt>
              <dd>{counts.warnings}</dd>
            </div>
            <div>
              <dt>События</dt>
              <dd>{snapshot.entries.length}</dd>
            </div>
          </dl>
          <div className="log-header-actions">
            <button
              aria-label="Обновить журнал"
              disabled={loading || !desktopRuntime}
              onClick={() => void refresh()}
              title="Обновить"
              type="button"
            >
              <RefreshCw aria-hidden="true" size={15} />
            </button>
            <button
              aria-label="Закрыть журнал"
              onClick={onClose}
              title="Закрыть"
              type="button"
            >
              <X aria-hidden="true" size={16} />
            </button>
          </div>
        </header>

        <div className="log-toolbar">
          <label className="log-search">
            <Search aria-hidden="true" size={14} />
            <input
              aria-label="Поиск по журналу"
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Поиск: ALARM, строка, команда..."
              value={query}
            />
          </label>
          <select
            aria-label="Категория журнала"
            onChange={(event) =>
              setCategory(event.target.value as AuditCategory | "all")
            }
            value={category}
          >
            {categories.map((item) => (
              <option key={item} value={item}>
                {item === "all" ? "Все категории" : item}
              </option>
            ))}
          </select>
          <div className="log-levels" role="group" aria-label="Уровни журнала">
            {(Object.keys(levelLabels) as AuditLevel[]).map((level) => (
              <button
                aria-pressed={levels.has(level)}
                className={`is-${level}`}
                key={level}
                onClick={() => toggleLevel(level)}
                type="button"
              >
                {levelLabels[level]}
              </button>
            ))}
          </div>
          <div className="log-export-actions">
            <button
              disabled={exporting || !desktopRuntime}
              onClick={() => void save("text")}
              type="button"
            >
              <FileText aria-hidden="true" size={14} />
              .log
            </button>
            <button
              disabled={exporting || !desktopRuntime}
              onClick={() => void save("jsonLines")}
              type="button"
            >
              <Download aria-hidden="true" size={14} />
              .jsonl
            </button>
          </div>
        </div>

        <div
          aria-hidden={
            snapshot.droppedEntries === 0 && snapshot.writeFailures === 0
          }
          className={`log-health-warning${
            snapshot.droppedEntries === 0 && snapshot.writeFailures === 0
              ? " is-empty"
              : ""
          }`}
          role="alert"
        >
          <TriangleAlert aria-hidden="true" size={14} />
          Очередь потеряла {snapshot.droppedEntries} событий; ошибок записи:{" "}
          {snapshot.writeFailures}
        </div>

        <div className="log-stream" role="log">
          {visibleEntries.length === 0 ? (
            <div className="log-empty">
              <Bug aria-hidden="true" size={24} />
              <strong>Событий по этому фильтру нет</strong>
              <span>Включите уровень «Отладка» или измените категорию</span>
            </div>
          ) : (
            [...visibleEntries].reverse().map((entry) => (
              <details
                className={`log-entry is-${entry.level}`}
                key={`${entry.sessionId}-${entry.sequence}`}
              >
                <summary>
                  <i aria-hidden="true" />
                  <time dateTime={new Date(entry.timestampMs).toISOString()}>
                    {new Date(entry.timestampMs).toLocaleTimeString("ru-RU", {
                      hour12: false,
                      hour: "2-digit",
                      minute: "2-digit",
                      second: "2-digit",
                      fractionalSecondDigits: 3,
                    })}
                  </time>
                  <code>{entry.category}</code>
                  <span>
                    <strong>{entry.message}</strong>
                    <small>{entry.event}</small>
                  </span>
                  <em>#{entry.sequence}</em>
                  {(entry.level === "error" || entry.level === "critical") && (
                    <AlertCircle aria-hidden="true" size={14} />
                  )}
                </summary>
                <pre>{JSON.stringify(entry.data, null, 2)}</pre>
              </details>
            ))
          )}
        </div>
        <footer>
          <span>Сессия {snapshot.sessionId}</span>
          <strong>
            {visibleEntries.length} из {snapshot.entries.length}
          </strong>
        </footer>
      </DialogSurface>
    </div>
  );
}
