import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { ProgramRecoveryCandidate } from "../../shared/recovery";
import { ProgramRecoveryDialog } from "./ProgramRecoveryDialog";

const candidate: ProgramRecoveryCandidate = {
  acknowledgedLines: 936,
  checkpointRestartAvailable: true,
  detail: "Последний подтверждённый участок сохранён",
  executingSourceLine: 911,
  fullRestartAvailable: true,
  id: 7,
  intent: "cutting",
  interruption: "controllerDisconnected",
  minimumSafeZMm: 5,
  ready: true,
  restartPosition: { x: 48.2, y: 31.7, z: 5 },
  restartSourceLine: 884,
  sourceName: "engraving.nc",
  state: "running",
  totalLines: 1_420,
  updatedAtUnixMs: 1,
};

describe("ProgramRecoveryDialog", () => {
  it("explains uncertain completion without claiming that the machine stopped", () => {
    const markup = renderToStaticMarkup(
      <ProgramRecoveryDialog
        candidate={candidate}
        onClose={() => undefined}
        onDismiss={async () => undefined}
        onPrepare={async () => {
          throw new Error("not used during server render");
        }}
        onPrepared={() => undefined}
        open
      />,
    );

    expect(markup).toContain("Завершение не подтверждено");
    expect(markup).toContain("Работа уже завершена");
    expect(markup).toContain("Подготовить повторный запуск");
    expect(markup).toContain("станок дошёл до конца");
    expect(markup).not.toContain("must be recovered or dismissed");
  });
});
