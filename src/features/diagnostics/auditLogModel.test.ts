import { describe, expect, it } from "vitest";

import type { AuditEntry } from "../../shared/audit";
import { auditCounts, defaultAuditLevels, filterAuditEntries } from "./auditLogModel";

const entry = (
  sequence: number,
  level: AuditEntry["level"],
  category: AuditEntry["category"],
  message: string,
): AuditEntry => ({
  schemaVersion: 1,
  sequence,
  sessionId: "test",
  timestampMs: sequence,
  level,
  category,
  event: `${category}.test`,
  message,
  data: { sourceLine: sequence },
});

describe("audit log read model", () => {
  const entries = [
    entry(1, "debug", "controller", "Idle status"),
    entry(2, "warning", "program", "Preflight blocked"),
    entry(3, "error", "sender", "ALARM on source line"),
  ];

  it("hides debug noise by default and combines category with search", () => {
    expect(
      filterAuditEntries(entries, {
        category: "all",
        levels: defaultAuditLevels,
        query: "",
      }).map((item) => item.sequence),
    ).toEqual([2, 3]);
    expect(
      filterAuditEntries(entries, {
        category: "sender",
        levels: new Set(["error"]),
        query: "source line",
      }).map((item) => item.sequence),
    ).toEqual([3]);
  });

  it("counts warning and critical attention independently", () => {
    expect(auditCounts([...entries, entry(4, "critical", "storage", "Disk failed")])).toEqual({
      errors: 2,
      warnings: 1,
    });
  });
});
