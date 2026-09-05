import { invoke } from "@tauri-apps/api/core";
import { isDesktopRuntime } from "./controller";

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

export function reportUiError(component: string, error: unknown, stack = ""): void {
  if (!isDesktopRuntime()) {
    console.error(`[${component}]`, error);
    return;
  }
  void invoke("report_ui_error", {
    component: component.slice(0, 160),
    message: String(error).slice(0, 4000),
    stack: stack.slice(0, 8000),
  }).catch((failure: unknown) => console.error("UI diagnostics unavailable", failure));
}
