import { invoke } from "@tauri-apps/api/core";

import type {
  AuditExportFormat,
  AuditExportOutcome,
  AuditLogSnapshot,
} from "../shared/audit";

export const getDiagnosticLog = (limit = 500): Promise<AuditLogSnapshot> =>
  invoke<AuditLogSnapshot>("diagnostic_log_snapshot", { limit });

export const exportDiagnosticLog = (
  format: AuditExportFormat,
): Promise<AuditExportOutcome | null> =>
  invoke<AuditExportOutcome | null>("export_diagnostic_log", { format });
