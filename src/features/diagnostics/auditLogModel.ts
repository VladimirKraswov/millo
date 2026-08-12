import type { AuditCategory, AuditEntry, AuditLevel } from "../../shared/audit";

export interface AuditLogFilter {
  readonly category: AuditCategory | "all";
  readonly levels: ReadonlySet<AuditLevel>;
  readonly query: string;
}

export const defaultAuditLevels = new Set<AuditLevel>([
  "info",
  "warning",
  "error",
  "critical",
]);

export const filterAuditEntries = (
  entries: readonly AuditEntry[],
  filter: AuditLogFilter,
): readonly AuditEntry[] => {
  const query = filter.query.trim().toLocaleLowerCase();
  return entries.filter((entry) => {
    if (!filter.levels.has(entry.level)) return false;
    if (filter.category !== "all" && entry.category !== filter.category) return false;
    if (!query) return true;
    const data = entry.data === null || entry.data === undefined ? "" : JSON.stringify(entry.data);
    return [entry.message, entry.event, entry.category, data]
      .join(" ")
      .toLocaleLowerCase()
      .includes(query);
  });
};

export const auditCounts = (entries: readonly AuditEntry[]) => ({
  errors: entries.filter((entry) => entry.level === "error" || entry.level === "critical")
    .length,
  warnings: entries.filter((entry) => entry.level === "warning").length,
});
