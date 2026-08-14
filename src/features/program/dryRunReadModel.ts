import type { SenderSnapshot } from "../../shared/dryRun";

export interface SenderTimingReadModel {
  readonly elapsed: string;
  readonly estimateLabel: "ETA" | "ETA >=";
  readonly remaining: string;
}

export interface SenderHeartbeatReadModel {
  readonly sequence: number;
  readonly lastLine: string;
  readonly age: string;
  readonly shutdownAcknowledged: boolean;
}

const failureLabels: Record<NonNullable<SenderSnapshot["failure"]>["kind"], string> = {
  grblError: "GRBL error",
  alarm: "GRBL alarm",
  reset: "Controller reset",
  timeout: "Timeout",
  disconnected: "Disconnected",
  transport: "Transport error",
  unsafeState: "Unsafe controller state",
  internal: "Sender error",
};

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

export const senderHeartbeat = (
  sender: SenderSnapshot,
): SenderHeartbeatReadModel => {
  const reportedAge = sender.secondsSinceAcknowledgement ?? 0;
  const age = Number.isFinite(reportedAge) ? Math.max(0, reportedAge) : 0;
  return {
    sequence: Math.max(
      0,
      Math.trunc(sender.progressSequence ?? sender.acknowledgedLines),
    ),
    lastLine:
      sender.lastAcknowledgedSourceLine === undefined
        ? "Guard"
        : `L${sender.lastAcknowledgedSourceLine}`,
    age: age < 10 ? `${age.toFixed(1)}s` : `${Math.round(age)}s`,
    shutdownAcknowledged: sender.shutdownCommandsAcknowledged ?? false,
  };
};

export const senderFailureSummary = (
  sender: SenderSnapshot,
): string | undefined => {
  const failure = sender.failure;
  if (!failure) return sender.lastError;
  const code = failure.grblCode === undefined ? "" : ` ${failure.grblCode}`;
  const line = failure.sourceLine === undefined ? "" : ` · L${failure.sourceLine}`;
  return `${failureLabels[failure.kind]}${code}${line}`;
};
