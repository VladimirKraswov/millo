import { describe, expect, it } from "vitest";

import { idleSenderSnapshot } from "../../shared/dryRun";
import type { CuttingTool } from "../../shared/tooling";
import { programToolVisualization } from "./programToolVisualizationModel";
import { previewFixtureFirstCutProgram } from "./previewFixtureFirstCut";

const tools = [
  { id: "engraver", name: "Engraver" },
  { id: "end-mill", name: "End mill" },
] as CuttingTool[];
const assignments = [
  { toolNumber: 1, toolId: "engraver" },
  { toolNumber: 2, toolId: "end-mill" },
];

describe("programToolVisualization", () => {
  it("resolves a generated-job tool and spins only during a cutting run", () => {
    const selected = programToolVisualization(
      previewFixtureFirstCutProgram,
      idleSenderSnapshot,
      "cutting",
      assignments,
      tools,
    );
    const running = programToolVisualization(
      previewFixtureFirstCutProgram,
      {
        ...idleSenderSnapshot,
        mode: "cutRun",
        state: "running",
        executingSourceLine: 4,
      },
      "cutting",
      assignments,
      tools,
    );

    expect(selected.tool?.id).toBe("engraver");
    expect(selected.state).toBe("selected");
    expect(running.spinning).toBe(true);
    expect(running.state).toBe("spinning");
  });

  it("shows the requested geometry at a host-managed tool-change barrier", () => {
    const visualization = programToolVisualization(
      previewFixtureFirstCutProgram,
      {
        ...idleSenderSnapshot,
        mode: "cutRun",
        state: "toolChange",
        requestedTool: 2,
      },
      "cutting",
      assignments,
      tools,
    );

    expect(visualization.tool?.id).toBe("end-mill");
    expect(visualization.state).toBe("changing");
    expect(visualization.spinning).toBe(false);
  });

  it("removes the cutter body from a physical motion check", () => {
    const visualization = programToolVisualization(
      previewFixtureFirstCutProgram,
      { ...idleSenderSnapshot, mode: "airRun", state: "running" },
      "airRun",
      assignments,
      tools,
    );

    expect(visualization.showCutter).toBe(false);
    expect(visualization.state).toBe("removed");
  });
});
