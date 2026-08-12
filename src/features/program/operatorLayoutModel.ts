import type { SenderState } from "../../shared/dryRun";

export type SenderPrimaryAction = "start" | "pause" | "resume" | "none";

export interface SenderActionLayout {
  readonly primary: SenderPrimaryAction;
  readonly cancelVisible: boolean;
}

export const senderActionLayout = (state: SenderState): SenderActionLayout => {
  switch (state) {
    case "running":
      return { primary: "pause", cancelVisible: true };
    case "paused":
      return { primary: "resume", cancelVisible: true };
    case "toolChange":
    case "draining":
      return { primary: "none", cancelVisible: true };
    default:
      return { primary: "start", cancelVisible: false };
  }
};
