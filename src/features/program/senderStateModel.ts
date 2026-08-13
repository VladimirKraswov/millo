import type { SenderState } from "../../shared/dryRun";

const activeSenderStates: ReadonlySet<SenderState> = new Set([
  "running",
  "paused",
  "toolChange",
  "draining",
]);

const terminalSenderStates: ReadonlySet<SenderState> = new Set([
  "completed",
  "failed",
  "cancelled",
]);

export const isSenderActive = (state: SenderState): boolean =>
  activeSenderStates.has(state);

export const isSenderTerminal = (state: SenderState): boolean =>
  terminalSenderStates.has(state);
