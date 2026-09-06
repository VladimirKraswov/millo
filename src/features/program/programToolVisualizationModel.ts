import type { SenderSnapshot } from "../../shared/dryRun";
import type { JobToolAssignment } from "../../shared/jobs";
import type { GcodeProgram } from "../../shared/program";
import type { ProgramRunIntent } from "../../shared/realRun";
import type { CuttingTool } from "../../shared/tooling";
import { programToolNumberAtSourceLine } from "./programToolPlanModel";

export type ProgramToolVisualState =
  | "selected"
  | "spinning"
  | "paused"
  | "changing"
  | "removed";

export interface ProgramToolVisualization {
  readonly tool?: CuttingTool;
  readonly toolId?: string;
  readonly toolNumber?: number;
  readonly state: ProgramToolVisualState;
  readonly showCutter: boolean;
  readonly spinning: boolean;
}

export function programToolVisualization(
  program: Pick<GcodeProgram, "lines" | "document">,
  sender: SenderSnapshot,
  intent: ProgramRunIntent,
  assignments: readonly JobToolAssignment[],
  tools: readonly CuttingTool[],
): ProgramToolVisualization {
  const executionLine = sender.executingSourceLine ??
    sender.lastAcknowledgedSourceLine ??
    sender.currentSourceLine;
  const inferredToolNumber = programToolNumberAtSourceLine(program, executionLine);
  const unknownIndexedTool = program.document !== undefined && executionLine !== undefined && executionLine > program.document.toolSelectionCoverageLine;
  const toolNumber = sender.state === "toolChange"
    ? (sender.requestedTool ?? inferredToolNumber)
    : (inferredToolNumber ?? (unknownIndexedTool ? undefined : assignments[0]?.toolNumber));
  const assignment = unknownIndexedTool ? undefined : assignments.find((candidate) => candidate.toolNumber === toolNumber) ??
    (toolNumber === undefined ? assignments[0] : undefined);
  const tool = tools.find((candidate) => candidate.id === assignment?.toolId);
  const showCutter = intent === "cutting" && sender.mode !== "airRun";
  const spinning = showCutter && sender.mode === "cutRun" &&
    (sender.state === "running" || sender.state === "draining");
  const state: ProgramToolVisualState = !showCutter
    ? "removed"
    : sender.state === "toolChange"
      ? "changing"
      : spinning
        ? "spinning"
        : sender.state === "paused"
          ? "paused"
          : "selected";

  return {
    tool,
    toolId: assignment?.toolId,
    toolNumber: toolNumber ?? assignment?.toolNumber,
    state,
    showCutter,
    spinning,
  };
}
