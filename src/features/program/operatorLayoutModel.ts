import type { SenderSnapshot, SenderState } from "../../shared/dryRun";

export type SenderPrimaryAction = "start" | "pause" | "resume" | "none";

export interface SenderActionLayout {
  readonly primary: SenderPrimaryAction;
  readonly cancelVisible: boolean;
}

export type PhysicalSenderPrimaryAction =
  | "pause"
  | "resume"
  | "toolChange"
  | "prepareRerun"
  | "resolveInterruption"
  | "none";

export interface PhysicalSenderActionLayout {
  readonly primary: PhysicalSenderPrimaryAction;
  readonly stopVisible: boolean;
}

export type CheckSenderAction = "cancel" | "returnToPreparation" | "none";

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

export const physicalSenderActionLayout = (
  state: SenderState,
): PhysicalSenderActionLayout => {
  switch (state) {
    case "running":
    case "draining":
      return { primary: "pause", stopVisible: true };
    case "paused":
      return { primary: "resume", stopVisible: true };
    case "toolChange":
      return { primary: "toolChange", stopVisible: true };
    case "completed":
      return { primary: "prepareRerun", stopVisible: false };
    case "failed":
    case "cancelled":
      return { primary: "resolveInterruption", stopVisible: false };
    default:
      return { primary: "none", stopVisible: false };
  }
};

export const checkSenderAction = (state: SenderState): CheckSenderAction => {
  switch (state) {
    case "running":
    case "paused":
    case "toolChange":
    case "draining":
      return "cancel";
    case "failed":
    case "cancelled":
      return "returnToPreparation";
    default:
      return "none";
  }
};

export const senderRunIsVisibleForProgram = (
  sender: Pick<SenderSnapshot, "runSequence" | "sourceName">,
  programSourceName: string | undefined,
  clearedRunSequence: number | undefined,
): boolean =>
  sender.sourceName === programSourceName &&
  sender.runSequence !== clearedRunSequence;
