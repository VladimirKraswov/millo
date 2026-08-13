import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { emptySnapshot } from "../../shared/machine";
import { ProgramWorkspace } from "./ProgramWorkspace";
import {
  previewFixtureCompletedSender,
  previewFixtureFirstCutGateway,
  previewFixtureFirstCutProgram,
} from "./previewFixtureFirstCut";

describe("ProgramWorkspace completed-run workflow", () => {
  it("puts one safe work-origin return beside the repeat-run action", () => {
    const markup = renderToStaticMarkup(
      <ProgramWorkspace
        desktopRuntime={false}
        gateway={{ parse: async () => previewFixtureFirstCutProgram }}
        initialProgram={previewFixtureFirstCutProgram}
        initialRunIntent="cutting"
        initialSender={previewFixtureCompletedSender}
        machineContext={{
          activeCoordinateSystem: "G54",
          busy: false,
          machineBound: true,
          machineName: "Fixture CNC",
          machineSyncing: false,
          onAcknowledgeReset: () => undefined,
          onConnect: () => undefined,
          onOpenWorkZero: () => undefined,
          onReturnToWorkOrigin: async () => undefined,
          onSyncMachine: () => undefined,
          onUnlock: () => undefined,
          snapshot: {
            ...emptySnapshot,
            connection: "connected",
            machine: {
              ...emptySnapshot.machine,
              mode: "idle",
              reportedMode: "Idle",
              workPosition: { x: 12, y: 8, z: 5 },
            },
          },
          workPosition: { x: 12, y: 8, z: 5 },
        }}
        realRunAvailable
        realRunGateway={previewFixtureFirstCutGateway}
        realRunTarget
      />,
    );

    expect(markup).toContain("Вернуться в рабочий ноль");
    expect(markup).toContain("Подготовить повторный запуск");
    expect(markup).toContain("aria-label=\"Редактировать G-code\"");
  });
});
