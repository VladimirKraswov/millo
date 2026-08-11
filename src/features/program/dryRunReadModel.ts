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

export const dryRunControls = (
  sender: SenderSnapshot,
  context: DryRunControlContext,
): DryRunControls => {
  const active = sender.state === "running" || sender.state === "paused";
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

