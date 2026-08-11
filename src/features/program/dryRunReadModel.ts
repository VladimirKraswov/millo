import type { SenderSnapshot } from "../../shared/dryRun";

export interface DryRunControlContext {
  readonly mockAvailable: boolean;
  readonly policyEligible: boolean;
  readonly loading: boolean;
}

export interface DryRunControls {
  readonly canStart: boolean;
  readonly canPause: boolean;
  readonly canResume: boolean;
  readonly canCancel: boolean;
  readonly active: boolean;
  readonly progressPercent: number;
}

export interface SenderTimingReadModel {
  readonly elapsed: string;
  readonly estimateLabel: "ETA" | "ETA >=";
  readonly remaining: string;
}

const formatRuntime = (seconds: number): string => {
  const rounded = Math.max(0, Math.round(seconds));
  const hours = Math.floor(rounded / 3_600);
  const minutes = Math.floor((rounded % 3_600) / 60);
  const remainder = rounded % 60;
  return hours > 0
    ? `${hours}:${minutes.toString().padStart(2, "0")}:${remainder.toString().padStart(2, "0")}`
    : `${minutes}:${remainder.toString().padStart(2, "0")}`;
};

export const senderTiming = (
  sender: SenderSnapshot,
): SenderTimingReadModel => ({
  elapsed: formatRuntime(sender.elapsedSeconds),
  estimateLabel: sender.timeEstimateComplete ? "ETA" : "ETA >=",
  remaining: formatRuntime(sender.estimatedRemainingSeconds),
});

export const dryRunControls = (
  sender: SenderSnapshot,
  context: DryRunControlContext,
): DryRunControls => {
  const active = ["running", "paused", "draining"].includes(sender.state);
  return {
    canStart:
      !active &&
      context.mockAvailable &&
      context.policyEligible &&
      !context.loading,
    canPause: sender.state === "running",
    canResume: sender.state === "paused" && context.mockAvailable,
    canCancel: active,
    active,
    progressPercent: Math.round(
      Math.min(1, Math.max(0, sender.progress)) * 100,
    ),
  };
};
