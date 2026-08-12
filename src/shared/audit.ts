export type AuditLevel = "debug" | "info" | "warning" | "error" | "critical";

export type AuditCategory =
  | "application"
  | "transport"
  | "controller"
  | "sender"
  | "safety"
  | "program"
  | "storage"
  | "ui";

export interface AuditEntry {
  readonly schemaVersion: number;
  readonly sequence: number;
  readonly sessionId: string;
  readonly timestampMs: number;
  readonly level: AuditLevel;
  readonly category: AuditCategory;
  readonly event: string;
  readonly message: string;
  readonly data: unknown;
}

export interface AuditLogSnapshot {
  readonly entries: readonly AuditEntry[];
  readonly droppedEntries: number;
  readonly writeFailures: number;
  readonly activePath?: string;
  readonly sessionId: string;
}

export type AuditExportFormat = "jsonLines" | "text";

export interface AuditExportOutcome {
  readonly path: string;
  readonly entryCount: number;
}
