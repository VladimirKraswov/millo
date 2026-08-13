import type { ProgramExecutionOptions } from "../../shared/realRun";

export const sameExecutionOptions = (
  left: ProgramExecutionOptions,
  right: ProgramExecutionOptions,
): boolean =>
  left.optionalStop === right.optionalStop &&
  left.blockDelete === right.blockDelete &&
  left.surfaceMapId === right.surfaceMapId &&
  left.cuttingDepthAdjustmentUm === right.cuttingDepthAdjustmentUm;
